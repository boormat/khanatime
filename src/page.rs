pub mod chat;
pub mod entries;
pub mod event;
pub mod events;
pub mod finish;
pub mod help;
pub mod home;
pub mod khana_rule;
pub mod pad;
pub mod penalty;
pub mod results;
pub mod stage;
pub mod start;
pub mod stopwatch;

use sycamore::prelude::*;

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Record a start/finish [crate::event::RunRecord] locally (runs signal) and
/// enqueue it to the current event's pending outbox — the durable record until
/// it's flushed to the timing room.  No-op when no event is selected.
pub fn enqueue_run(model: crate::Model, run: &crate::event::RunRecord) {
    let (id, uid) = model.app.event.with(|e| (e.id.clone(), e.uid.clone()));
    if id.is_empty() || uid.is_empty() {
        return;
    }
    // The observation's uid is stamped here: the wire carries it and mirrors
    // from room/relay/QR collapse to one record.
    let mut run = run.clone();
    if run.uid.is_empty() {
        run.uid = crate::ids::gen_short_id();
    }
    model.app.runs.update(|runs| {
        crate::event::add_run(runs, run.clone());
    });
    let te = crate::timing_event::TimingEvent {
        r#type: run.r#type.clone(),
        event_id: uid,
        uid: run.uid.clone(),
        target: None,
        test: run.test,
        car: run.car.clone(),
        ts: run.ts,
        time_ds: run.time_ds,
        status: run.status.clone(),
        flags: run.flags,
        official_id: run.official_id.clone(),
        comment: run.comment.clone(),
        refs: run.refs.clone(),
    };
    let sender = model.app.identity.get_clone();
    crate::log::enqueue_pending(&id, crate::log::LogMsg::new_pending(te.body(), sender));
    crate::sync::flush_pending(model);
    crate::app::refresh_feed(model);
}

/// Enqueue a `finish` timing message for a stage/car/time (the command-line
/// stopwatch page) to the current event's pending outbox.
pub fn enqueue_ktime(
    model: crate::Model,
    test: u8,
    car: &str,
    time: &crate::event::KTime,
    comment: Option<String>,
) {
    let (id, uid) = model.app.event.with(|e| (e.id.clone(), e.uid.clone()));
    if id.is_empty() || uid.is_empty() {
        return;
    }
    let mut te = crate::timing_event::TimingEvent::finish(&uid, test, car, time, vec![]);
    te.official_id = Some(model.app.identity.get_clone());
    te.comment = comment;
    let sender = model.app.identity.get_clone();
    crate::log::enqueue_pending(&id, crate::log::LogMsg::new_pending(te.body(), sender));
    // Mirror the finish into the local run log (results read runs, not the
    // collapsed scores) so manually entered times show up even before the
    // room echoes the message back.
    let run = crate::event::record_from_timing(&te);
    model.app.runs.update(|runs| {
        crate::event::add_run(runs, run);
    });
    crate::sync::flush_pending(model);
    crate::app::refresh_feed(model);
}

/// Enqueue an entry state message (upsert or tombstone) for the current event,
/// apply it to the local event immediately, and flush + refresh.
pub fn enqueue_entry(model: crate::Model, entry: &crate::event::Entry, delete: bool) {
    let (id, uid) = model.app.event.with(|e| (e.id.clone(), e.uid.clone()));
    if id.is_empty() || uid.is_empty() {
        return;
    }
    let body = crate::event::entry_body(&uid, entry, delete);
    let sender = model.app.identity.get_clone();
    crate::log::enqueue_pending(&id, crate::log::LogMsg::new_pending(body, sender));
    model.app.event.update(|e| {
        if delete {
            e.remove_entry(entry.entry_no);
        } else {
            e.upsert_entry(entry.clone());
        }
    });
    crate::sync::flush_pending(model);
    crate::app::refresh_feed(model);
    crate::update(model, crate::Msg::Reload);
}

#[allow(dead_code)] // wired by the amend/void UI (Phase 1 backend)
/// Correct an existing observation by `target_uid`: enqueue an `amend` message
/// and patch the local run record in place (the original stays in the log).
pub fn enqueue_amend(
    model: crate::Model,
    target_uid: &str,
    test: u8,
    car: &str,
    time: &crate::event::KTime,
    comment: Option<String>,
) {
    let (id, uid) = model.app.event.with(|e| (e.id.clone(), e.uid.clone()));
    if id.is_empty() || uid.is_empty() {
        return;
    }
    let mut te = crate::timing_event::TimingEvent::amend(&uid, target_uid, test, car, time);
    te.official_id = Some(model.app.identity.get_clone());
    te.comment = comment;
    let sender = model.app.identity.get_clone();
    crate::log::enqueue_pending(&id, crate::log::LogMsg::new_pending(te.body(), sender));
    model.app.runs.update(|runs| {
        if let Some(r) = runs.iter_mut().find(|r| r.uid == target_uid) {
            r.test = te.test;
            r.car = te.car.clone();
            r.time_ds = te.time_ds;
            r.status = te.status.clone();
            r.flags = te.flags;
            r.official_id = te.official_id.clone();
            r.comment = te.comment.clone();
        }
    });
    crate::sync::flush_pending(model);
    crate::app::refresh_feed(model);
    crate::update(model, crate::Msg::Reload);
}

#[allow(dead_code)] // wired by the amend/void UI (Phase 1 backend)
/// Void an existing observation by `target_uid`: enqueue a `void` message and
/// mark the local run record voided (excluded from pairing/scores).
pub fn enqueue_void(model: crate::Model, target_uid: &str, test: u8, car: &str) {
    let (id, uid) = model.app.event.with(|e| (e.id.clone(), e.uid.clone()));
    if id.is_empty() || uid.is_empty() {
        return;
    }
    let mut te = crate::timing_event::TimingEvent::void(&uid, target_uid, test, car);
    te.official_id = Some(model.app.identity.get_clone());
    let sender = model.app.identity.get_clone();
    crate::log::enqueue_pending(&id, crate::log::LogMsg::new_pending(te.body(), sender));
    model.app.runs.update(|runs| {
        if let Some(r) = runs.iter_mut().find(|r| r.uid == target_uid) {
            r.voided = true;
        }
    });
    crate::sync::flush_pending(model);
    crate::app::refresh_feed(model);
    crate::update(model, crate::Msg::Reload);
}

/// Shared timing log: all starts and finishes for a test, newest first, with
/// void buttons.  Used by all timing screens.
pub fn view_timing_log(model: crate::Model, test: u8) -> View {
    use crate::event::{RunRecord, RUN_FINISH, RUN_START, RUN_STOP};
    view! {
        div(class="box") {
            h3(class="title is-6") { "Log" }
            (move || {
                let mut runs: Vec<RunRecord> = model.app.runs.with(|runs| {
                    runs.iter()
                        .filter(|r| r.test == test && !r.voided)
                        .filter(|r| r.r#type == RUN_START || r.r#type == RUN_FINISH || r.r#type == RUN_STOP)
                        .cloned()
                        .collect()
                });
                runs.sort_by_key(|r| std::cmp::Reverse(r.ts));
                if runs.is_empty() {
                    return view! { p(class="help") { "No timing observations yet." } };
                }
                let views: Vec<View> = runs
                    .iter()
                    .map(|r| {
                        let uid = r.uid.clone();
                        let icon = if r.r#type == RUN_START { "\u{25B6}" } else if r.r#type == RUN_STOP { "\u{23F9}" } else { "\u{25A0}" };
                        let label = format!("{} #{}", icon, r.car);
                        let ts = fmt_log_ts(r.ts);
                        let official_view: View = match &r.official_id {
                            Some(o) if !o.is_empty() => {
                                let text = format!("by {}", o);
                                view! { span(class="has-text-grey-light ml-2") { (text) } }
                            }
                            _ => view! {},
                        };
                        let comment_view: View = match &r.comment {
                            Some(c) if !c.is_empty() => {
                                let text = format!("\"{}\"", c);
                                view! { span(class="has-text-grey ml-2 is-size-7") { (text) } }
                            }
                            _ => view! {},
                        };
                        view! {
                            div(class="level is-mobile") {
                                div(class="level-left") {
                                    span(class="has-text-weight-semibold") { (label) }
                                    span(class="has-text-grey ml-2") { (ts) }
                                    (official_view)
                                    (comment_view)
                                }
                                div(class="level-right") {
                                    button(
                                        class="button is-small is-light is-danger",
                                        on:click=move |_| crate::update(model, crate::Msg::VoidObservation(uid.clone())),
                                    ) { span(class="icon is-small") { i(class="fa fa-xmark") } }
                                }
                            }
                        }
                    })
                    .collect();
                views.into()
            })
        }
    }
}

fn fmt_log_ts(ms: i64) -> String {
    let d = js_sys::Date::new(&js_sys::Number::from(ms as f64).into());
    d.to_string().into()
}

/// Offline handoff box: export the current event's log as a QR parcel, scan or
/// paste one to import.  The exporter and scanner open as full-screen modals
/// (see [`view_handoff_modals`]); this box stays compact.
pub fn view_handoff(model: crate::Model) -> View {
    let status = model.app.parcel_status.get_clone();
    let status_view = if status.is_empty() {
        view! {}
    } else {
        let s = status.clone();
        view! { p(class="help has-text-info") { (s) } }
    };
    let open_event = model.app.parcel_open_event.get_clone();
    let open_view = if open_event.is_some() {
        let (_, name) = open_event.clone().unwrap();
        view! {
            div(class="control") {
                button(
                    class="button is-small is-warning",
                    on:click=move |_| crate::update(model, crate::Msg::OpenParcelEvent),
                ) {
                    span(class="icon is-small") { i(class="fa fa-folder-open") }
                    span { (format!("Open {name} and import")) }
                }
            }
        }
    } else {
        view! {}
    };
    view! {
        div(class="box is-hidden-print") {
            h2(class="title is-5") {
                "Offline handoff"
                span(class="tag is-light is-pulled-right") { "QR parcel" }
            }
            p(class="help") {
                "Carry the event's messages device-to-device with no network: export on one phone, scan or paste the parcel on another."
            }
            div(class="field is-grouped") {
                div(class="control") {
                    button(
                        class="button is-primary",
                        on:click=move |_| crate::update(model, crate::Msg::ExportParcel),
                    ) {
                        span(class="icon") { i(class="fa fa-qrcode") }
                        span { "Export parcel" }
                    }
                }
                div(class="control") {
                    button(
                        class="button is-warning",
                        on:click=move |_| crate::update(model, crate::Msg::ScanStart),
                    ) {
                        span(class="icon") { i(class="fa fa-camera") }
                        span { "Scan parcel" }
                    }
                }
            }
            div(class="field") {
                label(class="label is-small") { "Export contains" }
                div(class="control") {
                    div(class="buttons has-addons") {
                        button(
                            class=move || mode_class(model, crate::app::ParcelMode::Full),
                            on:click=move |_| crate::update(model, crate::Msg::SetParcelMode(crate::app::ParcelMode::Full)),
                        ) { "Full event" }
                        button(
                            class=move || mode_class(model, crate::app::ParcelMode::TimingOnly),
                            on:click=move |_| crate::update(model, crate::Msg::SetParcelMode(crate::app::ParcelMode::TimingOnly)),
                        ) { "Timing only" }
                    }
                }
            }
            div(class="field") {
                label(class="label is-small") { "Import a parcel" }
                div(class="control") {
                    textarea(
                        class="textarea is-small",
                        rows="4",
                        bind:value=model.app.parcel_import,
                        placeholder="Paste a khanatime_parcel:… string",
                    ) {}
                }
                div(class="control") {
                    button(
                        class="button is-small is-link",
                        on:click=move |_| crate::update(model, crate::Msg::ImportParcel),
                    ) {
                        span(class="icon is-small") { i(class="fa fa-download") }
                        span { "Import parcel" }
                    }
                }
                (open_view)
            }
            (status_view)
        }
    }
}

/// Button class for the parcel-mode toggle: active mode is solid.
fn mode_class(model: crate::Model, mode: crate::app::ParcelMode) -> String {
    if model.app.parcel_mode.get() == mode {
        "button is-small is-primary".to_string()
    } else {
        "button is-small".to_string()
    }
}

/// Full-screen modal overlays for the QR exporter and camera scanner.  Shown
/// whenever either is active; rendered at app level so it covers any screen.
pub fn view_handoff_modals(model: crate::Model) -> View {
    view! {
        (move || {
            if model.app.scan_active.get() {
                view_scan_modal(model)
            } else if !model.app.parcel_qr_svgs.with(|v| v.is_empty()) {
                view_qr_modal(model)
            } else {
                view! {}
            }
        })
    }
}

/// The exported parcel's QR modal: the current frame as a large SVG, animated
/// across frames when the parcel spans more than one code.
fn view_qr_modal(model: crate::Model) -> View {
    let svgs = model.app.parcel_qr_svgs.get_clone();
    let i = model.app.parcel_qr_index.get();
    let total = model.app.parcel_qr_total.get();
    let paused = model.app.parcel_qr_paused.get();
    let svg = svgs.get(i).cloned().unwrap_or_default();
    let exported = model.app.parcel_export.get_clone();
    let hint = if total > 1 {
        let anim = if paused {
            "paused — resume to cycle frames."
        } else {
            "animated; hold the other phone's camera steady."
        };
        format!("Frame {}/{} — {anim}", i + 1, total)
    } else {
        "Show the other phone's camera at this code.".to_string()
    };
    let pause_label = if paused { "Resume" } else { "Pause" };
    view! {
        div(class="kt-modal") {
            div(class="kt-modal-card") {
                h3(class="title is-5") { "QR export" }
                div(class="kt-qr-box") {
                    div(dangerously_set_inner_html=svg) {}
                }
                p(class="help kt-qr-hint") { (hint) }
                div(class="field is-grouped is-grouped-centered") {
                    (if total > 1 {
                        view! {
                            div(class="control") {
                                button(
                                    class="button is-small is-light",
                                    on:click=move |_| crate::update(model, crate::Msg::QrPauseToggle),
                                ) {
                                    span(class="icon is-small") { i(class="fa fa-pause") }
                                    span { (pause_label) }
                                }
                            }
                        }
                    } else {
                        view! {}
                    })
                    div(class="control") {
                        button(
                            class="button is-small is-link",
                            on:click=move |_| crate::update(model, crate::Msg::QrClear),
                        ) {
                            span(class="icon is-small") { i(class="fa fa-times") }
                            span { "Close" }
                        }
                    }
                    (if exported.is_empty() {
                        view! {}
                    } else {
                        let copy_btn = exported.clone();
                        view! {
                            div(class="control") {
                                button(
                                    class="button is-small is-light",
                                    on:click=move |_| copy_text(&copy_btn),
                                ) {
                                    span(class="icon is-small") { i(class="fa fa-copy") }
                                    span { "Copy text" }
                                }
                            }
                        }
                    })
                }
            }
        }
    }
}

/// Full-screen camera viewfinder modal for scanning a parcel.
fn view_scan_modal(model: crate::Model) -> View {
    let status = model.app.scan_status.get_clone();
    let status_view = if status.is_empty() {
        view! {}
    } else {
        let s = status.clone();
        view! { p(class="help has-text-white") { (s) } }
    };
    view! {
        div(class="kt-modal kt-modal-dark") {
            div(class="kt-modal-scan") {
                div(class="kt-scan-frame") {
                    video(
                        id="kt-scan-video",
                        autoplay=true,
                        playsinline=true,
                        muted=true,
                    ) {}
                    div(class="kt-scan-corners") {
                        i(class="kt-corner kt-corner-tl") {}
                        i(class="kt-corner kt-corner-tr") {}
                        i(class="kt-corner kt-corner-bl") {}
                        i(class="kt-corner kt-corner-br") {}
                    }
                    div(class="kt-scan-mask") {}
                }
                (status_view)
            }
            div(class="kt-modal-topbar") {
                button(
                    class="button is-light is-small",
                    on:click=move |_| crate::update(model, crate::Msg::ScanStop),
                ) {
                    span(class="icon is-small") { i(class="fa fa-times") }
                    span { "Close" }
                }
            }
        }
    }
}

/// Copy `text` to the clipboard (best effort).
pub fn copy_text(text: &str) {
    if let Some(nav) = web_sys::window().map(|w| w.navigator()) {
        let _ = nav.clipboard().write_text(text);
    }
}
