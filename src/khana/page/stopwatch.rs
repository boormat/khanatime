use serde::{Deserialize, Serialize};
use sycamore::prelude::*;

use crate::event::{
    elapsed_ds, pending_for_car, pending_starts, upsert_ktime, KTime, KTimeTime, RunRecord,
    RUN_FINISH, RUN_START, RUN_STOP,
};
use crate::khana::page::penalty;

// Cooperative stopwatch: START sends a start event, STOP sends a stop event
// and opens a confirm panel with auto-attached observations, CONFIRM sends a
// finish event referencing the attached UIDs.
//
// Workflow: Select car → Start (or Manual) → Stop → Confirm penalties → Done.
// Only one pending finish at a time.

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub enum Msg {
    Test(u8),
    Start,
    Stop,
    /// Toggle attachment of an event in the pending confirm panel (attached_idx).
    ToggleAttach(usize),
    Commit,
    Cancel,
    /// Open the confirm panel with a manual time input (requires car selected).
    ManualTime,
    Void(String),
}

// ---------------------------------------------------------------------------
// Attached event (shown in confirm panel)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AttachedEvent {
    pub uid: String,
    pub r#type: String,
    pub ts: i64,
    pub car: String,
    pub attached: bool,
}

// ---------------------------------------------------------------------------
// Pending finish
// ---------------------------------------------------------------------------

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub enum PendingMode {
    Stopwatch,
    Manual,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PendingFinish {
    pub car: String,
    pub time_ds: u16,
    pub status: String,
    pub flags: u8,
    pub garage: bool,
    pub comment: String,
    pub attached: Vec<AttachedEvent>,
    pub mode: PendingMode,
}

impl PendingFinish {
    fn refs(&self) -> Vec<String> {
        self.attached
            .iter()
            .filter(|a| a.attached)
            .map(|a| a.uid.clone())
            .collect()
    }

    /// Build a KTime directly from this finish's penalty fields (no signal
    /// round-trip — safe for concurrent pending finishes).
    pub fn to_ktime(&self) -> KTime {
        match self.status.as_str() {
            "dns" => KTime::NOSHO,
            "dnf" => KTime::DNF,
            "fts" => KTime::FTS,
            "wd" => KTime::WD,
            _ => KTime::Time(KTimeTime {
                time_ds: self.time_ds,
                flags: self.flags,
                garage: self.garage,
            }),
        }
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
    pub penalty: penalty::PenaltyModel,
    pub pending: Signal<Option<PendingFinish>>,
    pub feedback: Signal<Option<String>>,
    pub show_car_picker: Signal<bool>,
    pub editing_observation: Signal<Option<String>>,
}

pub fn init() -> Model {
    Model {
        test: create_signal(1),
        car: create_signal(String::new()),
        comment: create_signal(String::new()),
        time: create_signal(String::new()),
        penalty: penalty::init(),
        pending: create_signal(None),
        feedback: create_signal(None),
        show_car_picker: create_signal(false),
        editing_observation: create_signal(None),
    }
}

pub fn update(model: crate::Model, msg: Msg) {
    match msg {
        Msg::Test(t) => model.screens.stopwatch.test.set(t),
        Msg::Start => start_car(model),
        Msg::Stop => stop_car(model),
        Msg::ToggleAttach(attached_idx) => toggle_attach(model, attached_idx),
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

fn start_car(model: crate::Model) {
    let sm = model.screens.stopwatch;
    let car = sm.car.get_clone().trim().to_string();
    if car.is_empty() {
        sm.feedback.set(Some("Select a car first".to_string()));
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
    let car = sm.car.get_clone().trim().to_string();
    if car.is_empty() {
        sm.feedback.set(Some("Select a car first".to_string()));
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

    let runs = model.khana.runs.get_clone();
    let attached = auto_attach(&runs, test, &car);
    set_pending(
        sm.pending,
        Some(PendingFinish {
            car,
            time_ds: elapsed,
            status: "clean".to_string(),
            flags: 0,
            garage: false,
            comment: String::new(),
            attached,
            mode: PendingMode::Stopwatch,
        }),
    );
    sm.feedback.set(None);
    sm.car.set(String::new());
    save_car("");
}

fn toggle_attach(model: crate::Model, attached_idx: usize) {
    let sm = model.screens.stopwatch;
    update_pending(sm.pending, |p| {
        if let Some(a) = p.attached.get_mut(attached_idx) {
            a.attached = !a.attached;
        }
    });
}

fn commit(model: crate::Model) {
    let sm = model.screens.stopwatch;
    let pending = sm.pending.with(|p| p.as_ref().cloned());
    let pending = match pending {
        Some(p) => p,
        None => return,
    };
    let now = js_sys::Date::now() as i64;
    let time_ds = pending.time_ds;
    let ktime = pending.to_ktime();
    let refs = pending.refs();
    let comment = sm.comment.get_clone();
    let car = pending.car;
    let record = RunRecord {
        uid: String::new(),
        r#type: RUN_FINISH.to_string(),
        test: sm.test.get(),
        car: car.clone(),
        ts: now,
        time_ds: Some(time_ds),
        status: Some(pending.status),
        flags: Some(pending.flags),
        official_id: Some(model.sync.identity.get_clone()),
        voided: false,
        comment: Some(comment).filter(|c| !c.is_empty()),
        refs,
    };
    model.khana.scores.update(|s| {
        upsert_ktime(s, sm.test.get(), &car, ktime);
    });
    crate::khana::helpers::enqueue_run(model, &record);
    set_pending(sm.pending, None);
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
    let pending = sm.pending.with(|p| p.as_ref().cloned());
    if let Some(p) = pending {
        let test = sm.test.get();
        for a in &p.attached {
            if a.attached {
                crate::khana::helpers::enqueue_void(model, &a.uid, test, &a.car);
            }
        }
    }
    set_pending(sm.pending, None);
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
    set_pending(
        sm.pending,
        Some(PendingFinish {
            car,
            time_ds: 0,
            status: "clean".to_string(),
            flags: 0,
            garage: false,
            comment: String::new(),
            attached: vec![],
            mode: PendingMode::Manual,
        }),
    );
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

fn save_pending(pending: &PendingFinish) {
    if let Some(st) = session_storage() {
        if let Ok(json) = serde_json::to_string(pending) {
            let _ = st.set_item("kt_sw_pending", &json);
        }
    }
}

fn load_pending() -> Option<PendingFinish> {
    session_storage()
        .and_then(|st| st.get_item("kt_sw_pending").ok().flatten())
        .and_then(|json| serde_json::from_str(&json).ok())
}

fn clear_pending_storage() {
    if let Some(st) = session_storage() {
        let _ = st.remove_item("kt_sw_pending");
    }
}

/// Set the pending finish and persist to sessionStorage.
fn set_pending(pending: Signal<Option<PendingFinish>>, p: Option<PendingFinish>) {
    if let Some(ref p) = p {
        save_pending(p);
    } else {
        clear_pending_storage();
    }
    pending.set(p);
}

/// Update the pending finish in-place and persist to sessionStorage.
fn update_pending(pending: Signal<Option<PendingFinish>>, f: impl FnOnce(&mut PendingFinish)) {
    pending.update(|p| {
        if let Some(pending) = p.as_mut() {
            f(pending);
            save_pending(pending);
        }
    });
}

/// On reload: restore car selection and pending finish from sessionStorage.
/// Merges persisted penalty/comment/mode with fresh auto-attached observations.
pub fn restore_session(model: crate::Model) {
    let sm = model.screens.stopwatch;
    let test = sm.test.get();
    let car = load_car();

    if let Some(mut saved) = load_pending() {
        // Re-run auto_attach to get fresh observations from the runs log.
        let runs = model.khana.runs.get_clone();
        let fresh = auto_attach(&runs, test, &saved.car);
        // Merge: keep user's attached toggles for known UIDs, default new ones.
        let merged: Vec<AttachedEvent> = fresh
            .into_iter()
            .map(|a| {
                let attached = saved
                    .attached
                    .iter()
                    .find(|s| s.uid == a.uid)
                    .map(|s| s.attached)
                    .unwrap_or(true);
                AttachedEvent { attached, ..a }
            })
            .collect();
        saved.attached = merged;
        set_pending(sm.pending, Some(saved));
    }
    if !car.is_empty() {
        sm.car.set(car);
    }
    let comment = load_comment();
    if !comment.is_empty() {
        sm.comment.set(comment);
    }
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
            (view_action_buttons(model))
            (move || {
                if sm.pending.with(|p| p.is_some()) {
                    view! {}
                } else {
                    view! {
                        (view_car_chips(model))
                    }
                }
            })
            (view_pending(model))
            (crate::khana::helpers::view_timing_log(model, sm.test.get(), Some(sm.editing_observation)))
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
                    let has_pending = sm.pending.with(|p| p.is_some());
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
                    let cls = if has_pending || is_on_course {
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
                                if !has_pending && !is_on_course {
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
                    let has_pending = sm.pending.with(|p| p.is_some());
                    let trimmed = car.trim();
                    let is_on_course = pending_for_car(
                        &model.khana.runs.get_clone(),
                        sm.test.get(),
                        trimmed,
                    );
                    let no_car = trimmed.is_empty();
                    // Show Stop when car is on course, Start otherwise.
                    if is_on_course && !has_pending {
                        view! {
                            button(
                                class="button is-danger",
                                disabled=has_pending || no_car,
                                on:click=move |_| update(model, Msg::Stop),
                            ) {
                                span(class="icon") { i(class="fa fa-flag-checkered") }
                                span { " Stop" }
                            }
                        }
                    } else {
                        let cls = if no_car || has_pending {
                            "button"
                        } else {
                            "button is-success"
                        };
                        view! {
                            button(
                                class=cls,
                                disabled=has_pending || no_car,
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
                    let has_pending = sm.pending.with(|p| p.is_some());
                    let trimmed = car.trim();
                    let no_car = trimmed.is_empty();
                    if no_car || has_pending {
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
fn view_pending(model: crate::Model) -> View {
    let sm = model.screens.stopwatch;
    view! {
        (move || {
            let pending = sm.pending.with(|p| p.as_ref().cloned());
            let p = match pending {
                Some(p) => p,
                None => return view! {},
            };
            let car = p.car.clone();
            let is_manual = p.mode == PendingMode::Manual;
            let time_ds = p.time_ds;
            let (entry_name, entry_desc) = model.khana.event.with(|e| {
                let entry = e.entries.iter().find(|en| en.car == car);
                match entry {
                    Some(en) => (en.name.clone(), en.description.clone()),
                    None => (String::new(), None),
                }
            });
            let desc_text = entry_desc.unwrap_or_default();
            let time_str = if is_manual {
                "manual".to_string()
            } else {
                format!("{:.1}s", time_ds as f32 / 10.0)
            };
            view! {
                div(class="box") {
                    // Car tag (clickable to open picker) + time
                    div(class="level is-mobile mb-1") {
                        div(class="level-left") {
                            div(
                                class="is-clickable",
                                on:click=move |_| sm.show_car_picker.set(true),
                            ) {
                                (crate::view::car_tag(&car))
                            }
                            span(class="has-text-weight-semibold ml-2") { (entry_name) }
                            span(class="has-text-grey ml-2 is-size-7") { (time_str) }
                        }
                    }
                    (if !desc_text.is_empty() {
                        let t = desc_text.clone();
                        view! { div(class="has-text-grey is-size-7 mb-1 ml-4") { (t) } }
                    } else {
                        view! {}
                    })
                    // Manual time input (shown when mode == Manual and time not yet set)
                    (if is_manual && p.time_ds == 0 {
                        view! {
                            div(class="field has-addons mb-2") {
                                div(class="control is-expanded") {
                                    input(
                                        class="input is-small",
                                        r#type="text",
                                        placeholder="Time in seconds",
                                        bind:value=sm.time,
                                    )
                                }
                                div(class="control") {
                                    button(
                                        class="button is-primary is-small",
                                        on:click=move |_| {
                                            let time_str = sm.time.get_clone();
                                            let elapsed = match time_str.trim().parse::<f32>() {
                                                Ok(v) if v > 0.0 => (v * 10.0).round() as u16,
                                                _ => {
                                                    sm.feedback
                                                        .set(Some("Enter a valid time in seconds".to_string()));
                                                    return;
                                                }
                                            };
                                            update_pending(sm.pending, |p| {
                                                p.time_ds = elapsed;
                                            });
                                            sm.time.set(String::new());
                                            sm.feedback.set(None);
                                        },
                                    ) { "Set" }
                                }
                            }
                        }
                    } else if is_manual {
                        // Manual time already set — show it
                        view! {}
                    } else {
                        view! {}
                    })
                    // Status chips + Garage + Flags (working on PendingFinish fields directly)
                    (move || {
                        let status = sm.pending.with(|p| p.as_ref().map(|pp| pp.status.clone()).unwrap_or_default());
                        let on = sm.pending.with(|p| p.as_ref().map(|pp| pp.garage).unwrap_or(false));
                        let flags = sm.pending.with(|p| p.as_ref().map(|pp| pp.flags).unwrap_or(0));
                        let time_ds = sm.pending.with(|p| p.as_ref().map(|pp| pp.time_ds).unwrap_or(0));
                        let is_manual = sm.pending.with(|p| p.as_ref().map(|pp| pp.mode == PendingMode::Manual).unwrap_or(false));
                        let mut chips: Vec<View> = penalty::STATUS_CHIPS.iter().map(|(val, label, cls)| {
                            let val = *val;
                            let label = *label;
                            let cls = *cls;
                            let active = status == val;
                            view! {
                                button(
                                    class=format!("button is-small {}", if active { cls } else { "is-light" }),
                                    on:click=move |_| {
                                        update_pending(sm.pending, |p| {
                                            if active {
                                                p.status = "clean".to_string();
                                            } else {
                                                p.status = val.to_string();
                                            }
                                        });
                                    },
                                ) { (label) }
                            }
                        }).collect();
                        if is_manual {
                            let active = status == "dns";
                            chips.push(view! {
                                button(
                                    class=format!("button is-small {}", if active { "is-warning" } else { "is-light" }),
                                    on:click=move |_| {
                                        update_pending(sm.pending, |p| {
                                            if active {
                                                p.status = "clean".to_string();
                                            } else {
                                                p.status = "dns".to_string();
                                            }
                                        });
                                    },
                                ) { "DNS" }
                            });
                        }
                        let chips_view: View = chips.into();
                        view! {
                            div(class="level is-mobile mb-2") {
                                div(class="level-left") {
                                    (chips_view)
                                    button(
                                        class=format!("button is-small ml-2 {}", if on { "is-warning" } else { "is-light" }),
                                    on:click=move |_| {
                                        update_pending(sm.pending, |p| {
                                            p.garage = !p.garage;
                                        });
                                    },
                                    ) {
                                        span(class="icon is-small") { i(class="fa fa-warehouse") }
                                    }
                                    div(class="buttons has-addons ml-2") {
                                        button(
                                            class="button is-small",
                                            disabled=flags == 0,
                                            on:click=move |_| {
                                                update_pending(sm.pending, |p| {
                                                    p.flags = p.flags.saturating_sub(1);
                                                });
                                            },
                                        ) { "\u{2212}" }
                                        span(class="button is-small is-static") {
                                            span(class="icon is-small has-text-warning") { i(class="fa fa-flag") }
                                            span { (flags) }
                                        }
                                        button(
                                            class="button is-small",
                                            disabled=flags >= 9,
                                            on:click=move |_| {
                                                update_pending(sm.pending, |p| {
                                                    p.flags += 1;
                                                });
                                            },
                                        ) { "+" }
                                    }
                                }
                            }
                            (move || {
                                let net = penalty::net_ds(time_ds, flags, on);
                                let is_terminal = status == "dns" || status == "dnf" || status == "fts" || status == "wd";
                                if is_terminal {
                                    let upper = status.to_uppercase();
                                    view! {
                                        div(class="notification is-warning is-light has-text-centered") {
                                            ("Result: ") (upper)
                                        }
                                    }
                                } else {
                                    let raw = format!("{:.1}", time_ds as f32 / 10.0);
                                    let net_str = format!("{:.1}", net as f32 / 10.0);
                                    let penalized = net > time_ds as u32;
                                    view! {
                                        div(class=if penalized {
                                            "notification is-warning is-light has-text-centered"
                                        } else {
                                            "notification is-success is-light has-text-centered"
                                        }) {
                                            ("Elapsed ") (raw) (" s → ") ("Net ") (net_str) (" s")
                                        }
                                    }
                                }
                            })
                        }
                    })
                    // Comment — highlighted when TBA (car == "?")
                    (move || {
                        let is_tba = sm.pending.with(|p| p.as_ref().map(|pp| pp.car.as_str()) == Some("?"));
                        let input_cls = if is_tba { "input is-small is-warning" } else { "input is-small" };
                        let placeholder = if is_tba { "Comment (required for TBA)" } else { "Comment (required for #?)" };
                        view! {
                            div(class="field mb-2") {
                                input(class=input_cls, r#type="text", placeholder=placeholder, bind:value=sm.comment)
                            }
                        }
                    })
                    // CONFIRM / Cancel
                    (move || {
                        let is_tba = sm.pending.with(|p| p.as_ref().map(|pp| pp.car.as_str()) == Some("?"));
                        let comment_empty = sm.comment.get_clone().trim().is_empty();
                        let confirm_disabled = is_tba && comment_empty;
                        view! {
                            div(class="field is-grouped is-grouped-centered") {
                                div(class="control") {
                                    button(
                                        class="button is-success is-small",
                                        disabled=confirm_disabled,
                                        on:click=move |_| update(model, Msg::Commit),
                                    ) {
                                        span(class="icon is-small") { i(class="fa fa-flag-checkered") }
                                        span { " CONFIRM" }
                                    }
                                }
                                div(class="control") {
                                    button(
                                        class="button is-light is-small",
                                        on:click=move |_| update(model, Msg::Cancel),
                                    ) { "Cancel" }
                                }
                            }
                        }
                    })
                    // Attached observations (bottom)
                    (view_attached_events(model))
                }
            }
        })
        // Car picker modal
        (move || {
            if !sm.show_car_picker.get_clone() {
                return view! {};
            }
            let entries = model.khana.event.with(|e| e.entries.clone());
            let current_car = sm.pending.with(|p| p.as_ref().map(|pp| pp.car.clone()).unwrap_or_default());
            // Group entries by runs remaining.
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
                    let car_set = ci.car.clone();
                    let car_display = ci.car.clone();
                    let car_name = ci.name.clone();
                    let is_active = car_set == current_car;
                    let cls = if is_active { "button is-link is-small" } else { "button is-light is-small" };
                    view! {
                        button(
                            class=cls,
                            on:click=move |_| {
                                update_pending(sm.pending, |p| { p.car = car_set.clone(); });
                                sm.show_car_picker.set(false);
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
            let is_tba = current_car == "?";
            let tba_cls = if is_tba { "button is-warning is-small" } else { "button is-light is-small" };
            rows.push(view! {
                div(class="field is-grouped is-grouped-multiline is-align-items-center mb-1") {
                    span(class="tag is-small is-light kt-runs-separator") { "TBA" }
                    button(
                        class=tba_cls,
                        on:click=move |_| {
                            update_pending(sm.pending, |p| { p.car = "?".to_string(); });
                            sm.show_car_picker.set(false);
                        },
                    ) {
                        span(class="kt-car-tag has-text-weight-semibold") { "?" }
                    }
                }
            });
            let picker_content: View = rows.into();
            view! {
                div(class="modal is-active") {
                    div(class="modal-background", on:click=move |_| sm.show_car_picker.set(false))
                    div(class="modal-card") {
                        header(class="modal-card-head") {
                            p(class="modal-card-title") { "Change car" }
                            button(class="delete", aria-label="close", on:click=move |_| sm.show_car_picker.set(false))
                        }
                        section(class="modal-card-body") {
                            (picker_content)
                        }
                    }
                }
            }
        })
    }
}

fn view_attached_events(model: crate::Model) -> View {
    let sm = model.screens.stopwatch;
    view! {
        div(class="mt-2") {
            (move || {
                let _now = model.tick.get();
                let now = js_sys::Date::now() as i64;
                let events = sm.pending.with(|p| {
                    p.as_ref()
                        .map(|pp| pp.attached.clone())
                        .unwrap_or_default()
                });
                if events.is_empty() {
                    return view! {};
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
                        let ts = crate::khana::helpers::fmt_ts(a.ts, now);
                        let strike = if is_attached { "" } else { "has-text-grey-light has-text-decoration-line-through" };
                        view! {
                            div(class="level is-mobile mb-0") {
                                div(class="level-left") {
                                    span(class="is-size-7") {
                                        span(class=icon_class) { (icon_char) }
                                        span(class=strike) { (format!(" {} {}", car, ts)) }
                                    }
                                }
                                div(class="level-right") {
                                    button(
                                        class=format!("button is-small {}", if is_attached { "is-light is-danger" } else { "is-light" }),
                                        on:click=move |_| {
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
