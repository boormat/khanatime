//! Per-event message log storage.
//!
//! The only durable per-event state is a message log: everything received from
//! the event's timing room (`log:<id>`) plus locally created messages that
//! haven't been acknowledged by the room yet (`pending:<id>`, the outbox).
//! Convenient in-memory state (event, scores, runs) is reconstructed by
//! replaying these two lists (see `replay.rs`).
//!
//! `pending` entries carry a client-generated `local_id`; when a send succeeds
//! the entry is promoted into the log with its real Matrix event id.  The
//! sender's own message also comes back through sync — the log dedupes by
//! `mid`, and `reconcile` drops pending copies whose body already landed in the
//! log (lost-ack / echo safety).

use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

const LOG_PREFIX: &str = "log:";
const PENDING_PREFIX: &str = "pending:";

static SEQ: AtomicU64 = AtomicU64::new(0);

/// Current time in ms since the epoch (native + wasm).
pub fn now_ms() -> i64 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now() as i64
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }
}

/// A client-unique id for pending messages (created timestamp + counter).
pub fn next_local_id() -> String {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    format!("{}-{n}", now_ms())
}

/// One message in an event's log or pending list.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct LogMsg {
    /// Matrix event id; empty while the message is still pending.
    pub mid: String,
    /// Client-generated id for pending messages (empty for room messages).
    pub local_id: String,
    /// origin_server_ts for room messages, local create ts for pending.
    pub ts: i64,
    /// Room sender (or local identity) of the message.
    pub sender: String,
    /// The `m.text` body (`khanatime_setup:` / `khanatime_result:` / `KT {json}`).
    pub body: String,
    /// Full raw `m.room.message` JSON (room messages only; feed pretty-print).
    pub raw: String,
    /// True while this entry is unsent (in the outbox).
    pub pending: bool,
}

impl LogMsg {
    /// A locally created, not-yet-sent message.
    pub fn new_pending(body: String, sender: String) -> Self {
        Self {
            mid: String::new(),
            local_id: next_local_id(),
            ts: now_ms(),
            sender,
            body,
            raw: String::new(),
            pending: true,
        }
    }

    /// A message received from the room.
    pub fn from_room(mid: String, ts: i64, sender: String, body: String, raw: String) -> Self {
        Self {
            mid,
            local_id: String::new(),
            ts,
            sender,
            body,
            raw,
            pending: false,
        }
    }
}

fn log_key(id: &str) -> String {
    format!("{LOG_PREFIX}{id}")
}

fn pending_key(id: &str) -> String {
    format!("{PENDING_PREFIX}{id}")
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

fn get_json<T: serde::de::DeserializeOwned>(key: &str) -> Option<T> {
    storage()?
        .get_item(key)
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
}

fn set_json<T: Serialize>(key: &str, value: &T) {
    if let Some(st) = storage() {
        let _ = st.set_item(key, &serde_json::to_string(value).unwrap());
    }
}

pub fn load_log(id: &str) -> Vec<LogMsg> {
    if id.is_empty() {
        return vec![];
    }
    get_json(&log_key(id)).unwrap_or_default()
}

pub fn load_pending(id: &str) -> Vec<LogMsg> {
    if id.is_empty() {
        return vec![];
    }
    get_json(&pending_key(id)).unwrap_or_default()
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // wasm sync sink
fn save_log(id: &str, msgs: &[LogMsg]) {
    if !id.is_empty() {
        set_json(&log_key(id), &msgs.to_vec());
    }
}

fn save_pending(id: &str, msgs: &[LogMsg]) {
    if !id.is_empty() {
        set_json(&pending_key(id), &msgs.to_vec());
    }
}

/// Enqueue a local message for later broadcast.  Persists immediately.
pub fn enqueue_pending(id: &str, msg: LogMsg) {
    if id.is_empty() {
        return;
    }
    let mut pending = load_pending(id);
    pending.push(msg);
    save_pending(id, &pending);
}

/// Append a received room message to the log, skipping a duplicate `mid` or a
/// message whose body resolves to an already-logged content id (the same
/// observation arriving via room, relay or QR collapses to one entry).
/// Returns true when the message was new.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // wasm sync sink
pub fn append_log(id: &str, msg: LogMsg) -> bool {
    if id.is_empty() || msg.mid.is_empty() {
        return false;
    }
    let mut log = load_log(id);
    if dedup_by_mid(&log, &msg.mid) {
        return false;
    }
    let cid = crate::ids::content_id(&msg.body);
    if log.iter().any(|m| crate::ids::content_id(&m.body) == cid) {
        return false;
    }
    log.push(msg);
    save_log(id, &log);
    true
}

/// True when `mid` already appears in `log`.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // wasm sync sink + tests
pub fn dedup_by_mid(log: &[LogMsg], mid: &str) -> bool {
    log.iter().any(|m| !m.mid.is_empty() && m.mid == mid)
}

/// Promote a sent pending message into the log with its real Matrix event id.
/// Returns true when a pending entry was found and moved.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // wasm sync sink
pub fn promote(id: &str, local_id: &str, mid: &str) -> bool {
    if id.is_empty() || mid.is_empty() {
        return false;
    }
    let mut pending = load_pending(id);
    let Some(idx) = pending.iter().position(|m| m.local_id == local_id) else {
        return false;
    };
    let mut msg = pending.remove(idx);
    msg.mid = mid.to_string();
    msg.pending = false;
    let mut log = load_log(id);
    if !dedup_by_mid(&log, &msg.mid) {
        log.push(msg);
        save_log(id, &log);
    }
    save_pending(id, &pending);
    true
}

/// Indexes of pending entries whose body (or content id) already appears in
/// the log (echoes that arrived via sync while the send-ack was lost).
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // wasm sync sink + tests
pub fn stale_pending(log: &[LogMsg], pending: &[LogMsg]) -> Vec<usize> {
    pending
        .iter()
        .enumerate()
        .filter(|(_, p)| {
            let cid = crate::ids::content_id(&p.body);
            log.iter()
                .any(|l| l.body == p.body || crate::ids::content_id(&l.body) == cid)
        })
        .map(|(i, _)| i)
        .collect()
}

/// Drop pending entries that already exist in the log (by body).  Returns the
/// number removed.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // wasm sync sink
pub fn reconcile(id: &str) -> usize {
    if id.is_empty() {
        return 0;
    }
    let log = load_log(id);
    let mut pending = load_pending(id);
    let stale = stale_pending(&log, &pending);
    let n = stale.len();
    for i in stale.into_iter().rev() {
        pending.remove(i);
    }
    save_pending(id, &pending);
    n
}

/// All event ids that have a log or pending entry.
pub fn list_event_ids() -> HashSet<String> {
    let mut out: HashSet<String> = Default::default();
    let Some(st) = storage() else {
        return out;
    };
    if let Ok(len) = st.length() {
        (0..len).for_each(|i| {
            if let Ok(Some(key)) = st.key(i) {
                if let Some(rest) = key
                    .strip_prefix(LOG_PREFIX)
                    .or_else(|| key.strip_prefix(PENDING_PREFIX))
                {
                    out.insert(rest.to_string());
                }
            }
        });
    }
    out
}

/// Remove all stored messages for an event.
pub fn remove_event_log(id: &str) {
    if id.is_empty() {
        return;
    }
    let Some(st) = storage() else {
        return;
    };
    let _ = st.remove_item(&log_key(id));
    let _ = st.remove_item(&pending_key(id));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(body: &str, ts: i64) -> LogMsg {
        LogMsg::new_pending(body.to_string(), String::new()).with_ts(ts)
    }

    impl LogMsg {
        fn with_ts(mut self, ts: i64) -> Self {
            self.ts = ts;
            self
        }
        fn with_mid(mut self, mid: &str) -> Self {
            self.mid = mid.to_string();
            self
        }
    }

    #[test]
    fn dedup_by_mid_ignores_empty() {
        let log = vec![msg("a", 1).with_mid("!1")];
        assert!(dedup_by_mid(&log, "!1"));
        assert!(!dedup_by_mid(&log, ""));
        assert!(!dedup_by_mid(&log, "!2"));
    }

    #[test]
    fn stale_pending_matches_by_body() {
        let log = vec![msg("KT {\"r#type\":\"finish\"}", 100).with_mid("!1")];
        let pending = vec![msg("KT {\"r#type\":\"finish\"}", 99), msg("other", 5)];
        let stale = stale_pending(&log, &pending);
        assert_eq!(stale, vec![0]);
    }

    #[test]
    fn stale_pending_matches_by_content_id() {
        // Same observation uid, different serialization -> content-id match.
        let log =
            vec![msg("KT {\"r#type\":\"finish\",\"uid\":\"OBS1\",\"ts\":1}", 100).with_mid("!1")];
        let pending = vec![msg(
            "KT {\"ts\":1,\"uid\":\"OBS1\",\"r#type\":\"finish\"}",
            99,
        )];
        let stale = stale_pending(&log, &pending);
        assert_eq!(stale, vec![0]);
    }

    #[test]
    fn next_local_id_is_unique() {
        let a = next_local_id();
        let b = next_local_id();
        assert_ne!(a, b);
    }
}
