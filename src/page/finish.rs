use sycamore::prelude::*;

use crate::event::{elapsed_ds, pending_starts, upsert_ktime, KTime, RunRecord, RUN_FINISH};
use crate::page::{pad, penalty};

// Big-button finish timing: pending starts (tap to select), car chips,
// penalty chips, and a big FINISH button.  Pairs with the pending start when
// one exists; otherwise the elapsed time is typed in.

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Car,
    Time,
}

#[derive(Clone)]
pub enum Msg {
    Test(u8),
    SetMode(Mode),
    SelectPending(RunRecord),
    Finish,
}

#[derive(Clone, Copy)]
pub struct Model {
    pub test: Signal<u8>,
    pub car: Signal<String>,
    pub comment: Signal<String>,
    pub time: Signal<String>,
    pub mode: Signal<Mode>,
    pub penalty: penalty::PenaltyModel,
    pub feedback: Signal<Option<String>>,
}

pub fn init() -> Model {
    Model {
        test: create_signal(1),
        car: create_signal(String::new()),
        comment: create_signal(String::new()),
        time: create_signal(String::new()),
        mode: create_signal(Mode::Car),
        penalty: penalty::init(),
        feedback: create_signal(None),
    }
}

pub fn update(model: crate::Model, msg: Msg) {
    match msg {
        Msg::Test(t) => model.screens.finish.test.set(t),
        Msg::SetMode(m) => model.screens.finish.mode.set(m),
        Msg::SelectPending(r) => select_pending(model, r),
        Msg::Finish => do_finish(model),
    }
}

fn find_pending(model: crate::Model, test: u8, car: &str) -> Option<RunRecord> {
    model.khana.runs.with(|runs| {
        pending_starts(runs, test)
            .into_iter()
            .find(|r| r.car == car)
            .cloned()
    })
}

fn select_pending(model: crate::Model, r: RunRecord) {
    let sm = model.screens.finish;
    sm.test.set(r.test);
    sm.car.set(r.car.clone());
    sm.mode.set(Mode::Car);
    // Fill the time with the elapsed so it can be tweaked.
    sm.time
        .set((elapsed_ds(r.ts, js_sys::Date::now() as i64) as f32 / 10.0).to_string());
    sm.feedback.set(None);
}

/// Parse a decimal-seconds string to deciseconds: "45.25" -> 452.
fn time_to_ds(s: &str) -> u16 {
    s.trim()
        .parse::<f32>()
        .ok()
        .map(|v| (v * 10.0).round() as u16)
        .unwrap_or(0)
}

fn do_finish(model: crate::Model) {
    let sm = model.screens.finish;
    let test = sm.test.get();
    let car = sm.car.get_clone().trim().to_string();
    if car.is_empty() {
        sm.feedback.set(Some("Pick a car number".to_string()));
        return;
    }
    if car == "?" && sm.comment.get_clone().trim().is_empty() {
        sm.feedback
            .set(Some("Comment is required for unknown cars".to_string()));
        return;
    }
    let now = js_sys::Date::now() as i64;
    let pending = find_pending(model, test, &car);
    let time_ds = match &pending {
        Some(start) => elapsed_ds(start.ts, now),
        None => time_to_ds(&sm.time.get_clone()),
    };
    let ktime: KTime = penalty::to_ktime(sm.penalty, time_ds);
    let comment = sm.comment.get_clone();
    let comment_opt = if comment.trim().is_empty() {
        None
    } else {
        Some(comment)
    };
    let finish = RunRecord {
        uid: String::new(), // stamped at enqueue
        r#type: RUN_FINISH.to_string(),
        test,
        car: car.clone(),
        ts: now,
        time_ds: Some(time_ds),
        status: Some(sm.penalty.status.get_clone()),
        flags: Some(sm.penalty.flags.get()),
        official_id: Some(model.sync.identity.get_clone()),
        voided: false,
        comment: comment_opt,
        refs: vec![],
    };

    model.khana.scores.update(|s| {
        upsert_ktime(s, test, &car, ktime);
    });

    crate::page::enqueue_run(model, &finish);
    crate::update(model, crate::Msg::Reload);
    sm.car.set(String::new());
    sm.time.set(String::new());
    sm.comment.set(String::new());
    sm.feedback.set(None);
    penalty::clear(sm.penalty);
}

pub fn view(model: crate::Model) -> View {
    let sm = model.screens.finish;
    let count = model.khana.event.with(|e| e.stage_count());
    view! {
        div {
            h1(class="title is-4") { "Finish timing" }
            (pad::test_chips(count as u8, sm.test))
            (view_pending(model))
            div(class="box") {
                (move || {
                    let mode = sm.mode.get();
                    let is_time = mode == Mode::Time;
                    view! {
                        div(class="field has-addons") {
                            div(class="control is-expanded") {
                                button(
                                    class=format!("button is-fullwidth {}", if !is_time { "is-primary" } else { "is-light" }),
                                    on:click=move |_| update(model, Msg::SetMode(Mode::Car)),
                                ) { "Car number" }
                            }
                            div(class="control is-expanded") {
                                button(
                                    class=format!("button is-fullwidth {}", if is_time { "is-primary" } else { "is-light" }),
                                    on:click=move |_| update(model, Msg::SetMode(Mode::Time)),
                                ) { "Time" }
                            }
                        }
                    }
                })
                (move || {
                    let is_time = sm.mode.get() == Mode::Time;
                    if is_time {
                        view! {
                            h3(class="title is-6") { "Elapsed time (seconds)" }
                            input(class="input", r#type="text", placeholder="e.g. 45.25", bind:value=sm.time)
                        }
                    } else {
                        view! {
                            div(class="kt-car-chips mt-2") {
                                (move || {
                                    let entries = model.khana.event.with(|e| e.entries.clone());
                                    pad::car_chips(entries, sm.car)
                                })
                            }
                        }
                    }
                })
                (view_comment_input(model))
                (view_selected(model))
                (move || match sm.feedback.get_clone() {
                    Some(f) => view! { p(class="help is-danger") { (f) } },
                    None => view! {},
                })
                (move || {
                    let time_ds = resolved_time_ds(model);
                    penalty::view(model, sm.penalty, time_ds)
                })
                div(class="field mt-3") {
                    div(class="control") {
                        button(
                            class="button is-danger is-large is-fullwidth",
                            on:click=move |_| update(model, Msg::Finish),
                        ) {
                            span(class="icon") { i(class="fa fa-flag-checkered") }
                            span { "FINISH" }
                        }
                    }
                }
            }
            (crate::page::view_timing_log(model, sm.test.get()))
        }
    }
}

/// Elapsed for the current selection: auto from a pending start, else typed.
fn resolved_time_ds(model: crate::Model) -> u16 {
    let sm = model.screens.finish;
    let test = sm.test.get();
    let car = sm.car.get_clone().trim().to_string();
    match find_pending(model, test, &car) {
        Some(start) => elapsed_ds(start.ts, js_sys::Date::now() as i64),
        None => time_to_ds(&sm.time.get_clone()),
    }
}

fn view_comment_input(model: crate::Model) -> View {
    let sm = model.screens.finish;
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

fn view_pending(model: crate::Model) -> View {
    view! {
        div(class="box") {
            h2(class="title is-5") { "Pending starts" }
            (move || {
                let test = model.screens.finish.test.get();
                let pending = model.khana.runs.with(|runs| {
                    pending_starts(runs, test)
                        .into_iter()
                        .cloned()
                        .collect::<Vec<RunRecord>>()
                });
                if pending.is_empty() {
                    return view! { p(class="help") { "No cars out on course for this test." } };
                }
                let now = js_sys::Date::now() as i64;
                let views: Vec<View> = pending
                    .iter()
                    .map(|r| {
                        let rr = r.clone();
                        let disp = format!("#{}", r.car);
                        let age = fmt_age(now - r.ts);
                        let is_selected = r.car == model.screens.finish.car.get_clone();
                        view! {
                            button(
                                class=format!(
                                    "button is-fullwidth is-justify-content-space-between {}",
                                    if is_selected { "is-primary" } else { "is-light" }
                                ),
                                on:click=move |_| update(model, Msg::SelectPending(rr.clone())),
                            ) {
                                span { (disp) }
                                span(class="has-text-grey") { (age) }
                            }
                        }
                    })
                    .collect();
                views.into()
            })
        }
    }
}

fn view_selected(model: crate::Model) -> View {
    view! {
        (move || {
            let sm = model.screens.finish;
            let test = sm.test.get();
            let car = sm.car.get_clone().trim().to_string();
            if car.is_empty() {
                return view! {};
            }
            match find_pending(model, test, &car) {
                Some(start) => {
                    let ds = elapsed_ds(start.ts, js_sys::Date::now() as i64);
                    let age = fmt_age(js_sys::Date::now() as i64 - start.ts);
                    view! {
                        div(class="notification is-primary is-light") {
                            ("#") (car) (" — started ") (age) (" — elapsed ") (fmt_ds(ds)) (" s")
                        }
                    }
                }
                None => view! {
                    div(class="notification is-light") {
                        ("#") (car) (" — no pending start; enter the elapsed time or select one above.")
                    }
                },
            }
        })
    }
}

fn fmt_ds(ds: u16) -> String {
    format!("{:.2}", ds as f32 / 10.0)
}

fn fmt_age(ms_ago: i64) -> String {
    let s = ms_ago / 1000;
    if s < 60 {
        format!("{s}s ago")
    } else {
        format!("{}m {}s ago", s / 60, s % 60)
    }
}
