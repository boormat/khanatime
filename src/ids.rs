//! Short ids + transport-independent content ids.
//!
//! Phase 1 of the multi-transport plan (see `docs/plan/identity-amendments.md`):
//! every event and every timing observation gets a generated short id (the
//! `uid`), and every message body resolves to one content id so merging is
//! idempotent across transports (room, relay, QR parcel).

/// Crocker base32, no `I/L/O/U` and no `0/1`: unambiguous when read off a
/// phone screen or QR.  10 chars ~= 50 bits.
const CHARSET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

const ID_LEN: usize = 10;

/// A 10-char Crocker base32 id (~50 bits).
///
/// wasm: `Math.random()` per char.  native: a xorshift64* PRNG seeded from
/// wall-clock nanoseconds so a batch of calls doesn't repeat.
pub fn gen_short_id() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        let mut out = String::with_capacity(ID_LEN);
        for _ in 0..ID_LEN {
            let idx = (js_sys::Math::random() * CHARSET.len() as f64) as usize % CHARSET.len();
            out.push(CHARSET[idx] as char);
        }
        out
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
            ^ COUNTER
                .fetch_add(1, Ordering::Relaxed)
                .wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let mut state = if seed == 0 {
            0x1234_5678_9ABC_DEF0
        } else {
            seed
        };
        let mut out = String::with_capacity(ID_LEN);
        for _ in 0..ID_LEN {
            // xorshift64*
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            let idx = (state.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32) as usize % CHARSET.len();
            out.push(CHARSET[idx] as char);
        }
        out
    }
}

/// Transport-independent dedup key for a message body.
///
/// Timing messages (`KT {json}`) carry their observation `uid`, which is the
/// content id — the same observation delivered by room, relay and QR parcel
/// collapses to one id.  Everything else (setup/entry manifests, chat) is
/// FNV-1a-hashed to a stable 16-hex id.
pub fn content_id(body: &str) -> String {
    if let Some(json) = body.strip_prefix("KT ") {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(json) {
            if let Some(uid) = value.get("uid").and_then(|v| v.as_str()) {
                if !uid.is_empty() {
                    return uid.to_string();
                }
            }
        }
    }
    // FNV-1a 64-bit, hex-encoded (16 chars).
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in body.bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x1000_0000_01B3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn id_charset_and_length() {
        let id = gen_short_id();
        assert_eq!(id.len(), ID_LEN);
        assert!(id.bytes().all(|b| CHARSET.contains(&b)));
    }

    #[test]
    fn batch_has_no_collisions() {
        let mut seen = HashSet::new();
        for _ in 0..10_000 {
            assert!(seen.insert(gen_short_id()));
        }
    }

    #[test]
    fn content_id_of_timing_is_its_uid() {
        let uid = gen_short_id();
        let body = format!("KT {{\"r#type\":\"finish\",\"uid\":\"{uid}\"}}");
        assert_eq!(content_id(&body), uid);
    }

    #[test]
    fn content_id_stable_for_other_bodies() {
        assert_eq!(
            content_id("khanatime_setup:{...}"),
            content_id("khanatime_setup:{...}")
        );
        assert_eq!(content_id("hello"), content_id("hello"));
        assert_ne!(content_id("hello"), content_id("hellp"));
    }
}
