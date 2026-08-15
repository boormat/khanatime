//! QR parcel codec — offline handoff of an event's message log.
//!
//! A parcel is the event's durable log wrapped in a plain-text envelope so it
//! can be carried phone-to-phone via QR, copy/paste or any text channel when
//! the room transports are unavailable.  Import is the same path as a room
//! message: `append_log` (content-id dedup makes re-import idempotent) then
//! replay.  Export marks the exporter's own outbox published (`publish_outbox`)
//! — handing a message off is publishing it.
//!
//! Pure Rust (no wasm gating) so `cargo test` covers the codec natively.
//!
//! A parcel bigger than one QR can hold is split into **frames** (`QR_PREFIX`):
//! each frame is a small self-describing `khanatime_qr:{json}` string carrying
//! `index/total` + a shared `id`, rendered as its own QR and shown in sequence.
//! The receiving camera decodes one frame at a time; `assemble_frames` groups
//! by `id`, orders by `index`, checks completeness and re-joins the parcel.
//! `qr_svg` renders any of these strings as an SVG QR code.

use serde::{Deserialize, Serialize};

pub const PARCEL_PREFIX: &str = "khanatime_parcel:";
pub const QR_PREFIX: &str = "khanatime_qr:";

/// Max characters of parcel payload per frame — a comfortable single-QR size.
pub const MAX_FRAME_DATA: usize = 1200;

/// One frame of a chunked parcel: a slice of the parcel text plus framing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub v: u8,
    /// Shared by every frame of one parcel so a scanner groups them.
    pub id: String,
    pub total: u32,
    pub index: u32,
    pub data: String,
}

/// One message inside a parcel — the same `body` strings the room carries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParcelMsg {
    pub body: String,
    pub ts: i64,
    pub sender: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Parcel {
    pub v: u8,
    pub event_uid: String,
    pub created: i64,
    pub msgs: Vec<ParcelMsg>,
}

/// Pack the event's durable log into a parcel envelope.  `event_uid` keys the
/// parcel so imports only land in the matching event.
pub fn pack_parcel(event_uid: &str, msgs: &[crate::log::LogMsg]) -> String {
    let parcel = Parcel {
        v: 1,
        event_uid: event_uid.to_string(),
        created: crate::log::now_ms(),
        msgs: msgs
            .iter()
            .map(|m| ParcelMsg {
                body: m.body.clone(),
                ts: m.ts,
                sender: m.sender.clone(),
            })
            .collect(),
    };
    format!("{PARCEL_PREFIX}{}", serde_json::to_string(&parcel).unwrap())
}

/// Parse and validate a parcel envelope.
pub fn unpack_parcel(text: &str) -> Result<Parcel, String> {
    let json = text
        .strip_prefix(PARCEL_PREFIX)
        .ok_or_else(|| format!("not a parcel — missing '{PARCEL_PREFIX}' prefix"))?;
    let parcel: Parcel = serde_json::from_str(json).map_err(|e| format!("bad parcel json: {e}"))?;
    if parcel.v != 1 {
        return Err(format!("unsupported parcel version {}", parcel.v));
    }
    if parcel.event_uid.is_empty() {
        return Err("parcel has no event_uid".to_string());
    }
    Ok(parcel)
}

/// Split `parcel` into frames, each `khanatime_qr:{json}`.  A single frame for
/// a small parcel; chunked (by character, to keep UTF-8 intact) for big ones.
pub fn pack_frames(parcel: &str) -> Vec<String> {
    let chunks = chunk_data(parcel, MAX_FRAME_DATA);
    let total = chunks.len() as u32;
    let id = crate::ids::gen_short_id();
    chunks
        .into_iter()
        .enumerate()
        .map(|(i, data)| {
            let frame = Frame {
                v: 1,
                id: id.clone(),
                total,
                index: i as u32,
                data,
            };
            format!("{QR_PREFIX}{}", serde_json::to_string(&frame).unwrap())
        })
        .collect()
}

/// Parse a single frame string.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // wasm scan sink + tests
pub fn unpack_frame(text: &str) -> Result<Frame, String> {
    let json = text
        .strip_prefix(QR_PREFIX)
        .ok_or_else(|| format!("not a QR frame — missing '{QR_PREFIX}' prefix"))?;
    let frame: Frame = serde_json::from_str(json).map_err(|e| format!("bad frame json: {e}"))?;
    if frame.v != 1 {
        return Err(format!("unsupported frame version {}", frame.v));
    }
    if frame.total == 0 || frame.index >= frame.total {
        return Err(format!("bad frame index {}/{}", frame.index, frame.total));
    }
    Ok(frame)
}

/// Re-join a set of frames into the original parcel string.  All frames must
/// share one `id`, cover every `0..total`, and be present exactly once.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // wasm scan sink + tests
pub fn assemble_frames(frames: &[Frame]) -> Result<String, String> {
    let Some(first) = frames.first() else {
        return Err("no frames to assemble".to_string());
    };
    let id = first.id.clone();
    let total = first.total as usize;
    if frames.iter().any(|f| f.id != id) {
        return Err("frames from different parcels".to_string());
    }
    if total == 0 {
        return Err("bad frame total".to_string());
    }
    let mut slots: Vec<Option<&Frame>> = vec![None; total];
    for f in frames {
        if (f.index as usize) >= total {
            return Err("bad frame index".to_string());
        }
        slots[f.index as usize] = Some(f);
    }
    if slots.iter().any(|s| s.is_none()) {
        return Err(format!("incomplete — {}/{} frames", frames.len(), total));
    }
    let mut out = String::new();
    for slot in slots.into_iter().flatten() {
        out.push_str(&slot.data);
    }
    Ok(out)
}

/// Render `data` as an SVG QR code (white quiet border, black modules).
pub fn qr_svg(data: &str) -> Option<String> {
    let code = qrcode::QrCode::new(data).ok()?;
    let n = code.width() as u32;
    let scale = 4u32;
    let quiet = 4u32;
    let size = (n + 2 * quiet) * scale;
    let origin = quiet * scale;
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{size}\" height=\"{size}\" viewBox=\"0 0 {size} {size}\"><rect width=\"{size}\" height=\"{size}\" fill=\"#fff\"/>"
    );
    for (i, color) in code.to_colors().iter().enumerate() {
        if *color == qrcode::Color::Dark {
            let x = origin + (i as u32 % n) * scale;
            let y = origin + (i as u32 / n) * scale;
            svg.push_str(&format!(
                "<rect x=\"{x}\" y=\"{y}\" width=\"{scale}\" height=\"{scale}\" fill=\"#000\"/>"
            ));
        }
    }
    svg.push_str("</svg>");
    Some(svg)
}

/// Split a UTF-8 string into chunks no larger than `max` bytes, keeping
/// multibyte characters intact (chunk boundary is a char boundary).
fn chunk_data(text: &str, max: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        if !cur.is_empty() && cur.len() + ch.len_utf8() > max {
            out.push(std::mem::take(&mut cur));
        }
        cur.push(ch);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(body: &str, ts: i64) -> crate::log::LogMsg {
        crate::log::LogMsg::from_parcel(body.to_string(), ts, "@o:server".into())
    }

    #[test]
    fn round_trip() {
        let msgs = vec![
            msg("KT {\"uid\":\"A\"}", 1),
            msg("khanatime_setup:{...}", 2),
        ];
        let text = pack_parcel("ev-uid", &msgs);
        assert!(text.starts_with(PARCEL_PREFIX));
        let parcel = unpack_parcel(&text).expect("parses");
        assert_eq!(parcel.v, 1);
        assert_eq!(parcel.event_uid, "ev-uid");
        assert_eq!(parcel.msgs.len(), 2);
        assert_eq!(parcel.msgs[0].body, "KT {\"uid\":\"A\"}");
        assert_eq!(parcel.msgs[0].ts, 1);
    }

    #[test]
    fn missing_prefix_is_rejected() {
        assert!(unpack_parcel("{\"v\":1,\"event_uid\":\"e\"}").is_err());
    }

    #[test]
    fn corrupt_json_is_rejected() {
        assert!(unpack_parcel(&format!("{PARCEL_PREFIX}{{broken")).is_err());
    }

    #[test]
    fn wrong_version_is_rejected() {
        let text =
            format!("{PARCEL_PREFIX}{{\"v\":9,\"event_uid\":\"e\",\"created\":0,\"msgs\":[]}}");
        let err = unpack_parcel(&text).unwrap_err();
        assert!(err.contains("version"));
    }

    #[test]
    fn missing_event_uid_is_rejected() {
        let text =
            format!("{PARCEL_PREFIX}{{\"v\":1,\"event_uid\":\"\",\"created\":0,\"msgs\":[]}}");
        let err = unpack_parcel(&text).unwrap_err();
        assert!(err.contains("event_uid"));
    }

    #[test]
    fn parcel_body_round_trips_verbatim() {
        let body = "KT {\"r#type\":\"finish\",\"uid\":\"OBS1\",\"ts\":1,\"time_ds\":123}";
        let text = pack_parcel("e", &[msg(body, 99)]);
        let parcel = unpack_parcel(&text).unwrap();
        assert_eq!(parcel.msgs[0].body, body);
    }

    fn make_parcel(n: usize) -> String {
        // Build a parcel comfortably bigger than MAX_FRAME_DATA.
        let bodies: Vec<String> = (0..n)
            .map(|i| format!("KT {{\"uid\":\"{i}\",\"body\":\"{}\"}}", "x".repeat(80)))
            .collect();
        let msgs: Vec<crate::log::LogMsg> = bodies.iter().map(|b| msg(b, 1)).collect();
        pack_parcel("e", &msgs)
    }

    #[test]
    fn frames_single_for_small_parcel() {
        let text = pack_parcel("e", &[msg("hi", 1)]);
        let frames = pack_frames(&text);
        assert_eq!(frames.len(), 1);
        assert!(frames[0].starts_with(QR_PREFIX));
        let out = assemble_frames(
            &frames
                .iter()
                .map(|s| unpack_frame(s).unwrap())
                .collect::<Vec<_>>(),
        )
        .unwrap();
        assert_eq!(out, text);
    }

    #[test]
    fn frames_split_and_round_trip_out_of_order() {
        let text = make_parcel(60);
        let frames = pack_frames(&text);
        assert!(frames.len() > 1, "expected chunking, got {}", frames.len());
        let parsed: Vec<Frame> = frames.iter().map(|s| unpack_frame(s).unwrap()).collect();
        // All frames share one id, unique indices, consistent total.
        let ids: std::collections::HashSet<_> = parsed.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids.len(), 1);
        let idx: Vec<u32> = parsed.iter().map(|f| f.index).collect();
        assert_eq!(
            idx.len(),
            idx.iter()
                .cloned()
                .collect::<std::collections::HashSet<_>>()
                .len()
        );
        // Shuffle the order; assemble must still reconstruct the parcel.
        let mut shuffled = parsed.clone();
        shuffled.reverse();
        let out = assemble_frames(&shuffled).unwrap();
        assert_eq!(out, text);
    }

    #[test]
    fn frames_incomplete_rejected() {
        let text = make_parcel(60);
        let parsed: Vec<Frame> = pack_frames(&text)
            .iter()
            .map(|s| unpack_frame(s).unwrap())
            .collect();
        let partial: Vec<Frame> = parsed[..parsed.len() - 1].to_vec();
        assert!(assemble_frames(&partial).is_err());
        // Mixed ids rejected too.
        let mut mixed = partial.clone();
        mixed[0].id = "other".to_string();
        assert!(assemble_frames(&mixed).is_err());
    }

    #[test]
    fn unpack_frame_rejects_bad_input() {
        assert!(unpack_frame("{\"v\":1}").is_err()); // no prefix
        let bad =
            format!("{QR_PREFIX}{{\"v\":9,\"id\":\"a\",\"total\":1,\"index\":0,\"data\":\"\"}}");
        assert!(unpack_frame(&bad).is_err()); // wrong version
        let badidx =
            format!("{QR_PREFIX}{{\"v\":1,\"id\":\"a\",\"total\":2,\"index\":5,\"data\":\"\"}}");
        assert!(unpack_frame(&badidx).is_err()); // index >= total
    }

    #[test]
    fn chunk_keeps_multibyte_chars_intact() {
        let text = "héllo→wörld😀".repeat(500);
        let chunks = chunk_data(&text, 300);
        let joined: String = chunks.concat();
        assert_eq!(joined, text);
        assert!(chunks.iter().all(|c| c.len() <= 300));
    }

    #[test]
    fn svg_renders_valid_svg() {
        let svg =
            qr_svg("khanatime_parcel:{\"v\":1,\"event_uid\":\"e\",\"created\":0,\"msgs\":[]}")
                .expect("svg renders");
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("<rect"));
    }

    #[test]
    fn svg_none_for_oversized_data() {
        let huge = "x".repeat(100_000);
        assert!(qr_svg(&huge).is_none());
    }
}
