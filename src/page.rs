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
    let event_id = model.app.event.with(|e| e.id.clone());
    if event_id.is_empty() {
        return;
    }
    model.app.runs.update(|runs| {
        crate::event::add_run(runs, run.clone());
    });
    let te = crate::timing_event::TimingEvent {
        r#type: run.r#type.clone(),
        event_id,
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
    crate::log::enqueue_pending(
        &te.event_id,
        crate::log::LogMsg::new_pending(te.body(), sender),
    );
    crate::sync::flush_pending(model);
    crate::app::refresh_feed(model);
}

/// Enqueue a `finish` timing message for a stage/car/time (the command-line
/// stopwatch page) to the current event's pending outbox.
pub fn enqueue_ktime(model: crate::Model, test: u8, car: &str, time: &crate::event::KTime) {
    let event_id = model.app.event.with(|e| e.id.clone());
    if event_id.is_empty() {
        return;
    }
    let run = model
        .app
        .scores
        .with(|s| s.iter().filter(|x| x.stage == test && x.car == car).count() as u8 + 1);
    let mut te = crate::timing_event::TimingEvent::finish(&event_id, test, car, run, time);
    te.official_id = Some(model.app.identity.get_clone());
    let sender = model.app.identity.get_clone();
    crate::log::enqueue_pending(
        &event_id,
        crate::log::LogMsg::new_pending(te.body(), sender),
    );
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
    let event_id = model.app.event.with(|e| e.id.clone());
    if event_id.is_empty() {
        return;
    }
    let body = crate::event::entry_body(&event_id, entry, delete);
    let sender = model.app.identity.get_clone();
    crate::log::enqueue_pending(&event_id, crate::log::LogMsg::new_pending(body, sender));
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
