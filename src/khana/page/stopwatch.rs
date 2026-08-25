use sycamore::prelude::*;

use crate::event::{
    elapsed_ds, pending_for_car, pending_starts, upsert_ktime, RunRecord, RUN_FINISH, RUN_START,
    RUN_STOP,
};
use crate::khana::page::penalty;

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
    /// (pending_idx, attached_idx)
    ToggleAttach(usize, usize),
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
        Msg::ToggleAttach(pending_idx, attached_idx) => {
            toggle_attach(model, pending_idx, attached_idx)
        }
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

fn toggle_attach(model: crate::Model, pending_idx: usize, attached_idx: usize) {
    let sm = model.screens.stopwatch;
    sm.pending.update(|v| {
        if let Some(p) = v.get_mut(pending_idx) {
            if let Some(a) = p.attached.get_mut(attached_idx) {
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

/// Compute runs remaining per car and for the unknown "?" car.
fn compute_runs_remaining(
    model: crate::Model,
    test: u8,
) -> (std::collections::HashMap<String, u8>, u8) {
    let entries = model.khana.event.with(|e| e.entries.clone());
    let runs_total = model.khana.event.with(|e| {
        e.stages
            .iter()
            .find(|s| s.num == test)
            .map(|s| s.runs_total)
            .unwrap_or(1)
    });
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
                let has_pending = sm.pending.with(|v| !v.is_empty());
                if has_pending {
                    view! {}
                } else {
                    view! {
                        (view_car_chips(model))
                        (view_comment(model))
                        (view_manual_entry(model))
                    }
                }
            })
            (view_pending_stack(model))
            (crate::khana::helpers::view_timing_log(model, sm.test.get()))
        }
    }
}

/// Selected car (detailed chip) + START / STOP — compact top row.
fn view_action_buttons(model: crate::Model) -> View {
    let sm = model.screens.stopwatch;
    view! {
        div(class="columns is-mobile is-vcentered is-gapless mt-3") {
            div(class="column is-one-third") {
                (move || {
                    let car = sm.car.get_clone();
                    let trimmed = car.trim().to_string();
                    if trimmed.is_empty() {
                        return view! {
                            div(
                                class="notification is-light is-clickable has-text-grey kt-selected-car",
                                on:click=move |_| sm.car.set(String::new()),
                            ) { "No car" }
                        };
                    }
                    let test = sm.test.get();
                    let runs = model.khana.runs.get_clone();
                    let (entry_name, runs_total) =
                        model.khana.event.with(|e| {
                            let stage = e.stages.iter().find(|s| s.num == test);
                            let total = stage.map(|s| s.runs_total).unwrap_or(1);
                            let entry = e.entries.iter().find(|en| en.car == trimmed);
                            match entry {
                                Some(en) => (en.name.clone(), total),
                                None => (String::new(), total),
                            }
                        });
                    let finished = runs.iter()
                        .filter(|r| r.r#type == RUN_FINISH && r.test == test && r.car == trimmed && !r.voided)
                        .count() as u8;
                    let remaining = runs_total.saturating_sub(finished);
                    let run_number = finished + 1;
                    let at_max = remaining == 0;
                    let run_label = format!("{}/{}", run_number, runs_total);
                    view! {
                        div(
                            class="notification is-primary is-light is-clickable kt-selected-car",
                            on:click=move |_| sm.car.set(String::new()),
                        ) {
                            div(class="has-text-weight-semibold is-size-7") { (format!("#{}", trimmed)) }
                            div(class="is-size-7") { (entry_name) }
                            div(class="is-size-7 mt-1") {
                                (if at_max {
                                    view! { span(class="icon is-small has-text-warning mr-1") { i(class="fa fa-triangle-exclamation") } }
                                } else {
                                    view! {}
                                })
                                span { (run_label) }
                            }
                        }
                    }
                })
            }
            div(class="column is-one-third") {
                (move || {
                    let car = sm.car.get_clone();
                    let has = pending_for_car(
                        &model.khana.runs.get_clone(),
                        sm.test.get(),
                        car.trim(),
                    );
                    let start_cls = if has { "button is-light is-small is-fullwidth" } else { "button is-success is-small is-fullwidth" };
                    view! {
                        button(
                            class=start_cls,
                            on:click=move |_| update(model, Msg::Start),
                        ) {
                            span(class="icon is-small") { i(class="fa fa-flag-checkered") }
                            span { " Start" }
                        }
                    }
                })
            }
            div(class="column is-one-third") {
                (move || {
                    let car = sm.car.get_clone();
                    let has = pending_for_car(
                        &model.khana.runs.get_clone(),
                        sm.test.get(),
                        car.trim(),
                    );
                    let stop_cls = if has { "button is-danger is-small is-fullwidth" } else { "button is-light is-small is-fullwidth" };
                    view! {
                        button(
                            class=stop_cls,
                            on:click=move |_| update(model, Msg::Stop),
                        ) {
                            span(class="icon is-small") { i(class="fa fa-flag-checkered") }
                            span { " Stop" }
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
                                on:click=move |_| sm.car.set(car_set.clone()),
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
                            on:click=move |_| sm.car.set("?".to_string()),
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
                let car = p.car.clone();
                let time_ds = p.time_ds;
                let raw_str = format!("{:.1}s", time_ds as f32 / 10.0);
                let net = penalty::net_ds(time_ds, sm.penalty.flags.get(), sm.penalty.garage.get());
                let net_str = format!("{:.1}s", net as f32 / 10.0);
                let has_penalty = net > time_ds as u32;
                let status = sm.penalty.status.get_clone();
                let is_dns = status == "dnf" || status == "fts" || status == "wd";
                let time_display = if is_dns {
                    status.to_uppercase()
                } else if has_penalty {
                    format!("{} → {}", raw_str, net_str)
                } else {
                    raw_str
                };
                let (entry_name, entry_desc) = model.khana.event.with(|e| {
                    let entry = e.entries.iter().find(|en| en.car == car);
                    match entry {
                        Some(en) => (en.name.clone(), en.description.clone()),
                        None => (String::new(), None),
                    }
                });
                let desc_text = entry_desc.unwrap_or_default();
                let show_desc = !desc_text.is_empty();
                view! {
                    // Car + name + time → net
                    div(class="level is-mobile mb-1") {
                        div(class="level-left") {
                            (crate::view::car_tag(&car))
                            span(class="has-text-weight-semibold ml-2") { (entry_name) }
                            span(class="has-text-grey ml-2 is-size-7") { (time_display) }
                        }
                    }
                    (if show_desc {
                        let t = desc_text;
                        view! { div(class="has-text-grey is-size-7 mb-1 ml-4") { (t) } }
                    } else {
                        view! {}
                    })
                    // Status chips + Garage + Flags
                    (move || {
                        let status = sm.penalty.status.get_clone();
                        let on = sm.penalty.garage.get();
                        let flags = sm.penalty.flags.get();
                        let chips: Vec<View> = penalty::STATUS_CHIPS.iter().map(|(val, label, cls)| {
                            let val = *val;
                            let label = *label;
                            let cls = *cls;
                            let active = status == val;
                            view! {
                                button(
                                    class=format!("button is-small {}", if active { cls } else { "is-light" }),
                                    on:click=move |_| {
                                        if active {
                                            sm.penalty.status.set("clean".to_string());
                                        } else {
                                            sm.penalty.status.set(val.to_string());
                                        }
                                    },
                                ) { (label) }
                            }
                        }).collect();
                        let chips_view: View = chips.into();
                        view! {
                            div(class="level is-mobile mb-2") {
                                div(class="level-left") {
                                    (chips_view)
                                    button(
                                        class=format!("button is-small ml-2 {}", if on { "is-warning" } else { "is-light" }),
                                        on:click=move |_| sm.penalty.garage.set(!sm.penalty.garage.get()),
                                    ) {
                                        span(class="icon is-small") { i(class="fa fa-warehouse") }
                                    }
                                    div(class="buttons has-addons ml-2") {
                                        button(
                                            class="button is-small",
                                            disabled=flags == 0,
                                            on:click=move |_| sm.penalty.flags.update(|f| *f = f.saturating_sub(1)),
                                        ) { "\u{2212}" }
                                        span(class="button is-small is-static") {
                                            span(class="icon is-small has-text-warning") { i(class="fa fa-flag") }
                                            span { (flags) }
                                        }
                                        button(
                                            class="button is-small",
                                            disabled=flags >= 9,
                                            on:click=move |_| sm.penalty.flags.update(|f| *f += 1),
                                        ) { "+" }
                                    }
                                }
                            }
                        }
                    })
                    // Comment
                    div(class="field mb-2") {
                        input(class="input is-small", r#type="text", placeholder="Comment (required for #?)", bind:value=sm.comment)
                    }
                    // CONFIRM / Cancel
                    div(class="field is-grouped is-grouped-centered") {
                        div(class="control") {
                            button(
                                class="button is-success is-small",
                                on:click=move |_| update(model, Msg::Commit(idx)),
                            ) {
                                span(class="icon is-small") { i(class="fa fa-flag-checkered") }
                                span { " CONFIRM" }
                            }
                        }
                        div(class="control") {
                            button(
                                class="button is-light is-small",
                                on:click=move |_| update(model, Msg::Cancel(idx)),
                            ) { "Cancel" }
                        }
                    }
                    // Attached observations (bottom)
                    (view_attached_events(model, idx))
                }
            })
        }
    }
}

fn view_attached_events(model: crate::Model, idx: usize) -> View {
    let sm = model.screens.stopwatch;
    view! {
        div(class="mt-2") {
            (move || {
                let _now = model.tick.get();
                let now = js_sys::Date::now() as i64;
                let events = sm.pending.with(|v| {
                    v.get(idx)
                        .map(|p| p.attached.clone())
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
                                        class=format!("button is-small is-small {}", if is_attached { "is-light is-danger" } else { "is-light" }),
                                        on:click=move |_| {
                                            sycamore::reactive::untrack(move || {
                                                update(model, Msg::ToggleAttach(idx, i));
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
