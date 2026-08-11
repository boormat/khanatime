pub mod chat;
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

/// Send a start/finish [crate::event::RunRecord] to the current event's Matrix
/// timing room (no-op on native / before a room is joined).
pub fn broadcast_run(model: crate::Model, run: &crate::event::RunRecord) {
    #[cfg(target_arch = "wasm32")]
    broadcast_run_wasm(model, run);
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (model, run);
    }
}

#[cfg(target_arch = "wasm32")]
fn broadcast_run_wasm(model: crate::Model, run: &crate::event::RunRecord) {
    use crate::services::matrix::room;
    use crate::timing_event::TimingEvent;

    let Some(room) = room() else {
        return;
    };
    let event_id = model.app.event.with(|e| e.id.clone());
    if event_id.is_empty() {
        return;
    }
    let te = TimingEvent {
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
    wasm_bindgen_futures::spawn_local(async move {
        let _ = crate::services::matrix::send_timing(&room, &te).await;
    });
}
