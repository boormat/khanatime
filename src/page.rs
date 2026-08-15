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
        run: run.run,
        ts: run.ts,
        time_ds: run.time_ds,
        status: run.status.clone(),
        flags: run.flags,
        official_id: run.official_id.clone(),
    };
    let sender = model.app.identity.get_clone();
    crate::log::enqueue_pending(&id, crate::log::LogMsg::new_pending(te.body(), sender));
    crate::sync::flush_pending(model);
    crate::app::refresh_feed(model);
}

/// Enqueue a `finish` timing message for a stage/car/time (the command-line
/// stopwatch page) to the current event's pending outbox.
pub fn enqueue_ktime(model: crate::Model, test: u8, car: &str, time: &crate::event::KTime) {
    let (id, uid) = model.app.event.with(|e| (e.id.clone(), e.uid.clone()));
    if id.is_empty() || uid.is_empty() {
        return;
    }
    let run = model
        .app
        .runs
        .with(|runs| crate::event::next_run(runs, test, car));
    let mut te = crate::timing_event::TimingEvent::finish(&uid, test, car, run, time);
    te.official_id = Some(model.app.identity.get_clone());
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
    run: u8,
    time: &crate::event::KTime,
) {
    let (id, uid) = model.app.event.with(|e| (e.id.clone(), e.uid.clone()));
    if id.is_empty() || uid.is_empty() {
        return;
    }
    let mut te = crate::timing_event::TimingEvent::amend(&uid, target_uid, test, car, run, time);
    te.official_id = Some(model.app.identity.get_clone());
    let sender = model.app.identity.get_clone();
    crate::log::enqueue_pending(&id, crate::log::LogMsg::new_pending(te.body(), sender));
    model.app.runs.update(|runs| {
        if let Some(r) = runs.iter_mut().find(|r| r.uid == target_uid) {
            r.test = te.test;
            r.car = te.car.clone();
            r.run = te.run;
            r.time_ds = te.time_ds;
            r.status = te.status.clone();
            r.flags = te.flags;
            r.official_id = te.official_id.clone();
        }
    });
    crate::sync::flush_pending(model);
    crate::app::refresh_feed(model);
    crate::update(model, crate::Msg::Reload);
}

#[allow(dead_code)] // wired by the amend/void UI (Phase 1 backend)
/// Void an existing observation by `target_uid`: enqueue a `void` message and
/// mark the local run record voided (excluded from pairing/scores).
pub fn enqueue_void(model: crate::Model, target_uid: &str, test: u8, car: &str, run: u8) {
    let (id, uid) = model.app.event.with(|e| (e.id.clone(), e.uid.clone()));
    if id.is_empty() || uid.is_empty() {
        return;
    }
    let mut te = crate::timing_event::TimingEvent::void(&uid, target_uid, test, car, run);
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

/// Offline handoff box: export the current event's log as a QR parcel (shown
/// as text to copy/scan until QR rendering lands) or import a parcel pasted
/// from another device.  See `sync::export_parcel` / `sync::import_parcel`.
pub fn view_handoff(model: crate::Model) -> View {
    let exported = model.app.parcel_export.get_clone();
    let status = model.app.parcel_status.get_clone();
    let exported_view = if exported.is_empty() {
        view! {}
    } else {
        let text_view = exported.clone();
        let copy = exported.clone();
        view! {
            div(class="field") {
                label(class="label is-small") { "Copy this parcel" }
                div(class="control") {
                    textarea(
                        class="textarea is-small",
                        readonly=true,
                        rows="6",
                    ) { (text_view) }
                }
                div(class="control") {
                    button(
                        class="button is-small is-light",
                        on:click=move |_| copy_text(&copy),
                    ) {
                        span(class="icon is-small") { i(class="fa fa-copy") }
                        span { "Copy" }
                    }
                }
            }
        }
    };
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
                "Carry the event's messages device-to-device with no network: export on one phone, paste or scan the parcel on another."
            }
            div(class="field") {
                div(class="control") {
                    button(
                        class="button is-small is-primary",
                        on:click=move |_| crate::update(model, crate::Msg::ExportParcel),
                    ) {
                        span(class="icon is-small") { i(class="fa fa-qrcode") }
                        span { "Export parcel" }
                    }
                }
            }
            (exported_view)
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

/// Copy `text` to the clipboard (best effort).
fn copy_text(text: &str) {
    if let Some(nav) = web_sys::window().map(|w| w.navigator()) {
        let _ = nav.clipboard().write_text(text);
    }
}
