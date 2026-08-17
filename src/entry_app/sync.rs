use super::types::Entry;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct EntryAppEntryMsg {
    pub event_id: String,
    pub ts: i64,
    pub entry: Entry,
    #[serde(default)]
    pub delete: bool,
}

pub const WIRE_PREFIX: &str = "entryapp_entry";

pub fn entry_body(event_uid: &str, entry: &Entry, delete: bool) -> String {
    let msg = EntryAppEntryMsg {
        event_id: event_uid.to_string(),
        ts: crate::log::now_ms(),
        entry: entry.clone(),
        delete,
    };
    format!(
        "{WIRE_PREFIX}:{}",
        serde_json::to_string(&msg).unwrap_or_default()
    )
}

pub fn enqueue_entry(model: crate::Model, entry: &Entry, delete: bool) {
    let (id, uid) = model
        .entry_app
        .event
        .with(|e| (e.id.clone(), e.uid.clone()));
    if id.is_empty() || uid.is_empty() {
        return;
    }
    let body = entry_body(&uid, entry, delete);
    let sender = model.sync.identity.get_clone();
    crate::log::enqueue_pending(&id, crate::log::LogMsg::new_pending(body, sender));
    model.entry_app.event.update(|e| {
        if delete {
            e.remove_entry(entry.entry_no);
        } else {
            e.upsert_entry(entry.clone());
        }
    });
    crate::sync::flush_pending_entry_app(model);
    crate::app::refresh_entry_app(model);
}

/// Parse a wire-format entry app message body.  Returns `(Entry, delete)`.
pub fn parse_entry_body(body: &str) -> Option<(Entry, bool)> {
    let json = body.strip_prefix(&format!("{WIRE_PREFIX}:"))?;
    let msg: EntryAppEntryMsg = serde_json::from_str(json).ok()?;
    Some((msg.entry, msg.delete))
}
