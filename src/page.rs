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
