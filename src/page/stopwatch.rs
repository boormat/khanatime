use sycamore::prelude::*;

use crate::event::{
    elapsed_ds, pending_for_car, pending_starts, upsert_ktime, RunRecord, RUN_FINISH, RUN_START,
};
use crate::page::{pad, penalty};

// Big-button stopwatch for cooperative timing: one official starts, another
// stops.  Both buttons always shown; the likely-correct one is highlighted
// from runs (which merge remote messages).  After STOP a penalty panel
// appears for confirm.  A manual time entry toggle is available for
// officials using physical stopwatches.

#[derive(Clone)]
pub enum Msg {
    Test(u8),
    Start,
    Stop,
    Commit,
    Cancel,
    ManualTime,
    ToggleManual,
    Void(String), // observation uid to void
}

#[derive(Clone, Copy)]
pub struct Model {
    pub test: Signal<u8>,
    pub car: Signal<String>,
    pub comment: Signal<String>,
    pub time: Signal<String>,
    pub show_manual: Signal<bool>,
    pub penalty: penalty::PenaltyModel,
    pub pending: Signal<Option<PendingFinish>>,
    pub feedback: Signal<Option<String>>,
}

#[derive(Clone)]
pub struct PendingFinish {
    pub car: String,
    pub elapsed_ds: u16,
}

pub fn init() -> Model {
    Model {
        test: create_signal(1),
        car: create_signal(String::new()),
        comment: create_signal(String::new()),
        time: create_signal(String::new()),
        show_manual: create_signal(false),
        penalty: penalty::init(),
        pending: create_signal(None),
        feedback: create_signal(None),
    }
}

pub fn update(model: crate::Model, msg: Msg) {
    match msg {
        Msg::Test(t) => model.screens.stopwatch.test.set(t),
        Msg::Start => start_car(model),
        Msg::Stop => stop_car(model),
        Msg::Commit => commit(model),
        Msg::Cancel => cancel(model),
        Msg::ManualTime => manual_time(model),
        Msg::ToggleManual => {
            let sm = model.screens.stopwatch;
            let show = sm.show_manual.get();
            sm.show_manual.set(!show);
        }
        Msg::Void(uid) => void_observation(model, &uid),
    }
}

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
    let car = if sm.car.get_clone().trim().is_empty() {
        "?".to_string()
    } else {
        sm.car.get_clone().trim().to_string()
    };
    let test = sm.test.get();
    if pending_for_car(&model.app.runs.get_clone(), test, &car) {
        sm.feedback
            .set(Some("Car is already on course — use STOP".to_string()));
        return;
    }
    let comment = sm.comment.get_clone();
    let comment_opt = if comment.trim().is_empty() {
        None
    } else {
        Some(comment)
    };
    let record = RunRecord {
        uid: String::new(), // stamped at enqueue
        r#type: RUN_START.to_string(),
        test,
        car: car.clone(),
        ts: js_sys::Date::now() as i64,
        time_ds: None,
        status: Some("clean".to_string()),
        flags: None,
        official_id: Some(model.app.identity.get_clone()),
        voided: false,
        comment: comment_opt,
        refs: vec![],
    };
    crate::page::enqueue_run(model, &record);
    sm.feedback.set(None);
    // Car stays selected for the start→finish cycle
}

fn stop_car(model: crate::Model) {
    let sm = model.screens.stopwatch;
    let car = if sm.car.get_clone().trim().is_empty() {
        "?".to_string()
    } else {
        sm.car.get_clone().trim().to_string()
    };
    let test = sm.test.get();
    let runs = model.app.runs.get_clone();
    let pending = pending_starts(&runs, test)
        .into_iter()
        .find(|r| r.car == car)
        .cloned();
    let now = js_sys::Date::now() as i64;
    let (finish_car, elapsed) = match pending {
        Some(start) => (start.car.clone(), elapsed_ds(start.ts, now)),
        None => (car.clone(), 0),
    };
    sm.pending.set(Some(PendingFinish {
        car: finish_car,
        elapsed_ds: elapsed,
    }));
    sm.feedback.set(None);
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
    let runs = model.app.runs.get_clone();
    let pending = pending_starts(&runs, test)
        .into_iter()
        .find(|r| r.car == car)
        .cloned();
    let finish_car = match &pending {
        Some(start) => start.car.clone(),
        None => car.clone(),
    };
    sm.pending.set(Some(PendingFinish {
        car: finish_car,
        elapsed_ds: elapsed,
    }));
    sm.time.set(String::new());
    sm.feedback.set(None);
}

fn commit(model: crate::Model) {
    let sm = model.screens.stopwatch;
    let pending = match sm.pending.get_clone() {
        Some(p) => p,
        None => return,
    };
    let now = js_sys::Date::now() as i64;
    let time_ds = pending.elapsed_ds;
    let ktime = penalty::to_ktime(sm.penalty, time_ds);
    let record = RunRecord {
        uid: String::new(),
        r#type: RUN_FINISH.to_string(),
        test: sm.test.get(),
        car: pending.car.clone(),
        ts: now,
        time_ds: Some(time_ds),
        status: Some(sm.penalty.status.get_clone()),
        flags: Some(sm.penalty.flags.get()),
        official_id: Some(model.app.identity.get_clone()),
        voided: false,
        comment: None,
        refs: vec![],
    };
    model.app.scores.update(|s| {
        upsert_ktime(s, sm.test.get(), &pending.car, ktime);
    });
    crate::page::enqueue_run(model, &record);
    sm.pending.set(None);
    sm.car.set(String::new()); // clear car after finish
    sm.comment.set(String::new());
    sm.feedback.set(None);
    penalty::clear(sm.penalty);
}

fn cancel(model: crate::Model) {
    let sm = model.screens.stopwatch;
    sm.pending.set(None);
    sm.feedback.set(None);
    penalty::clear(sm.penalty);
}

fn void_observation(model: crate::Model, uid: &str) {
    let sm = model.screens.stopwatch;
    let test = sm.test.get();
    let runs = model.app.runs.get_clone();
    let run = match runs.iter().find(|r| r.uid == uid) {
        Some(r) => r.clone(),
        None => return,
    };
    crate::page::enqueue_void(model, uid, test, &run.car);
    sm.feedback
        .set(Some(format!("Voided {} #{}", run.r#type, run.car)));
}

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

pub fn view(model: crate::Model) -> View {
    let sm = model.screens.stopwatch;
    let count = model.app.event.with(|e| e.stage_count());
    view! {
        div {
            h1(class="title is-4") { "Stopwatch" }
            (pad::test_chips(count as u8, sm.test))
            (view_car_input(model))
            (view_pending(model))
            (crate::page::view_timing_log(model, sm.test.get()))
        }
    }
}

/// Car chips + comment + manual time toggle + both action buttons.
fn view_car_input(model: crate::Model) -> View {
    let sm = model.screens.stopwatch;
    view! {
        div(class="box") {
            (move || {
                let entries = model.app.event.with(|e| e.entries.clone());
                pad::car_chips(entries, sm.car)
            })
            (view_comment_input(model))
            (move || match sm.feedback.get_clone() {
                Some(f) => view! { p(class="help is-danger") { (f) } },
                None => view! {},
            })
            (view_manual_time(model))
            (view_action_buttons(model))
        }
    }
}

/// Comment input, shown only when car is "?".
fn view_comment_input(model: crate::Model) -> View {
    let sm = model.screens.stopwatch;
    view! {
        (move || {
            if sm.car.get_clone().trim() == "?" {
                view! {
                    div(class="field mt-3") {
                        label(class="label is-size-7") { "Comment (required)" }
                        input(class="input", r#type="text", placeholder="e.g. blue sedan, maybe #12", bind:value=sm.comment)
                    }
                }
            } else {
                view! {}
            }
        })
    }
}

/// Manual time entry toggle: shows a time input when toggled on.
fn view_manual_time(model: crate::Model) -> View {
    let sm = model.screens.stopwatch;
    view! {
        div(class="mt-3") {
            (move || {
                let show = sm.show_manual.get();
                if show {
                    view! {
                        div(class="field has-addons") {
                            div(class="control is-expanded") {
                                input(class="input", r#type="text", placeholder="Elapsed seconds (e.g. 45.25)", bind:value=sm.time)
                            }
                            div(class="control") {
                                button(class="button is-primary", on:click=move |_| update(model, Msg::ManualTime)) { "Enter" }
                            }
                        }
                        a(class="has-text-grey is-size-7", on:click=move |_| update(model, Msg::ToggleManual)) { "Hide manual entry" }
                    }
                } else {
                    view! {
                        a(class="has-text-grey is-size-7", on:click=move |_| update(model, Msg::ToggleManual)) { "Enter time manually" }
                    }
                }
            })
        }
    }
}

/// Both START and STOP buttons, with the likely-correct one highlighted.
fn view_action_buttons(model: crate::Model) -> View {
    let sm = model.screens.stopwatch;
    view! {
        div(class="field is-grouped mt-3") {
            div(class="control is-expanded") {
                button(
                    class=move || {
                        let car = sm.car.get_clone();
                        let has_pending = pending_for_car(
                            &model.app.runs.get_clone(),
                            sm.test.get(),
                            car.trim(),
                        );
                        if has_pending {
                            "button is-light is-large is-fullwidth"
                        } else {
                            "button is-success is-large is-fullwidth"
                        }
                    },
                    on:click=move |_| update(model, Msg::Start),
                ) {
                    span(class="icon") { i(class="fa fa-play") }
                    span { "START" }
                }
            }
            div(class="control") {
                button(
                    class=move || {
                        let car = sm.car.get_clone();
                        let has_pending = pending_for_car(
                            &model.app.runs.get_clone(),
                            sm.test.get(),
                            car.trim(),
                        );
                        if has_pending {
                            "button is-danger is-large"
                        } else {
                            "button is-light is-large"
                        }
                    },
                    on:click=move |_| update(model, Msg::Stop),
                ) {
                    span(class="icon") { i(class="fa fa-stop") }
                    span { "STOP" }
                }
            }
        }
    }
}

/// Penalty panel + confirm / cancel, shown after STOP sets pending.
fn view_pending(model: crate::Model) -> View {
    let sm = model.screens.stopwatch;
    view! {
        (move || {
            let pending_opt = sm.pending.get_clone();
            match pending_opt {
                Some(p) => {
                    let disp = format!("{} — elapsed {:.2}s", p.car, p.elapsed_ds as f32 / 10.0);
                    view! {
                        div(class="box") {
                            h3(class="title is-6") { "Confirm finish" }
                            p(class="notification is-primary is-light") { (disp) }
                            (penalty::view(model, sm.penalty, p.elapsed_ds))
                            div(class="field is-grouped") {
                                div(class="control is-expanded") {
                                    button(
                                        class="button is-success is-large is-fullwidth",
                                        on:click=move |_| update(model, Msg::Commit),
                                    ) { span(class="icon") { i(class="fa fa-flag-checkered") } span { "CONFIRM" } }
                                }
                                div(class="control") {
                                    button(
                                        class="button is-light is-large",
                                        on:click=move |_| update(model, Msg::Cancel),
                                    ) { "Cancel" }
                                }
                            }
                        }
                    }
                }
                None => view! {},
            }
        })
    }
}
