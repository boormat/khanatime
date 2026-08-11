use sycamore::prelude::*;

use crate::timing_event::TimingEvent;

// Chat: read-only view of the current event's timing room.  Every message that
// arrives over the sync loop lands in `feed` (newest last); the room is merged
// into local state elsewhere.  No send box — this is a viewer.

#[derive(Clone)]
pub struct FeedEntry {
    pub ts: i64,
    pub sender: String,
    pub body: String,
    pub timing: Option<TimingEvent>,
}

#[derive(Clone, Copy)]
pub struct Model {
    pub feed: Signal<Vec<FeedEntry>>,
}

pub fn init() -> Model {
    Model {
        feed: create_signal(Vec::new()),
    }
}

pub fn view(model: crate::Model) -> View {
    view! {
        div {
            h1(class="title") { "Chat" }
            p(class="help") {
                "Read-only view of the event's timing room — chat and timing messages."
            }
            (move || {
                let entries = model.screens.chat.feed.get_clone();
                if entries.is_empty() {
                    return view! {
                        p(class="help") {
                            "No messages yet. Connect and open an event to receive live times."
                        }
                    };
                }
                let views: Vec<View> = entries
                    .iter()
                    .rev()
                    .map(|e| {
                        let line = feed_line(e);
                        view! {
                            div(class="box is-paddingless") {
                                pre { (line) }
                            }
                        }
                    })
                    .collect();
                views.into()
            })
        }
    }
}

/// Single feed line.
pub fn feed_line(e: &FeedEntry) -> String {
    let timing = e
        .timing
        .as_ref()
        .map(|t| {
            format!(
                "  [KT {} test={} car={} run={}]",
                t.r#type, t.test, t.car, t.run
            )
        })
        .unwrap_or_default();
    format!("{} {}: {}{}", e.sender, fmt_ts(e.ts), e.body, timing)
}

fn fmt_ts(ms: i64) -> String {
    let d = js_sys::Date::new(&js_sys::Number::from(ms as f64).into());
    d.to_string().into()
}
