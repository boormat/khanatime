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

fn expand_all(chat: Model) {
    let ids: HashSet<String> = chat
        .feed
        .with(|v| v.iter().map(|e| e.mid.clone()).collect());
    chat.expanded.set(ids);
}

fn fold_all(chat: Model) {
    chat.expanded.set(HashSet::new());
}

fn toggle_expand(chat: Model, mid: String) {
    chat.expanded.update(|s| {
        if !s.insert(mid.clone()) {
            s.remove(&mid);
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
            p(class="help") {
                "Transaction log of the event's timing room — every message the server still holds, oldest first. Click a line to pretty-print its raw JSON."
            }
            div(class="field is-grouped") {
                div(class="control") {
                    button(class="button is-small", on:click=move |_| expand_all(chat), title="Expand all") { "++" }
                }
                div(class="control") {
                    button(class="button is-small", on:click=move |_| fold_all(chat), title="Fold all") { "--" }
                }
                div(class="control") {
                    (move || {
                        let label = if chat.reverse.get() { "Newest first" } else { "Oldest first" };
                        view! {
                            button(class="button is-small", on:click=move |_| toggle_reverse(chat), title="Toggle order") {
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
                                let mid = e.mid.clone();
                                let summary = line_summary(e);
                                let head = view! {
                                    div(
                                        class="kt-log-line",
                                        on:click=move |_| toggle_expand(chat, mid.clone()),
                                    ) {
                                        (summary)
                                    }
                                };
                                if expanded.contains(&e.mid) {
                                    let pretty = pretty_json(&e.raw);
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
        format!(
            "[KT {} test={} car={} run={}]",
            t.r#type, t.test, t.car, t.run
        )
    } else if e.body.starts_with(TimingEvent::SETUP_PREFIX) {
        "[setup]".to_string()
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

fn fmt_ts(ms: i64) -> String {
    let d = js_sys::Date::new(&js_sys::Number::from(ms as f64).into());
    format!(
        "{:02}:{:02}:{:02}",
        d.get_hours(),
        d.get_minutes(),
        d.get_seconds()
    )
}
