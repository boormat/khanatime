use sycamore::prelude::*;

use crate::event::{
    elapsed_ds, pending_for_car, pending_starts, upsert_ktime, RunRecord, RUN_FINISH, RUN_START,
    RUN_STOP,
};
use crate::khana::page::penalty;

// Cooperative stopwatch: START sends a start event, STOP creates a provisional
// finish in the log, CONFIRM sends it to the outbox.
//
// Workflow: Select car → Start (or Manual) → Stop → Edit penalties in log → Confirm → Done.

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub enum Msg {
    Test(u8),
    Start,
    Stop,
    Commit,
    Cancel,
    /// Open the confirm panel with a manual time input (requires car selected).
    ManualTime,
    Void(String),
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
    pub penalty: penalty::PenaltyModel,
    pub provisional_uid: Signal<Option<String>>,
    pub feedback: Signal<Option<String>>,
    pub show_car_picker: Signal<bool>,
    pub editing_observation: Signal<Option<String>>,
    pub edit_uid: Signal<Option<String>>,
    pub edit_time: Signal<String>,
    pub edit_flags: Signal<u8>,
    pub edit_garage: Signal<bool>,
    pub edit_status: Signal<String>,
    pub edit_comment: Signal<String>,
    pub selected_picker_car: Signal<Option<String>>,
}

pub fn init() -> Model {
    Model {
        test: create_signal(1),
        car: create_signal(String::new()),
        comment: create_signal(String::new()),
        time: create_signal(String::new()),
        penalty: penalty::init(),
        provisional_uid: create_signal(None),
        feedback: create_signal(None),
        show_car_picker: create_signal(false),
        editing_observation: create_signal(None),
        edit_uid: create_signal(None),
        edit_time: create_signal(String::new()),
        edit_flags: create_signal(0u8),
        edit_garage: create_signal(false),
        edit_status: create_signal(String::new()),
        edit_comment: create_signal(String::new()),
        selected_picker_car: create_signal(None),
    }
}

/// Populate the edit-form signals from a run record. Safe to call from any
/// context (no `create_signal` — only `.set()` on pre-existing signals).
pub fn populate_edit(model: crate::Model, r: &crate::event::RunRecord) {
    let sm = model.screens.stopwatch;
    sm.edit_uid.set(Some(r.uid.clone()));
    sm.edit_time.set(
        r.time_ds
            .map(|ds| format!("{:.1}", ds as f32 / 10.0))
            .unwrap_or_default(),
    );
    sm.edit_flags.set(r.flags.unwrap_or(0));
    let is_garage = r.status.as_deref() == Some("garage");
    sm.edit_garage.set(is_garage);
    let status_str = r.status.clone().unwrap_or_default();
    sm.edit_status.set(if is_garage {
        "clean".to_string()
    } else {
        status_str
    });
    sm.edit_comment.set(r.comment.clone().unwrap_or_default());
}

pub fn update(model: crate::Model, msg: Msg) {
    match msg {
        Msg::Test(t) => model.screens.stopwatch.test.set(t),
        Msg::Start => start_car(model),
        Msg::Stop => stop_car(model),
        Msg::Commit => commit(model),
        Msg::Cancel => cancel(model),
        Msg::ManualTime => manual_time(model),
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
/// Returns UIDs sorted by timestamp (oldest first).
fn auto_attach(runs: &[RunRecord], test: u8, car: &str) -> Vec<String> {
    let used = finish_refs_used(runs);
    let mut events: Vec<(i64, String)> = runs
        .iter()
        .filter(|r| r.test == test && r.car == car)
        .filter(|r| r.r#type == RUN_START || r.r#type == RUN_STOP)
        .filter(|r| !r.voided)
        .filter(|r| r.status.as_deref() != Some("dns"))
        .filter(|r| !used.contains(&r.uid))
        .map(|r| (r.ts, r.uid.clone()))
        .collect();
    events.sort_by_key(|a| a.0);
    events.into_iter().map(|(_, uid)| uid).collect()
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

fn start_car(model: crate::Model) {
    let sm = model.screens.stopwatch;
    let car = sm.car.get_clone().trim().to_string();
    if car.is_empty() {
        sm.feedback.set(Some("Select a car first".to_string()));
        return;
    }
    if crate::khana::helpers::check_unknown_comment(&car, &sm.comment.get_clone(), &sm.feedback) {
        return;
    }
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
        official_id: Some(crate::khana::helpers::current_official(model)),
        voided: false,
        comment: comment_opt,
        refs: vec![],
        provisional: false,
    };
    crate::khana::helpers::enqueue_run(model, &record);
    sm.feedback.set(None);
}

fn stop_car(model: crate::Model) {
    let sm = model.screens.stopwatch;
    let car = sm.car.get_clone().trim().to_string();
    if car.is_empty() {
        sm.feedback.set(Some("Select a car first".to_string()));
        return;
    }
    if crate::khana::helpers::check_unknown_comment(&car, &sm.comment.get_clone(), &sm.feedback) {
        return;
    }
    let test = sm.test.get();
    let runs = model.khana.runs.get_clone();
    if !pending_for_car(&runs, test, &car) {
        sm.feedback
            .set(Some("Car is not on course — use START first".to_string()));
        return;
    }
    let now = js_sys::Date::now() as i64;
    let elapsed = pending_starts(&runs, test)
        .iter()
        .find(|r| r.car == car)
        .map(|s| elapsed_ds(s.ts, now))
        .unwrap_or(0);
    // Auto-attach all matching start/stop observations.
    let attached = auto_attach(&runs, test, &car);
    let refs: Vec<String> = attached;
    let uid = crate::ids::gen_short_id();
    let comment = non_empty(sm.comment.get_clone());
    let record = RunRecord {
        uid: uid.clone(),
        r#type: RUN_FINISH.to_string(),
        test,
        car: car.clone(),
        ts: now,
        time_ds: Some(elapsed),
        status: Some("clean".to_string()),
        flags: Some(0),
        official_id: Some(crate::khana::helpers::current_official(model)),
        voided: false,
        comment,
        refs,
        provisional: true,
    };
    model.khana.runs.update(|runs| {
        crate::khana::event::add_run(runs, record);
    });
    sm.provisional_uid.set(Some(uid));
    sm.feedback.set(None);
    sm.car.set(String::new());
    save_car("");
}

fn commit(model: crate::Model) {
    let sm = model.screens.stopwatch;
    let uid = match sm.provisional_uid.get_clone() {
        Some(uid) => uid,
        None => return,
    };
    let runs = model.khana.runs.get_clone();
    let record = match runs.iter().find(|r| r.uid == uid && r.provisional) {
        Some(r) => r.clone(),
        None => return,
    };
    let car = record.car.clone();
    let test = record.test;
    // A manual timed run (no attached start/stop) supersedes any start still
    // on course: void it so the car isn't left staged waiting on a Stop.
    if record.refs.is_empty() {
        crate::khana::helpers::void_pending_starts_for_car(model, &car, test);
    }
    // Build KTime from the record's current penalty fields.
    let ktime = crate::khana::event::finish_to_ktime(&record);
    model.khana.scores.update(|s| {
        upsert_ktime(s, test, &car, ktime);
    });
    // Enqueue to outbox (the record is already in runs from stop_car).
    crate::khana::helpers::enqueue_run(model, &record);
    // Clear provisional flag — the record is now confirmed.
    model.khana.runs.update(|runs| {
        if let Some(r) = runs.iter_mut().find(|r| r.uid == uid) {
            r.provisional = false;
        }
    });
    sm.provisional_uid.set(None);
    sm.editing_observation.set(None);
    sm.edit_uid.set(None);
    sm.car.set(String::new());
    save_car("");
    sm.comment.set(String::new());
    save_comment("");
    sm.time.set(String::new());
    sm.feedback.set(None);
    penalty::clear(sm.penalty);
}

fn cancel(model: crate::Model) {
    let sm = model.screens.stopwatch;
    if let Some(uid) = sm.provisional_uid.get_clone() {
        // Remove the provisional record from runs — it was never sent.
        model.khana.runs.update(|runs| {
            runs.retain(|r| r.uid != uid);
        });
    }
    sm.provisional_uid.set(None);
    sm.editing_observation.set(None);
    sm.edit_uid.set(None);
    sm.time.set(String::new());
    sm.feedback.set(None);
    penalty::clear(sm.penalty);
}

fn manual_time(model: crate::Model) {
    let sm = model.screens.stopwatch;
    let car = sm.car.get_clone().trim().to_string();
    if car.is_empty() {
        sm.feedback.set(Some("Select a car first".to_string()));
        return;
    }
    if crate::khana::helpers::check_unknown_comment(&car, &sm.comment.get_clone(), &sm.feedback) {
        return;
    }
    let uid = crate::ids::gen_short_id();
    let comment = non_empty(sm.comment.get_clone());
    let record = RunRecord {
        uid: uid.clone(),
        r#type: RUN_FINISH.to_string(),
        test: sm.test.get(),
        car: car.clone(),
        ts: js_sys::Date::now() as i64,
        time_ds: Some(0),
        status: Some("clean".to_string()),
        flags: Some(0),
        official_id: Some(crate::khana::helpers::current_official(model)),
        voided: false,
        comment,
        refs: vec![],
        provisional: true,
    };
    model.khana.runs.update(|runs| {
        crate::khana::event::add_run(runs, record);
    });
    sm.provisional_uid.set(Some(uid));
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

fn non_empty(s: String) -> Option<String> {
    if s.trim().is_empty() {
        None
    } else {
        Some(s)
    }
}

// ---------------------------------------------------------------------------
// Session persistence (survives refresh, clears on tab close)
// ---------------------------------------------------------------------------

fn session_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.session_storage().ok().flatten()
}

fn save_car(car: &str) {
    if let Some(st) = session_storage() {
        if car.is_empty() {
            let _ = st.remove_item("kt_sw_car");
        } else {
            let _ = st.set_item("kt_sw_car", car);
        }
    }
}

fn load_car() -> String {
    session_storage()
        .and_then(|st| st.get_item("kt_sw_car").ok().flatten())
        .unwrap_or_default()
}

fn save_comment(comment: &str) {
    if let Some(st) = session_storage() {
        if comment.is_empty() {
            let _ = st.remove_item("kt_sw_comment");
        } else {
            let _ = st.set_item("kt_sw_comment", comment);
        }
    }
}

fn load_comment() -> String {
    session_storage()
        .and_then(|st| st.get_item("kt_sw_comment").ok().flatten())
        .unwrap_or_default()
}

/// On reload: restore car selection and comment from sessionStorage.
pub fn restore_session(model: crate::Model) {
    let sm = model.screens.stopwatch;
    let car = load_car();
    if !car.is_empty() {
        sm.car.set(car);
    }
    let comment = load_comment();
    if !comment.is_empty() {
        sm.comment.set(comment);
    }
}

/// Clear transient confirm/edit state (provisional finish, open edit forms,
/// manual-time input).  Called when a stage is entered/left and as part of the
/// event-change reset, so a stale provisional can't hide the timing UI (B12).
/// Car/comment are kept here — they belong to the timing session and are only
/// dropped when the event itself changes (see [`clear_session`]).
pub fn reset_transient(model: crate::Model) {
    let sm = model.screens.stopwatch;
    sm.provisional_uid.set(None);
    sm.editing_observation.set(None);
    sm.edit_uid.set(None);
    sm.edit_time.set(String::new());
    sm.edit_flags.set(0);
    sm.edit_garage.set(false);
    sm.edit_status.set(String::new());
    sm.edit_comment.set(String::new());
    sm.time.set(String::new());
    sm.feedback.set(None);
    sm.show_car_picker.set(false);
    sm.selected_picker_car.set(None);
    penalty::clear(sm.penalty);
}

/// Drop the session-persisted car/comment — called when the event changes so a
/// selection from a previous event can't carry over.  Same-event page refresh
/// still restores them via sessionStorage (the "session" design intent).
pub fn clear_session(model: crate::Model) {
    let sm = model.screens.stopwatch;
    sm.car.set(String::new());
    save_car("");
    sm.comment.set(String::new());
    save_comment("");
}

/// Show a transient error/notice line on the stopwatch page.
pub fn show_feedback(model: crate::Model, msg: impl Into<String>) {
    model.screens.stopwatch.feedback.set(Some(msg.into()));
}

/// Clear the selected car + comment + time + penalty after a provisional
/// record is confirmed or cancelled (the inline Confirm/Cancel path).  The
/// `Msg::Commit`/`cancel` functions did this; the inline edit-form handlers
/// (helpers) must too, or the car stays "staged" after a manual run.
pub fn clear_after_confirm(model: crate::Model) {
    let sm = model.screens.stopwatch;
    sm.car.set(String::new());
    save_car("");
    sm.comment.set(String::new());
    save_comment("");
    sm.time.set(String::new());
    sm.feedback.set(None);
    penalty::clear(sm.penalty);
}

/// Compute runs remaining per car and for the unknown "?" car.
fn compute_runs_remaining(
    model: crate::Model,
    test: u8,
) -> (std::collections::HashMap<String, u8>, u8) {
    let (entries, runs_total) = model.khana.event.with(|e| {
        let total = e
            .stages
            .iter()
            .find(|s| s.num == test)
            .map(|s| s.runs_total)
            .unwrap_or(1);
        (e.entries.clone(), total)
    });
    let runs = model.khana.runs.with(|r| r.clone());
    let mut runs_remaining: std::collections::HashMap<String, u8> = entries
        .iter()
        .filter(|e| !e.car.is_empty())
        .map(|e| (e.car.clone(), runs_total))
        .collect();
    let mut unknown_finishes: usize = 0;
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
    (runs_remaining, unknown_remaining)
}

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

pub fn view(model: crate::Model) -> View {
    let sm = model.screens.stopwatch;
    view! {
        div {
            (move || {
                if sm.provisional_uid.with(|p| p.is_some()) {
                    view! {}
                } else {
                    view_action_buttons(model)
                }
            })
            (move || {
                let fb = sm.feedback.get_clone();
                match fb {
                    Some(msg) => view! { p(class="help has-text-danger mb-1") { (msg) } },
                    None => view! {},
                }
            })
            (move || {
                if sm.provisional_uid.with(|p| p.is_some()) {
                    view! {}
                } else {
                    view! {
                        (view_comment(model))
                    }
                }
            })
            (move || {
                if sm.provisional_uid.with(|p| p.is_some()) {
                    view! {}
                } else {
                    view! {
                        (view_car_chips(model))
                    }
                }
            })
            (crate::khana::helpers::view_timing_log(model, sm.test.get(), Some(sm.editing_observation), Some(sm.provisional_uid)))
            (view_car_picker_modal(model))
        }
    }
}

/// Selected car + merged Start/Stop + Manual icon — compact top row.
fn view_action_buttons(model: crate::Model) -> View {
    let sm = model.screens.stopwatch;
    view! {
        div(class="columns is-mobile is-gapless mt-3") {
            div(class="column is-narrow") {
                (move || {
                    let car = sm.car.get_clone();
                    let trimmed = car.trim().to_string();
                    let has_provisional = sm.provisional_uid.with(|p| p.is_some());
                    if trimmed.is_empty() {
                        return view! {
                            div(class="box kt-selected-car has-text-grey") {
                                span(class="icon is-small mr-1") { i(class="fa fa-car") }
                                "No car"
                            }
                        };
                    }
                    let test = sm.test.get();
                    let runs = model.khana.runs.get_clone();
                    let is_on_course = pending_for_car(&runs, test, &trimmed);
                    let (entry_name, entry_desc, entry_passenger, entry_shared, runs_total) =
                        model.khana.event.with(|e| {
                            let stage = e.stages.iter().find(|s| s.num == test);
                            let total = stage.map(|s| s.runs_total).unwrap_or(1);
                            let entry = e.entries.iter().find(|en| en.car == trimmed);
                            match entry {
                                Some(en) => (
                                    en.name.clone(),
                                    en.description.clone().unwrap_or_default(),
                                    en.passenger.clone().unwrap_or_default(),
                                    en.shared.clone().unwrap_or_default(),
                                    total,
                                ),
                                None => (String::new(), String::new(), String::new(), String::new(), total),
                            }
                        });
                    let finished = runs.iter()
                        .filter(|r| r.r#type == RUN_FINISH && r.test == test && r.car == trimmed && !r.voided)
                        .count() as u8;
                    let remaining = runs_total.saturating_sub(finished);
                    let run_number = finished + 1;
                    let at_max = remaining == 0;
                    let run_label = format!("{}/{}", run_number, runs_total);
                    let cls = if has_provisional || is_on_course {
                        "box kt-selected-car"
                    } else {
                        "box kt-selected-car is-clickable"
                    };
                    // Build entry info tags.
                    let mut info_tags: Vec<View> = Vec::new();
                    if !entry_name.is_empty() {
                        let n = entry_name.clone();
                        info_tags.push(view! { span(class="tag is-info is-light is-small") { (n) } });
                    }
                    if !entry_desc.is_empty() {
                        let d = entry_desc.clone();
                        info_tags.push(view! { span(class="tag is-info is-light is-small") { (d) } });
                    }
                    if !entry_passenger.is_empty() {
                        let p = entry_passenger.clone();
                        info_tags.push(view! { span(class="tag is-info is-light is-small") { ("P: ") (p) } });
                    }
                    if !entry_shared.is_empty() {
                        let s = entry_shared.clone();
                        info_tags.push(view! { span(class="tag is-link is-light is-small") { ("S: ") (s) } });
                    }
                    let info_view: View = info_tags.into();
                    let run_view = if at_max {
                        view! { span(class="tag is-warning is-small") {
                            span(class="icon is-small mr-1") { i(class="fa fa-triangle-exclamation") }
                            (run_label)
                        } }
                    } else {
                        view! { span(class="tag is-light is-small") { (run_label) } }
                    };
                    view! {
                        div(
                            class=cls,
                            on:click=move |_| {
                                if !has_provisional && !is_on_course {
                                    sm.car.set(String::new());
                                    save_car("");
                                }
                            },
                        ) {
                            div(class="tags are-small mb-1 is-centered") {
                                (crate::view::car_tag(&trimmed))
                            }
                            (info_view)
                            div(class="tags are-small mb-0 is-centered") {
                                (run_view)
                            }
                        }
                    }
                })
            }
            div(class="column is-narrow is-flex is-align-items-center") {
                (move || {
                    let car = sm.car.get_clone();
                    let has_provisional = sm.provisional_uid.with(|p| p.is_some());
                    let trimmed = car.trim();
                    let is_on_course = pending_for_car(
                        &model.khana.runs.get_clone(),
                        sm.test.get(),
                        trimmed,
                    );
                    let no_car = trimmed.is_empty();
                    // TBA "?" needs a comment before it can be timed.
                    let tba_blocked = trimmed == "?" && sm.comment.get_clone().trim().is_empty();
                    let blocked = has_provisional || no_car || tba_blocked;
                    // Show Stop when car is on course, Start otherwise.
                    if is_on_course && !has_provisional {
                        view! {
                            button(
                                class="button is-danger",
                                disabled=blocked,
                                on:click=move |_| update(model, Msg::Stop),
                            ) {
                                span(class="icon") { i(class="fa fa-flag-checkered") }
                                span { " Stop" }
                            }
                        }
                    } else {
                        let cls = if blocked {
                            "button"
                        } else {
                            "button is-success"
                        };
                        view! {
                            button(
                                class=cls,
                                disabled=blocked,
                                on:click=move |_| update(model, Msg::Start),
                            ) {
                                span(class="icon") { i(class="fa fa-flag-checkered") }
                                span { " Start" }
                            }
                        }
                    }
                })
            }
            div(class="column is-narrow is-flex is-align-items-center") {
                (move || {
                    let car = sm.car.get_clone();
                    let has_provisional = sm.provisional_uid.with(|p| p.is_some());
                    let trimmed = car.trim();
                    let no_car = trimmed.is_empty();
                    let tba_blocked = trimmed == "?" && sm.comment.get_clone().trim().is_empty();
                    if no_car || has_provisional || tba_blocked {
                        view! {
                            button(class="button", disabled=true) {
                                span(class="icon") { i(class="fa fa-keyboard") }
                            }
                        }
                    } else {
                        view! {
                            button(
                                class="button",
                                on:click=move |_| update(model, Msg::ManualTime),
                            ) {
                                span(class="icon") { i(class="fa fa-keyboard") }
                            }
                        }
                    }
                })
            }
        }
    }
}

/// Comment input — shown when a car is selected (and no confirm panel open).
/// For the TBA "?" car a comment is required and the field is highlighted;
/// Start/Stop/Manual stay disabled until it's filled (see `view_action_buttons`).
fn view_comment(model: crate::Model) -> View {
    let sm = model.screens.stopwatch;
    view! {
        (move || {
            let car = sm.car.get_clone();
            let trimmed = car.trim().to_string();
            if trimmed.is_empty() {
                return view! {};
            }
            let is_tba = trimmed == "?";
            let input_cls = if is_tba { "input is-warning" } else { "input" };
            let label = if is_tba { "Comment (required for TBA)" } else { "Comment" };
            view! {
                div(class="box is-hidden-print") {
                    div(class="field mb-0") {
                        label(class="label is-small") { (label) }
                        div(class="control") {
                            input(
                                class=input_cls,
                                r#type="text",
                                placeholder="Optional note",
                                bind:value=sm.comment,
                                on:input=move |_| {
                                    save_comment(&sm.comment.get_clone());
                                },
                            )
                        }
                    }
                }
            }
        })
    }
}

/// Compact car-number chips grouped by runs remaining, using car_tag style.
fn view_car_chips(model: crate::Model) -> View {
    use crate::event::cmp_car_number;
    let sm = model.screens.stopwatch;
    view! {
        div(class="box") {
            (move || {
                let entries = model.khana.event.with(|e| e.entries.clone());
                let test = sm.test.get();
                let (runs_remaining, _unknown_remaining) = compute_runs_remaining(model, test);

                struct CarInfo { car: String, remaining: u8 }
                let mut cars: Vec<CarInfo> = entries
                    .iter()
                    .filter(|e| !e.car.is_empty())
                    .map(|e| CarInfo {
                        car: e.car.clone(),
                        remaining: *runs_remaining.get(&e.car).unwrap_or(&0),
                    })
                    .collect();
                cars.sort_by(|a, b| {
                    b.remaining.cmp(&a.remaining)
                        .then_with(|| cmp_car_number(&a.car, &b.car))
                });

                let mut groups: std::collections::HashMap<u8, Vec<&CarInfo>> = std::collections::HashMap::new();
                for c in &cars {
                    groups.entry(c.remaining).or_default().push(c);
                }
                let mut group_keys: Vec<u8> = groups.keys().copied().collect();
                group_keys.sort_by(|a, b| b.cmp(a));

                let mut rows: Vec<View> = Vec::new();
                for remaining in group_keys {
                    let car_list = groups.remove(&remaining).unwrap();
                    let badge_label = format!("{remaining}r");
                    let car_views: Vec<View> = car_list.iter().map(|ci| {
                        let car_set = ci.car.clone();
                        let car_display = ci.car.clone();
                        view! {
                            button(
                                class="button is-light is-small",
                                on:click=move |_| {
                                    sm.car.set(car_set.clone());
                                    save_car(&car_set);
                                },
                            ) {
                                (crate::view::car_tag(&car_display))
                            }
                        }
                    }).collect();
                    rows.push(view! {
                        div(class="field is-grouped is-grouped-multiline is-align-items-center mb-1") {
                            span(class="tag is-small is-link is-light kt-runs-separator") { (badge_label) }
                            (car_views)
                        }
                    });
                }

                // Unknown chip at the end.
                rows.push(view! {
                    div(class="field is-grouped is-grouped-multiline is-align-items-center mb-1") {
                        span(class="tag is-small is-light kt-runs-separator") { "TBA" }
                        button(
                            class="button is-warning is-small",
                            on:click=move |_| {
                                sm.car.set("?".to_string());
                                save_car("?");
                            },
                        ) {
                            span(class="kt-car-tag has-text-weight-semibold") { "?" }
                        }
                    }
                });

                let view: View = rows.into();
                view
            })
        }
    }
}

/// Single pending confirm panel — replaces the old stacked view.
fn view_car_picker_modal(model: crate::Model) -> View {
    let sm = model.screens.stopwatch;
    view! {
        (move || {
            if !sm.show_car_picker.get_clone() {
                return view! {};
            }
            let entries = model.khana.event.with(|e| e.entries.clone());
            // Current car = the provisional record's car, or the record being
            // edited if it's a confirmed finish.
            let current_car = sm
                .provisional_uid
                .with(|uid| {
                    uid.as_ref().and_then(|u| {
                        model.khana.runs.with(|runs| {
                            runs.iter().find(|r| r.uid == *u).map(|r| r.car.clone())
                        })
                    })
                })
                .or_else(|| {
                    sm.editing_observation.with(|uid| {
                        uid.as_ref().and_then(|u| {
                            model.khana.runs.with(|runs| {
                                runs.iter().find(|r| r.uid == *u).map(|r| r.car.clone())
                            })
                        })
                    })
                })
                .unwrap_or_default();
            // Initialize selected_picker_car to the current car on open.
            if sm.selected_picker_car.with(|c| c.is_none()) {
                sm.selected_picker_car.set(Some(current_car.clone()));
            }
            let selected = sm.selected_picker_car.with(|c| c.clone().unwrap_or_default());
            let test = sm.test.get();
            let (runs_remaining, _unknown_remaining) = compute_runs_remaining(model, test);
            use crate::event::cmp_car_number;
            struct CarInfo { car: String, name: String, remaining: u8 }
            let mut cars: Vec<CarInfo> = entries
                .iter()
                .filter(|e| !e.car.is_empty())
                .map(|e| CarInfo {
                    car: e.car.clone(),
                    name: e.name.clone(),
                    remaining: *runs_remaining.get(&e.car).unwrap_or(&0),
                })
                .collect();
            cars.sort_by(|a, b| {
                b.remaining.cmp(&a.remaining)
                    .then_with(|| cmp_car_number(&a.car, &b.car))
            });
            let mut groups: std::collections::HashMap<u8, Vec<&CarInfo>> = std::collections::HashMap::new();
            for c in &cars {
                groups.entry(c.remaining).or_default().push(c);
            }
            let mut group_keys: Vec<u8> = groups.keys().copied().collect();
            group_keys.sort_by(|a, b| b.cmp(a));
            let mut rows: Vec<View> = Vec::new();
            for remaining in group_keys {
                let car_list = groups.remove(&remaining).unwrap();
                let badge_label = format!("{remaining}r");
                let car_views: Vec<View> = car_list.iter().map(|ci| {
                    let car_key = ci.car.clone();
                    let car_display = ci.car.clone();
                    let car_name = ci.name.clone();
                    let is_current = car_key == current_car;
                    let is_selected = car_key == selected;
                    let cls = if is_selected {
                        "button is-warning is-small"
                    } else if is_current {
                        "button is-link is-small"
                    } else {
                        "button is-light is-small"
                    };
                    let picker_sig = sm.selected_picker_car;
                    view! {
                        button(
                            class=cls,
                            on:click=move |_| {
                                picker_sig.set(Some(car_key.clone()));
                            },
                        ) {
                            (crate::view::car_tag(&car_display))
                            span(class="ml-1 is-size-7") { (car_name) }
                        }
                    }
                }).collect();
                rows.push(view! {
                    div(class="field is-grouped is-grouped-multiline is-align-items-center mb-1") {
                        span(class="tag is-small is-link is-light kt-runs-separator") { (badge_label) }
                        (car_views)
                    }
                });
            }
            // TBA chip
            let is_tba_selected = selected == "?";
            let is_tba_current = current_car == "?";
            let tba_cls = if is_tba_selected {
                "button is-warning is-small"
            } else if is_tba_current {
                "button is-link is-small"
            } else {
                "button is-light is-small"
            };
            let picker_sig = sm.selected_picker_car;
            rows.push(view! {
                div(class="field is-grouped is-grouped-multiline is-align-items-center mb-1") {
                    span(class="tag is-small is-light kt-runs-separator") { "TBA" }
                    button(
                        class=tba_cls,
                        on:click=move |_| {
                            picker_sig.set(Some("?".to_string()));
                        },
                    ) {
                        span(class="kt-car-tag has-text-weight-semibold") { "?" }
                    }
                }
            });
            let picker_content: View = rows.into();
            let close_picker = move || {
                sm.show_car_picker.set(false);
                sm.selected_picker_car.set(None);
            };
            let apply_car = {
                let close = close_picker;
                move || {
                    let Some(new_car) = sm.selected_picker_car.get_clone() else {
                        close();
                        return;
                    };
                    if let Some(uid) = sm.provisional_uid.get_clone() {
                        // Provisional (not yet sent): patch the run; Confirm
                        // sends it with the new car.
                        model.khana.runs.update(|runs| {
                            if let Some(r) = runs.iter_mut().find(|r| r.uid == uid) {
                                r.car = new_car;
                            }
                        });
                    } else if let Some(uid) = sm.editing_observation.get_clone() {
                        // Confirmed finish being edited: amend with the new car
                        // and close the edit form.
                        use crate::event::{KTime, KTimeTime};
                        let t = sm.edit_time.get_clone();
                        let f = sm.edit_flags.get();
                        let g = sm.edit_garage.get();
                        let st = sm.edit_status.get_clone();
                        let Some(time_ds) = crate::event::parse_time_ds(&t, &st) else {
                            show_feedback(model, "Enter a valid time in seconds");
                            close();
                            return;
                        };
                        let kt = if st == "dnf" {
                            KTime::DNF
                        } else if st == "fts" {
                            KTime::FTS
                        } else if st == "wd" {
                            KTime::WD
                        } else if st == "dns" {
                            KTime::NOSHO
                        } else {
                            KTime::Time(KTimeTime {
                                time_ds,
                                flags: f,
                                garage: g,
                            })
                        };
                        let c = sm.edit_comment.get_clone();
                        let comment_opt = if c.trim().is_empty() { None } else { Some(c) };
                        crate::khana::helpers::enqueue_amend(
                            model, &uid, sm.test.get(), &new_car, &kt, comment_opt,
                        );
                        sm.editing_observation.set(None);
                        sm.edit_uid.set(None);
                    }
                    close();
                }
            };
            view! {
                div(class="modal is-active") {
                    div(class="modal-background", on:click=move |_| close_picker())
                    div(class="modal-card") {
                        header(class="modal-card-head") {
                            p(class="modal-card-title") { "Change car" }
                            button(class="delete", aria-label="close", on:click=move |_| close_picker())
                        }
                        section(class="modal-card-body") {
                            (picker_content)
                        }
                        footer(class="modal-card-foot is-justify-content-center") {
                            button(
                                class="button is-link",
                                on:click=move |_| apply_car(),
                            ) { "Change" }
                            button(
                                class="button",
                                on:click=move |_| close_picker(),
                            ) { "Cancel" }
                        }
                    }
                }
            }
        })
    }
}
