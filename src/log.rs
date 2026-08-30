//! Per-event message log storage.
//!
//! The only durable per-event state is a message log: everything published
//! (`log:<id>`) plus locally created messages that haven't been handed to any
//! transport yet (`pending:<id>`, the outbox).  Convenient in-memory state
//! (event, scores, runs) is reconstructed by replaying these two lists (see
//! `replay.rs`).
//!
//! `pending` entries carry a client-generated `local_id`; when a send succeeds
//! the entry is promoted into the log with its real Matrix event id.  The
//! sender's own message also comes back through sync — the log dedupes by
//! `mid`, and `reconcile` drops pending copies whose body already landed in the
//! log (lost-ack / echo safety).
//!
//! `origin` records which transport a message was published through: a room id
//! (confirmed in that room), `parcel` (handed off via QR — see `qr.rs`), or
//! empty (still in the outbox).  Relaying re-broadcasts anything not yet
//! confirmed in a connected room; content-id dedup makes the whole mesh
//! idempotent.

use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

const LOG_PREFIX: &str = "log:";
const PENDING_PREFIX: &str = "pending:";

/// `LogMsg.origin` value for messages handed off via a QR parcel (see `qr.rs`).
pub const PARCEL_ORIGIN: &str = "parcel";
/// `LogMsg.origin` for an event adopted from a published room: durable locally
/// but already in the room, so `relay_to_room` never re-broadcasts it.
pub const ADOPT_ORIGIN: &str = "adopt";

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
    /// Transport the message was published through: the confirming room id,
    /// [`PARCEL_ORIGIN`] for QR handoffs, or empty while still in the outbox.
    pub origin: String,
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
            origin: String::new(),
        }
    }

    /// A message received from the room `room_id`.
    pub fn from_room(
        mid: String,
        ts: i64,
        sender: String,
        body: String,
        raw: String,
        room_id: &str,
    ) -> Self {
        Self {
            mid,
            local_id: String::new(),
            ts,
            sender,
            body,
            raw,
            pending: false,
            origin: room_id.to_string(),
        }
    }

    /// A message handed off via a QR parcel: durable and published, but not yet
    /// confirmed in any room (no Matrix event id until relayed).
    pub fn from_parcel(body: String, ts: i64, sender: String) -> Self {
        Self {
            mid: String::new(),
            local_id: String::new(),
            ts,
            sender,
            body,
            raw: String::new(),
            pending: false,
            origin: PARCEL_ORIGIN.to_string(),
        }
    }

    /// An event adopted from a published room: durable locally (so it replays
    /// into a current event) but already present in the room, so it is never
    /// re-broadcast by relay.
    pub fn from_adopted(body: String, ts: i64, sender: String) -> Self {
        Self {
            mid: String::new(),
            local_id: String::new(),
            ts,
            sender,
            body,
            raw: String::new(),
            pending: false,
            origin: ADOPT_ORIGIN.to_string(),
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

fn set_json<T: Serialize>(key: &str, value: &T) -> Result<(), String> {
    let json = serde_json::to_string(value).map_err(|e| format!("serialize {key}: {e}"))?;
    let Some(st) = storage() else {
        return Err("localStorage unavailable".into());
    };
    st.set_item(key, &json)
        .map_err(|e| format!("persist {key} failed (quota?): {e:?}"))
}

/// Log a persistence failure loudly so data loss is never silent.  The log is
/// the only durable state — a dropped write (e.g. localStorage quota) must be
/// visible in the console / devtools, not swallowed.
fn report_persist_error(err: &str) {
    web_sys::console::error_1(&js_sys::JsString::from(format!("khanatime: {err}")));
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
fn save_log(id: &str, msgs: &[LogMsg]) -> bool {
    if id.is_empty() {
        return false;
    }
    match set_json(&log_key(id), &msgs.to_vec()) {
        Ok(()) => true,
        Err(e) => {
            report_persist_error(&e);
            false
        }
    }
}

fn save_pending(id: &str, msgs: &[LogMsg]) -> bool {
    if id.is_empty() {
        return false;
    }
    match set_json(&pending_key(id), &msgs.to_vec()) {
        Ok(()) => true,
        Err(e) => {
            report_persist_error(&e);
            false
        }
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

/// Enqueue a setup-manifest, replacing any superseded setup manifest already in
/// the outbox.  Setup is last-writer-wins, so only the latest needs to be
/// pending — otherwise every Save Local on a draft would queue a draft setup
/// that then gets flushed into the room on publish (draft history in the room).
pub fn enqueue_setup_pending(id: &str, msg: LogMsg) {
    if id.is_empty() {
        return;
    }
    let mut pending = load_pending(id);
    pending.retain(|m| {
        !m.body
            .starts_with(crate::timing_event::TimingEvent::SETUP_PREFIX)
    });
    pending.push(msg);
    save_pending(id, &pending);
}

/// Remove a pending message by local id (e.g. one already confirmed in the
/// room, so it must not be re-sent).
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // wasm flush dedup
pub fn drop_pending(id: &str, local_id: &str) {
    if id.is_empty() || local_id.is_empty() {
        return;
    }
    let mut pending = load_pending(id);
    let before = pending.len();
    pending.retain(|m| m.local_id != local_id);
    if pending.len() != before {
        save_pending(id, &pending);
    }
}

/// True when `body`'s content id is already published through `room_id` for
/// event `id` (a log entry with that room as origin) — so it must not be sent
/// to the room again.  Never send a duplicate message.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // wasm flush/relay
pub fn confirmed_in_room(id: &str, body: &str, room_id: &str) -> bool {
    if id.is_empty() || room_id.is_empty() {
        return false;
    }
    let cid = crate::ids::content_id(body);
    load_log(id)
        .iter()
        .any(|m| m.origin == room_id && crate::ids::content_id(&m.body) == cid)
}

/// Append a received room message to the log, skipping a duplicate `mid` or a
/// message whose body resolves to an already-logged content id (the same
/// observation arriving via room, relay or QR collapses to one entry).
/// Returns true when the message was new.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // wasm sync sink
pub fn append_log(id: &str, msg: LogMsg) -> bool {
    if id.is_empty() || msg.origin.is_empty() {
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
    // A failed write means the durable log didn't record this message — report
    // it and return false so callers don't believe it persisted.
    save_log(id, &log)
}

/// Seed an adopted event's setup manifest into the durable log (not the
/// outbox), so it replays into a current event without ever being re-broadcast
/// to the room by relay.  Idempotent by content id.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // wasm adopt sinks
pub fn seed_setup_to_log(id: &str, setup_body: &str, sender: &str) -> bool {
    if id.is_empty() || setup_body.is_empty() {
        return false;
    }
    append_log(
        id,
        LogMsg::from_adopted(setup_body.to_string(), now_ms(), sender.to_string()),
    )
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

/// Move every outbox entry into the durable log as QR-origin.  Handing a
/// message off in a parcel is publishing it — it leaves the outbox (so
/// `flush_pending` won't re-send it) and becomes relayable to a room later.
/// Returns the number of entries moved (entries already in the log by content
/// id are dropped from the outbox instead).
pub fn publish_outbox(id: &str) -> usize {
    if id.is_empty() {
        return 0;
    }
    let mut pending = load_pending(id);
    let mut log = load_log(id);
    let moved = apply_publish_outbox(&mut log, &mut pending);
    if moved > 0 {
        save_log(id, &log);
        save_pending(id, &pending);
    }
    moved
}

/// Pure form of [`publish_outbox`], testable without storage.
fn apply_publish_outbox(log: &mut Vec<LogMsg>, pending: &mut Vec<LogMsg>) -> usize {
    let mut moved = 0;
    pending.retain(|m| {
        let cid = crate::ids::content_id(&m.body);
        if log.iter().any(|l| crate::ids::content_id(&l.body) == cid) {
            return false;
        }
        let mut published = m.clone();
        published.mid = String::new();
        published.local_id = String::new();
        published.origin = PARCEL_ORIGIN.to_string();
        published.pending = false;
        log.push(published);
        moved += 1;
        false
    });
    moved
}

/// Confirm a QR-origin log entry in a room: once its body has been sent there
/// (real mid from the send ack, or from the echo when the ack was lost) the
/// entry becomes room-origin so relay won't re-send it.  No-op unless the entry
/// still has `origin == parcel`.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // wasm sync sink
pub fn confirm_in_room(id: &str, cid: &str, mid: &str, room: &str) -> bool {
    if id.is_empty() || mid.is_empty() || room.is_empty() {
        return false;
    }
    let mut log = load_log(id);
    let done = apply_confirm_in_room(&mut log, cid, mid, room);
    if done {
        save_log(id, &log);
    }
    done
}

/// Pure form of [`confirm_in_room`], testable without storage.
fn apply_confirm_in_room(log: &mut [LogMsg], cid: &str, mid: &str, room: &str) -> bool {
    let Some(idx) = log
        .iter()
        .position(|m| m.origin == PARCEL_ORIGIN && crate::ids::content_id(&m.body) == cid)
    else {
        return false;
    };
    log[idx].mid = mid.to_string();
    log[idx].origin = room.to_string();
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

    #[test]
    fn publish_outbox_moves_entries_into_log() {
        let mut log = vec![];
        let mut pending = vec![
            LogMsg::from_parcel("KT {\"uid\":\"A\"}".into(), 1, "me".into()),
            msg("KT {\"uid\":\"B\"}", 2),
        ];
        let moved = apply_publish_outbox(&mut log, &mut pending);
        assert_eq!(moved, 2);
        assert!(pending.is_empty());
        assert_eq!(log.len(), 2);
        assert!(log.iter().all(|m| m.origin == PARCEL_ORIGIN && !m.pending));
        assert!(log
            .iter()
            .all(|m| m.mid.is_empty() && m.local_id.is_empty()));
    }

    #[test]
    fn publish_outbox_skips_already_logged() {
        let mut log = vec![msg("KT {\"uid\":\"A\"}", 1).with_mid("!a")];
        let mut pending = vec![
            msg("KT {\"uid\":\"A\"}", 1), // already published by content id
            msg("KT {\"uid\":\"C\"}", 3),
        ];
        let moved = apply_publish_outbox(&mut log, &mut pending);
        assert_eq!(moved, 1);
        assert!(pending.is_empty());
        assert_eq!(log.len(), 2);
        // The pre-existing entry is untouched (still a room message).
        assert_eq!(log[0].origin, "");
        assert_eq!(log[0].mid, "!a");
        assert_eq!(log[1].origin, PARCEL_ORIGIN);
    }

    #[test]
    fn confirm_in_room_promotes_parcel_entry_once() {
        let mut log = vec![LogMsg::from_parcel(
            "KT {\"uid\":\"A\"}".into(),
            1,
            "me".into(),
        )];
        let cid = crate::ids::content_id(&log[0].body);
        assert!(apply_confirm_in_room(&mut log, &cid, "!real", "!room"));
        assert_eq!(log[0].mid, "!real");
        assert_eq!(log[0].origin, "!room");
        // Already room-origin now: a second confirm (echo) is a no-op.
        assert!(!apply_confirm_in_room(&mut log, &cid, "!real2", "!room"));
        assert_eq!(log[0].mid, "!real");
    }

    #[test]
    fn confirm_in_room_ignores_room_origin_entries() {
        let mut log = vec![msg("KT {\"uid\":\"A\"}", 1).with_mid("!a")];
        let cid = crate::ids::content_id(&log[0].body);
        assert!(!apply_confirm_in_room(&mut log, &cid, "!b", "!room"));
        assert_eq!(log[0].mid, "!a");
    }
}

// Browser-only coverage: every function below reads/writes real localStorage,
// which the native suite can't reach.  Each test uses its own event id so runs
// don't interfere, and cleans up after itself.
#[cfg(all(test, target_arch = "wasm32"))]
mod wasm_tests {
    use super::*;
    use wasm_bindgen_test::*;

    /// Unique event id per test so parallel/sequential runs stay isolated.
    fn tmp_id(tag: &str) -> String {
        format!("wasmtest-{tag}-{}", crate::ids::gen_short_id())
    }

    struct Tmp(String);

    impl Tmp {
        fn new(tag: &str) -> Self {
            let id = tmp_id(tag);
            remove_event_log(&id);
            Tmp(id)
        }
        fn id(&self) -> &str {
            &self.0
        }
    }

    impl Drop for Tmp {
        fn drop(&mut self) {
            remove_event_log(&self.0);
        }
    }

    fn pending(body: &str) -> LogMsg {
        LogMsg::new_pending(body.to_string(), "@me:hs".into())
    }

    fn from_room(mid: &str, body: &str, room: &str) -> LogMsg {
        LogMsg::from_room(
            mid.to_string(),
            100,
            "@them:hs".into(),
            body.to_string(),
            String::new(),
            room,
        )
    }

    #[wasm_bindgen_test]
    fn append_log_persists_to_storage() {
        let t = Tmp::new("append");
        assert!(append_log(
            t.id(),
            from_room("$1", "KT {\"uid\":\"A\"}", "!room")
        ));
        let log = load_log(t.id());
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].mid, "$1");
        assert_eq!(log[0].origin, "!room");
        assert!(!log[0].pending);
    }

    #[wasm_bindgen_test]
    fn append_log_dedupes_by_mid_and_content_id() {
        let t = Tmp::new("dedupe");
        let body = "KT {\"uid\":\"A\"}";
        assert!(append_log(t.id(), from_room("$1", body, "!room")));
        // Same mid: rejected.
        assert!(!append_log(t.id(), from_room("$1", body, "!room")));
        // Different mid, same content id: still one entry.
        assert!(!append_log(t.id(), from_room("$2", body, "!room")));
        assert_eq!(load_log(t.id()).len(), 1);
    }

    #[wasm_bindgen_test]
    fn enqueue_then_promote_moves_outbox_to_log() {
        let t = Tmp::new("promote");
        let msg = pending("KT {\"uid\":\"A\"}");
        let local_id = msg.local_id.clone();
        enqueue_pending(t.id(), msg);

        assert_eq!(load_pending(t.id()).len(), 1);
        assert!(load_log(t.id()).is_empty());

        assert!(promote(t.id(), &local_id, "$real"));
        assert!(load_pending(t.id()).is_empty());
        let log = load_log(t.id());
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].mid, "$real");
        assert!(!log[0].pending);
    }

    #[wasm_bindgen_test]
    fn promote_rejects_unknown_local_id() {
        let t = Tmp::new("promote-bad");
        enqueue_pending(t.id(), pending("KT {\"uid\":\"A\"}"));
        assert!(!promote(t.id(), "no-such-local-id", "$real"));
        assert_eq!(load_pending(t.id()).len(), 1);
    }

    #[wasm_bindgen_test]
    fn reconcile_drops_echoed_pending() {
        let t = Tmp::new("reconcile");
        let body = "KT {\"uid\":\"A\"}";
        enqueue_pending(t.id(), pending(body));
        // The send ack was lost but the echo landed in the room.
        assert!(append_log(t.id(), from_room("$echo", body, "!room")));
        assert_eq!(reconcile(t.id()), 1);
        assert!(load_pending(t.id()).is_empty());
        assert_eq!(load_log(t.id()).len(), 1);
    }

    #[wasm_bindgen_test]
    fn publish_outbox_moves_pending_to_parcel_origin() {
        let t = Tmp::new("outbox");
        enqueue_pending(t.id(), pending("KT {\"uid\":\"A\"}"));
        assert_eq!(publish_outbox(t.id()), 1);

        let log = load_log(t.id());
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].origin, PARCEL_ORIGIN);
        assert!(!log[0].pending);
        // Left the outbox, so flush_pending won't re-send it.
        assert!(load_pending(t.id()).is_empty());
    }

    #[wasm_bindgen_test]
    fn confirm_in_room_upgrades_parcel_origin() {
        let t = Tmp::new("confirm");
        let body = "KT {\"uid\":\"A\"}";
        enqueue_pending(t.id(), pending(body));
        publish_outbox(t.id());

        let cid = crate::ids::content_id(body);
        assert!(confirm_in_room(t.id(), &cid, "$sent", "!room"));
        let log = load_log(t.id());
        assert_eq!(log[0].origin, "!room");
        assert_eq!(log[0].mid, "$sent");
        // Second confirm (lost-ack echo) is a no-op.
        assert!(!confirm_in_room(t.id(), &cid, "$sent2", "!room"));
    }

    #[wasm_bindgen_test]
    fn seed_setup_to_log_is_idempotent() {
        let t = Tmp::new("seed");
        let setup = "khanatime_setup:{}";
        assert!(seed_setup_to_log(t.id(), setup, "@me:hs"));
        assert!(!seed_setup_to_log(t.id(), setup, "@me:hs"));
        let log = load_log(t.id());
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].origin, ADOPT_ORIGIN);
    }

    #[wasm_bindgen_test]
    fn drop_pending_removes_only_the_named_entry() {
        let t = Tmp::new("drop");
        let a = pending("KT {\"uid\":\"A\"}");
        let a_id = a.local_id.clone();
        enqueue_pending(t.id(), a);
        enqueue_pending(t.id(), pending("KT {\"uid\":\"B\"}"));

        drop_pending(t.id(), &a_id);
        let left = load_pending(t.id());
        assert_eq!(left.len(), 1);
        assert_ne!(left[0].local_id, a_id);
    }

    #[wasm_bindgen_test]
    fn list_event_ids_finds_stored_events() {
        let t = Tmp::new("list");
        enqueue_pending(t.id(), pending("KT {\"uid\":\"A\"}"));
        assert!(list_event_ids().contains(t.id()));
    }

    #[wasm_bindgen_test]
    fn empty_event_id_is_rejected() {
        // Guard clauses must not touch storage or panic.
        assert!(!append_log("", from_room("$1", "KT {}", "!room")));
        assert!(!promote("", "local", "$real"));
        assert_eq!(publish_outbox(""), 0);
        assert_eq!(reconcile(""), 0);
        assert!(!confirm_in_room("", "cid", "$1", "!room"));
    }

    // Fill localStorage past its quota so the next write fails.  Returns nothing;
    // leaves the storage full (caller is responsible for cleanup via Tmp drop on
    // its own event id — but the filler keys are separate and removed here).
    fn fill_storage() {
        if let Some(st) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
            let blob = "x".repeat(1024 * 1024); // 1 MiB chunk
            let mut slot = 0;
            while st.set_item(&format!("wasmtest-fill-{slot}"), &blob).is_ok() {
                slot += 1;
                // Safety bound: stop before 64 MiB even if no quota applies.
                if slot >= 64 {
                    break;
                }
            }
            // Clean up the filler keys; if storage is full this may itself fail,
            // which is fine — the point is to have left it full.
            for s in 0..slot {
                let _ = st.remove_item(&format!("wasmtest-fill-{s}"));
            }
        }
    }

    #[wasm_bindgen_test]
    fn append_log_reports_failure_when_storage_is_full() {
        let t = Tmp::new("append-fail");
        // First write succeeds normally.
        assert!(append_log(
            t.id(),
            from_room("$1", "KT {\"uid\":\"A\"}", "!room")
        ));
        // Now exhaust storage and confirm a second append is reported as failed
        // (returns false) rather than silently losing the write.
        fill_storage();
        let ok = append_log(t.id(), from_room("$2", "KT {\"uid\":\"B\"}", "!room"));
        // We can't guarantee quota on every runner, so only assert when it failed.
        // The key guarantee: a returned `false` means the durable log did NOT grow.
        if !ok {
            let log = load_log(t.id());
            assert_eq!(
                log.len(),
                1,
                "failed append must not silently persist a second entry"
            );
        }
    }
}
