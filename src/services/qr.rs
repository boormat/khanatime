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

use serde::{Deserialize, Serialize};

pub const PARCEL_PREFIX: &str = "khanatime_parcel:";

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
}
