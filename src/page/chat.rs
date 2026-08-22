use std::collections::HashSet;

use sycamore::prelude::*;

use crate::timing_event::TimingEvent;

// Chat: transaction-log viewer for the current event's timing room.  The log
// (backfill + live) plus any unsent pending messages land in `feed`; the room
// is merged into local state elsewhere.  No send box — this is a diagnostics
// viewer: one line per message, click a line to pretty-print its raw JSON.

#[derive(Clone)]
pub struct FeedEntry {
    /// Matrix event id, used to dedupe across live sync + backfill.
    pub mid: String,
    /// Client-generated id for pending (unsent) messages; empty for room ones.
    pub local_id: String,
    pub ts: i64,
    pub sender: String,
    pub body: String,
    pub timing: Option<TimingEvent>,
    /// Full raw `m.room.message` event JSON.
    pub raw: String,
    /// True while this message is still unsent (local outbox).
    pub pending: bool,
}

impl From<&crate::log::LogMsg> for FeedEntry {
    fn from(m: &crate::log::LogMsg) -> Self {
        Self {
            mid: m.mid.clone(),
            local_id: m.local_id.clone(),
            ts: m.ts,
            sender: m.sender.clone(),
            body: m.body.clone(),
            timing: TimingEvent::from_body(&m.body),
            raw: m.raw.clone(),
            pending: m.pending,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Model {
    pub feed: Signal<Vec<FeedEntry>>,
    /// Event ids of lines currently expanded to show pretty JSON.
    pub expanded: Signal<HashSet<String>>,
    /// False = chronological (oldest first, the default); true = newest first.
    pub reverse: Signal<bool>,
}

pub fn init() -> Model {
    Model {
        feed: create_signal(Vec::new()),
        expanded: create_signal(HashSet::new()),
        reverse: create_signal(false),
    }
}

/// Per-line expansion key: pending messages have no Matrix id yet, so their
/// client-generated local id stands in (all pending lines share `mid == ""`).
fn line_key(e: &FeedEntry) -> String {
    if !e.local_id.is_empty() {
        e.local_id.clone()
    } else {
        e.mid.clone()
    }
}

fn expand_all(chat: Model) {
    let ids: HashSet<String> = chat.feed.with(|v| v.iter().map(line_key).collect());
    chat.expanded.set(ids);
}

fn fold_all(chat: Model) {
    chat.expanded.set(HashSet::new());
}

fn toggle_expand(chat: Model, key: String) {
    chat.expanded.update(|s| {
        if !s.insert(key.clone()) {
            s.remove(&key);
        }
    });
}

fn toggle_reverse(chat: Model) {
    chat.reverse.set(!chat.reverse.get());
}

pub fn view(model: crate::Model) -> View {
    let chat = model.screens.chat;
    view! {
        div {
            h1(class="title") { "Chat" }
            (crate::page::home::view_comms(model))
            p(class="help") {
                "Transaction log of the event's timing room — every message the server still holds, oldest first. Click a line to pretty-print its raw JSON."
            }
            div(class="field is-grouped") {
                div(class="control") {
                    button(class="button", on:click=move |_| expand_all(chat), title="Expand all") { "++" }
                }
                div(class="control") {
                    button(class="button", on:click=move |_| fold_all(chat), title="Fold all") { "--" }
                }
                div(class="control") {
                    (move || {
                        let label = if chat.reverse.get() { "Newest first" } else { "Oldest first" };
                        view! {
                            button(class="button", on:click=move |_| toggle_reverse(chat), title="Toggle order") {
                                (label)
                            }
                        }
                    })
                }
                div(class="control") {
                    (move || {
                        let n = chat.feed.with(|v| v.len());
                        view! { span(class="tag is-light is-pulled-right") { (format!("{n} messages")) } }
                    })
                }
            }
            div(class="box is-paddingless") {
                div(class="kt-log") {
                    (move || {
                        let reverse = chat.reverse.get();
                        let expanded = chat.expanded.get_clone();
                        let mut entries = chat.feed.get_clone();
                        entries.sort_by(|a, b| {
                            a.ts.cmp(&b.ts).then_with(|| a.mid.cmp(&b.mid))
                        });
                        if reverse {
                            entries.reverse();
                        }
                        if entries.is_empty() {
                            return view! {
                                p(class="help") {
                                    "No messages yet. Connect and open an event to receive the room log."
                                }
                            };
                        }
                        let lines: Vec<View> = entries
                            .iter()
                            .map(|e| {
                                let key = line_key(e);
                                let click_key = key.clone();
                                let summary = line_summary(e);
                                let head = view! {
                                    div(
                                        class="kt-log-line",
                                        on:click=move |_| toggle_expand(chat, click_key.clone()),
                                    ) {
                                        (summary)
                                    }
                                };
                                if expanded.contains(&key) {
                                    // Pending messages carry no server raw JSON
                                    // yet — show the wire body instead.
                                    let pretty = if e.pending {
                                        pretty_body(&e.body)
                                    } else {
                                        pretty_json(&e.raw)
                                    };
                                    view! {
                                        div {
                                            (head)
                                            pre(class="kt-log-json") { (pretty) }
                                        }
                                    }
                                } else {
                                    head
                                }
                            })
                            .collect();
                        lines.into()
                    })
                }
            }
        }
    }
}

/// Compact one-line summary for a feed entry.
fn line_summary(e: &FeedEntry) -> String {
    let summary = if let Some(t) = &e.timing {
        let time = t
            .time_ds
            .map(|ds| format!(" {:.1}s", ds as f32 / 10.0))
            .unwrap_or_default();
        let flags = t
            .flags
            .filter(|&f| f > 0)
            .map(|f| format!(" {f}F"))
            .unwrap_or_default();
        let status = t
            .status
            .as_deref()
            .filter(|&s| s != "ok")
            .map(|s| format!(" {s}"))
            .unwrap_or_default();
        format!(
            "[{} test={} car={}{time}{flags}{status}]",
            t.r#type, t.test, t.car
        )
    } else if e.body.starts_with(TimingEvent::SETUP_PREFIX) {
        let payload = &e.body[TimingEvent::SETUP_PREFIX.len()..];
        let name = serde_json::from_str::<serde_json::Value>(payload)
            .ok()
            .and_then(|v| v.get("name").and_then(|n| n.as_str()).map(String::from))
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "unnamed".to_string());
        format!("[setup: {name}]")
    } else if e.body.starts_with(TimingEvent::RESULT_PREFIX) {
        "[result]".to_string()
    } else {
        e.body.clone()
    };
    let tag = if e.pending { " ↺ pending" } else { "" };
    format!("{}{}  {}  {}", fmt_ts(e.ts), tag, e.sender, summary)
}

/// Raw event JSON, pretty-printed.
fn pretty_json(raw: &str) -> String {
    serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .and_then(|v| serde_json::to_string_pretty(&v).ok())
        .unwrap_or_else(|| raw.to_string())
}

/// Wire body of a pending (unsent) message, pretty-printed.  Strips the known
/// `KT `/`khanatime_*:` prefix and pretty-prints the JSON payload, falling
/// back to the body text when it doesn't parse.
fn pretty_body(body: &str) -> String {
    let prefix = ["KT ", TimingEvent::SETUP_PREFIX, TimingEvent::RESULT_PREFIX]
        .into_iter()
        .find(|p| body.starts_with(p));
    let json = prefix.map(|p| &body[p.len()..]).unwrap_or(body);
    pretty_json(json)
}

fn fmt_ts(ms: i64) -> String {
    let d = js_sys::Date::new(&js_sys::Number::from(ms as f64).into());
    format!(
        "{:02}:{:02}:{:02}",
        d.get_hours(),
        d.get_minutes(),
        d.get_seconds()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(local_id: &str, mid: &str, pending: bool) -> FeedEntry {
        FeedEntry {
            mid: mid.into(),
            local_id: local_id.into(),
            ts: 0,
            sender: String::new(),
            body: String::new(),
            timing: None,
            raw: String::new(),
            pending,
        }
    }

    #[test]
    fn pretty_body_strips_timing_prefix() {
        let body = "KT {\"r#type\":\"finish\",\"uid\":\"OBS1\"}";
        let out = pretty_body(body);
        assert!(out.contains("\"r#type\": \"finish\""), "{out}");
        assert!(out.contains("\"uid\": \"OBS1\""), "{out}");
        assert!(!out.contains("KT "), "{out}");
    }

    #[test]
    fn pretty_body_strips_khanatime_prefixes() {
        for p in [TimingEvent::SETUP_PREFIX, TimingEvent::RESULT_PREFIX] {
            let out = pretty_body(&format!("{p}{{\"a\":1}}"));
            assert!(out.contains("\"a\": 1"), "prefix {p}: {out}");
        }
    }

    #[test]
    fn pretty_body_falls_back_to_text() {
        assert_eq!(pretty_body("not json"), "not json");
    }

    #[test]
    fn line_key_uses_local_id_when_pending() {
        let e = entry("l1", "", true);
        assert_eq!(line_key(&e), "l1");
    }

    #[test]
    fn line_key_uses_mid_for_room_messages() {
        let e = entry("", "!mid", false);
        assert_eq!(line_key(&e), "!mid");
    }
}
