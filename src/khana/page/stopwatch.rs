use sycamore::prelude::*;

use crate::event::{
    elapsed_ds, pending_for_car, pending_starts, upsert_ktime, RunRecord, RUN_FINISH, RUN_START,
    RUN_STOP,
};
use crate::khana::page::{pad, penalty};

// Cooperative stopwatch: START sends a start event, STOP sends a stop event
// and opens a confirm panel with auto-attached observations, CONFIRM sends a
// finish event referencing the attached UIDs.  Multiple stops stack.

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub enum Msg {
    Test(u8),
    Start,
    Stop,
    /// Toggle attachment of an event in the pending confirm panel.
    ToggleAttach(usize),
    Commit(usize),
    Cancel(usize),
    /// Manual time entry: car + time → opens confirm panel.
    ManualTime,
    ToggleManual,
    Void(String),
}

// ---------------------------------------------------------------------------
// Attached event (shown in confirm panel)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct AttachedEvent {
    pub uid: String,
    pub r#type: String,
    pub ts: i64,
    pub car: String,
    pub attached: bool,
}

// ---------------------------------------------------------------------------
// Pending finish (IS the finish record minus uid)
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct PendingFinish {
    pub car: String,
    pub time_ds: u16,
    pub status: String,
    pub flags: u8,
    pub garage: bool,
    pub comment: String,
    pub attached: Vec<AttachedEvent>,
}

impl PendingFinish {
    fn refs(&self) -> Vec<String> {
        self.attached
            .iter()
            .filter(|a| a.attached)
            .map(|a| a.uid.clone())
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub struct Model {
    pub test: Signal<u8>,
    pub car: Signal<String>,
    pub comment: Signal<String>,
    pub time: Signal<String>,
    pub show_manual: Signal<bool>,
    pub penalty: penalty::PenaltyModel,
    pub pending: Signal<Vec<PendingFinish>>,
    pub feedback: Signal<Option<String>>,
    pub cancel_warn: Signal<Option<usize>>,
}

pub fn init() -> Model {
    Model {
        test: create_signal(1),
        car: create_signal(String::new()),
        comment: create_signal(String::new()),
        time: create_signal(String::new()),
        show_manual: create_signal(false),
        penalty: penalty::init(),
        pending: create_signal(Vec::new()),
        feedback: create_signal(None),
        cancel_warn: create_signal(None),
    }
}

pub fn update(model: crate::Model, msg: Msg) {
    match msg {
        Msg::Test(t) => model.screens.stopwatch.test.set(t),
        Msg::Start => start_car(model),
        Msg::Stop => stop_car(model),
        Msg::ToggleAttach(idx) => toggle_attach(model, idx),
        Msg::Commit(idx) => commit(model, idx),
        Msg::Cancel(idx) => cancel(model, idx),
        Msg::ManualTime => manual_time(model),
        Msg::ToggleManual => {
            let sm = model.screens.stopwatch;
            sm.show_manual.set(!sm.show_manual.get());
        }
        Msg::Void(uid) => void_observation(model, &uid),
    }
}

// ---------------------------------------------------------------------------
// Auto-attach logic
// ---------------------------------------------------------------------------

/// Collect all UIDs already referenced by finish records (used by other results).
fn finish_refs_used(runs: &[RunRecord]) -> std::collections::HashSet<String> {
    runs.iter()
        .filter(|r| r.r#type == RUN_FINISH && !r.voided)
        .flat_map(|r| r.refs.iter().cloned())
        .collect()
}

/// Auto-attach all start+stop events for the same (test, car) that are:
/// - not voided
/// - not DNS
/// - not already referenced by another finish's refs
///
/// Sorted by timestamp (oldest first).
fn auto_attach(runs: &[RunRecord], test: u8, car: &str) -> Vec<AttachedEvent> {
    let used = finish_refs_used(runs);
    let mut events: Vec<AttachedEvent> = runs
        .iter()
        .filter(|r| r.test == test && r.car == car)
        .filter(|r| r.r#type == RUN_START || r.r#type == RUN_STOP)
        .filter(|r| !r.voided)
        .filter(|r| r.status.as_deref() != Some("dns"))
        .filter(|r| !used.contains(&r.uid))
        .map(|r| AttachedEvent {
            uid: r.uid.clone(),
            r#type: r.r#type.clone(),
            ts: r.ts,
            car: r.car.clone(),
            attached: true,
        })
        .collect();
    events.sort_by_key(|a| a.ts);
    events
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

fn unknown_comment_required(model: crate::Model) -> bool {
    let sm = model.screens.stopwatch;
    let car = sm.car.get_clone();
    if car.trim() == "?" && sm.comment.get_clone().trim().is_empty() {
        sm.feedback
            .set(Some("Comment is required for unknown cars".to_string()));
        return true;
    }
    false
}

fn start_car(model: crate::Model) {
    let sm = model.screens.stopwatch;
    if unknown_comment_required(model) {
        return;
    }
    let car = resolved_car(model);
    let test = sm.test.get();
    if pending_for_car(&model.khana.runs.get_clone(), test, &car) {
        sm.feedback
            .set(Some("Car is already on course — use STOP".to_string()));
        return;
    }
    let comment = sm.comment.get_clone();
    let comment_opt = non_empty(comment);
    let record = RunRecord {
        uid: String::new(),
        r#type: RUN_START.to_string(),
        test,
        car,
        ts: js_sys::Date::now() as i64,
        time_ds: None,
        status: Some("clean".to_string()),
        flags: None,
        official_id: Some(model.sync.identity.get_clone()),
        voided: false,
        comment: comment_opt,
        refs: vec![],
    };
    crate::khana::helpers::enqueue_run(model, &record);
    sm.feedback.set(None);
}

fn stop_car(model: crate::Model) {
    let sm = model.screens.stopwatch;
    let car = resolved_car(model);
    let test = sm.test.get();
    let now = js_sys::Date::now() as i64;
    let runs = model.khana.runs.get_clone();
    let elapsed = pending_starts(&runs, test)
        .iter()
        .find(|r| r.car == car)
        .map(|s| elapsed_ds(s.ts, now))
        .unwrap_or(0);
    let comment = sm.comment.get_clone();
    let comment_opt = non_empty(comment);
    let record = RunRecord {
        uid: String::new(),
        r#type: RUN_STOP.to_string(),
        test,
        car: car.clone(),
        ts: now,
        time_ds: Some(elapsed),
        status: Some("clean".to_string()),
        flags: None,
        official_id: Some(model.sync.identity.get_clone()),
        voided: false,
        comment: comment_opt,
        refs: vec![],
    };
    crate::khana::helpers::enqueue_run(model, &record);

    // Now auto-attach events for this car/test.
    let runs = model.khana.runs.get_clone();
    let attached = auto_attach(&runs, test, &car);
    sm.pending.update(|v| {
        v.push(PendingFinish {
            car,
            time_ds: elapsed,
            status: "clean".to_string(),
            flags: 0,
            garage: false,
            comment: String::new(),
            attached,
        });
    });
    sm.feedback.set(None);
    sm.car.set(String::new());
}

fn toggle_attach(model: crate::Model, idx: usize) {
    let sm = model.screens.stopwatch;
    sm.pending.update(|v| {
        if let Some(p) = v.get_mut(idx) {
            if let Some(a) = p.attached.get_mut(idx) {
                a.attached = !a.attached;
            }
        }
    });
}

fn commit(model: crate::Model, idx: usize) {
    let sm = model.screens.stopwatch;
    let pending = sm.pending.with(|v| v.get(idx).cloned());
    let pending = match pending {
        Some(p) => p,
        None => return,
    };
    let now = js_sys::Date::now() as i64;
    sm.penalty.flags.set(pending.flags);
    sm.penalty.garage.set(pending.garage);
    sm.penalty.status.set(pending.status.clone());
    let time_ds = pending.time_ds;
    let ktime = penalty::to_ktime(sm.penalty, time_ds);
    let refs = pending.refs();
    let comment = pending.comment;
    let car = pending.car;
    let record = RunRecord {
        uid: String::new(),
        r#type: RUN_FINISH.to_string(),
        test: sm.test.get(),
        car: car.clone(),
        ts: now,
        time_ds: Some(time_ds),
        status: Some(sm.penalty.status.get_clone()),
        flags: Some(sm.penalty.flags.get()),
        official_id: Some(model.sync.identity.get_clone()),
        voided: false,
        comment: Some(comment).filter(|c| !c.is_empty()),
        refs,
    };
    model.khana.scores.update(|s| {
        upsert_ktime(s, sm.test.get(), &car, ktime);
    });
    crate::khana::helpers::enqueue_run(model, &record);
    sm.pending.update(|v| v.remove(idx));
    sm.car.set(String::new());
    sm.comment.set(String::new());
    sm.feedback.set(None);
    penalty::clear(sm.penalty);
}

fn cancel(model: crate::Model, idx: usize) {
    let sm = model.screens.stopwatch;
    // Void all attached events in this pending finish.
    let pending = sm.pending.with(|v| v.get(idx).cloned());
    if let Some(p) = pending {
        let test = sm.test.get();
        for a in &p.attached {
            if a.attached {
                crate::khana::helpers::enqueue_void(model, &a.uid, test, &a.car);
            }
        }
    }
    sm.pending.update(|v| v.remove(idx));
    sm.feedback.set(None);
    sm.cancel_warn.set(None);
    penalty::clear(sm.penalty);
}

fn manual_time(model: crate::Model) {
    let sm = model.screens.stopwatch;
    if unknown_comment_required(model) {
        return;
    }
    let car = sm.car.get_clone().trim().to_string();
    if car.is_empty() {
        sm.feedback.set(Some("Pick a car number".to_string()));
        return;
    }
    let time_str = sm.time.get_clone();
    let elapsed = match time_str.trim().parse::<f32>() {
        Ok(v) if v > 0.0 => (v * 10.0).round() as u16,
        _ => {
            sm.feedback
                .set(Some("Enter a valid time in seconds".to_string()));
            return;
        }
    };
    let test = sm.test.get();
    let runs = model.khana.runs.get_clone();
    let attached = auto_attach(&runs, test, &car);
    sm.pending.update(|v| {
        v.push(PendingFinish {
            car,
            time_ds: elapsed,
            status: "clean".to_string(),
            flags: 0,
            garage: false,
            comment: String::new(),
            attached,
        });
    });
    sm.time.set(String::new());
    sm.feedback.set(None);
}

fn void_observation(model: crate::Model, uid: &str) {
    let sm = model.screens.stopwatch;
    let test = sm.test.get();
    let runs = model.khana.runs.get_clone();
    let r = match runs.iter().find(|r| r.uid == uid) {
        Some(r) => r.clone(),
        None => return,
    };
    crate::khana::helpers::enqueue_void(model, uid, test, &r.car);
    sm.feedback
        .set(Some(format!("Voided {} #{}", r.r#type, r.car)));
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn resolved_car(model: crate::Model) -> String {
    let car = model.screens.stopwatch.car.get_clone();
    let trimmed = car.trim().to_string();
    if trimmed.is_empty() {
        "?".to_string()
    } else {
        trimmed
    }
}

fn non_empty(s: String) -> Option<String> {
    if s.trim().is_empty() {
        None
    } else {
        Some(s)
    }
}

fn fmt_ts(ts: i64) -> String {
    let d = js_sys::Date::new(&js_sys::Number::from(ts as f64).into());
    d.to_string().into()
}

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

pub fn view(model: crate::Model) -> View {
    let sm = model.screens.stopwatch;
    view! {
        div {
            (view_action_buttons(model))
            (view_car_chips(model))
            (view_comment(model))
            (view_manual_entry(model))
            (view_pending_stack(model))
            (crate::khana::helpers::view_timing_log(model, sm.test.get()))
        }
    }
}

/// Selected car + START (green) / STOP (red) — all in one row.
fn view_action_buttons(model: crate::Model) -> View {
    let sm = model.screens.stopwatch;
    view! {
        div(class="columns is-gapless mt-3 is-vcentered") {
            div(class="column is-narrow mr-3") {
                (move || {
                    let car = sm.car.get_clone();
                    let trimmed = car.trim().to_string();
                    if trimmed.is_empty() {
                        return view! {
                            div(
                                class="notification is-light is-clickable has-text-grey",
                                on:click=move |_| sm.car.set(String::new()),
                                style="padding: 0.5rem 0.75rem; min-width: 8rem;",
                            ) { "No car" }
                        };
                    }
                    let label = if trimmed == "?" {
                        "#? Unknown".to_string()
                    } else {
                        let (name, _vehicle) = model.khana.event.with(|e| {
                            e.entries.iter()
                                .find(|en| en.car == trimmed)
                                .map(|en| (en.name.clone(), en.vehicle.clone()))
                                .unwrap_or_default()
                        });
                        format!("#{} {}", trimmed, name)
                    };
                    view! {
                        div(
                            class="notification is-primary is-light is-clickable",
                            on:click=move |_| sm.car.set(String::new()),
                            style="padding: 0.5rem 0.75rem; min-width: 8rem;",
                        ) {
                            span(class="has-text-weight-semibold is-size-6") { (label) }
                            span(class="has-text-grey is-size-7 ml-1") { " ×" }
                        }
                    }
                })
            }
            div(class="column") {
                button(
                    class=move || {
                        let car = sm.car.get_clone();
                        let has = pending_for_car(
                            &model.khana.runs.get_clone(),
                            sm.test.get(),
                            car.trim(),
                        );
                        if has { "button is-light is-large is-fullwidth" }
                        else { "button is-success is-large is-fullwidth" }
                    },
                    on:click=move |_| update(model, Msg::Start),
                ) {
                    span(class="icon is-medium") { i(class="fa fa-flag-checkered") }
                    span { " START" }
                }
            }
            div(class="column") {
                button(
                    class=move || {
                        let car = sm.car.get_clone();
                        let has = pending_for_car(
                            &model.khana.runs.get_clone(),
                            sm.test.get(),
                            car.trim(),
                        );
                        if has { "button is-danger is-large is-fullwidth" }
                        else { "button is-light is-large is-fullwidth" }
                    },
                    on:click=move |_| update(model, Msg::Stop),
                ) {
                    span(class="icon is-medium") { i(class="fa fa-flag-checkered") }
                    span { " STOP" }
                }
            }
        }
    }
}

/// Car chips for selecting a car, grouped by runs remaining.
fn view_car_chips(model: crate::Model) -> View {
    let sm = model.screens.stopwatch;
    view! {
        div(class="box") {
            (move || {
                let entries = model.khana.event.with(|e| e.entries.clone());
                let test = sm.test.get();
                let runs_total = model.khana.event.with(|e| {
                    e.stages.iter()
                        .find(|s| s.num == test)
                        .map(|s| s.runs_total)
                        .unwrap_or(1)
                });
                // Start every car at runs_total, subtract finishes.
                let mut runs_remaining: std::collections::HashMap<String, u8> = entries
                    .iter()
                    .filter(|e| !e.car.is_empty())
                    .map(|e| (e.car.clone(), runs_total))
                    .collect();
                let mut unknown_finishes: usize = 0;
                let runs: Vec<crate::event::RunRecord> = model.khana.runs.with(|r| r.clone());
                for r in &runs {
                    if r.r#type == RUN_FINISH && r.test == test && !r.voided {
                        if r.car == "?" {
                            unknown_finishes += 1;
                        } else if let Some(rem) = runs_remaining.get_mut(&r.car) {
                            *rem = rem.saturating_sub(1);
                        }
                    }
                }
                let unknown_remaining = runs_total.saturating_sub(unknown_finishes as u8);
                pad::car_chips_with_runs(entries, sm.car, &runs_remaining, unknown_remaining)
            })
        }
    }
}

/// Comment input — always visible, required only for "?".
fn view_comment(model: crate::Model) -> View {
    let sm = model.screens.stopwatch;
    view! {
        div(class="field") {
            label(class="label is-size-7") { "Comment" }
            input(class="input", r#type="text", placeholder="Optional note (required for #?)", bind:value=sm.comment)
        }
    }
}

/// Manual entry line — Car # + Time (s) + Add button.
fn view_manual_entry(model: crate::Model) -> View {
    let sm = model.screens.stopwatch;
    view! {
        div(class="field has-addons") {
            div(class="control") {
                button(
                    class=move || {
                        if sm.show_manual.get() { "button is-primary" } else { "button" }
                    },
                    on:click=move |_| update(model, Msg::ToggleManual),
                ) { "Manual" }
            }
            (move || {
                if sm.show_manual.get() {
                    view! {
                        div(class="control is-expanded") {
                            input(class="input", r#type="text", placeholder="Time in seconds", bind:value=sm.time)
                        }
                        div(class="control") {
                            button(
                                class="button is-primary",
                                on:click=move |_| update(model, Msg::ManualTime),
                            ) { "Add" }
                        }
                    }
                } else {
                    view! {}
                }
            })
        }
    }
}

/// Stacked pending confirmations — each with attached events, penalties, CONFIRM/Cancel.
fn view_pending_stack(model: crate::Model) -> View {
    let sm = model.screens.stopwatch;
    view! {
        (move || {
            let count = sm.pending.with(|v| v.len());
            if count == 0 {
                return view! {};
            }
            let views: Vec<View> = (0..count)
                .map(|idx| view_pending_one(model, idx))
                .collect();
            views.into()
        })
    }
}

fn view_pending_one(model: crate::Model, idx: usize) -> View {
    let sm = model.screens.stopwatch;
    view! {
        div(class="box") {
            (move || {
                let p = sm.pending.with(|v| v.get(idx).cloned());
                let p = match p {
                    Some(p) => p,
                    None => return view! {},
                };
                let elapsed_str = format!("{:.2}s", p.time_ds as f32 / 10.0);
                let car = p.car.clone();
                let time_ds = p.time_ds;
                let attached_events = view_attached_events(model, idx, &p);
                let penalty_view = penalty::view(model, sm.penalty, time_ds);
                view! {
                    h3(class="title is-6") { "Confirm finish" }
                    p(class="notification is-primary is-light kt-entrant-line") {
                        (crate::view::car_tag(&car))
                        span { " \u{2014} " (elapsed_str) }
                    }
                    (attached_events)
                    (penalty_view)
                    div(class="field is-grouped") {
                        div(class="control is-expanded") {
                            button(
                                class="button is-success is-large is-fullwidth",
                                on:click=move |_| update(model, Msg::Commit(idx)),
                            ) {
                                span(class="icon") { i(class="fa fa-flag-checkered") }
                                span { "CONFIRM" }
                            }
                        }
                        div(class="control") {
                            button(
                                class="button is-light is-large",
                                on:click=move |_| update(model, Msg::Cancel(idx)),
                            ) { "Cancel" }
                        }
                    }
                }
            })
        }
    }
}

fn view_attached_events(model: crate::Model, idx: usize, _p: &PendingFinish) -> View {
    let sm = model.screens.stopwatch;
    view! {
        div(class="mb-3") {
            p(class="has-text-weight-semibold is-size-7 mb-1") { "Attached observations:" }
            (move || {
                let events = sm.pending.with(|v| {
                    v.get(idx)
                        .map(|p| p.attached.clone())
                        .unwrap_or_default()
                });
                if events.is_empty() {
                    return view! { p(class="help") { "No matching start/stop events found." } };
                }
                let views: Vec<View> = events
                    .iter()
                    .enumerate()
                    .map(|(i, a)| {
                        let (icon_char, icon_class) = if a.r#type == RUN_START {
                            ("\u{25B6}", "has-text-success")
                        } else {
                            ("\u{23F9}", "has-text-danger")
                        };
                        let is_attached = a.attached;
                        let car = a.car.clone();
                        let ts = fmt_ts(a.ts);
                        let strike = if is_attached { "" } else { "has-text-grey-light has-text-decoration-line-through" };
                        view! {
                            div(class="level is-mobile mb-1") {
                                div(class="level-left") {
                                    span(class=strike) {
                                        span(class=icon_class) { (icon_char) }
                                        span { (format!(" #{} — {}", car, ts)) }
                                    }
                                }
                                div(class="level-right") {
                                    button(
                                        class=format!("button is-small {}", if is_attached { "is-light is-danger" } else { "is-light" }),
                                        on:click=move |_| {
                                            // Toggle via message
                                            sycamore::reactive::untrack(move || {
                                                update(model, Msg::ToggleAttach(i));
                                            });
                                        },
                                    ) {
                                        span(class="icon is-small") {
                                            i(class=if is_attached { "fa fa-xmark" } else { "fa fa-plus" })
                                        }
                                    }
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
