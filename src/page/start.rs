use sycamore::prelude::*;

use crate::event::{next_run, RunRecord, RUN_START};
use crate::page::pad;

// Big-button start timing: pick a test, pick a car, press START.  Records a
// `start` run (for pending-starts / run numbering) to the pending outbox.

#[derive(Clone)]
pub enum Msg {
    Test(u8),
    Start,
    Dns,
}

#[derive(Clone, Copy)]
pub struct Model {
    pub test: Signal<u8>,
    pub car: Signal<String>,
    pub feedback: Signal<Option<String>>,
}

pub fn init() -> Model {
    Model {
        test: create_signal(1),
        car: create_signal(String::new()),
        feedback: create_signal(None),
    }
}

pub fn update(model: crate::Model, msg: Msg) {
    match msg {
        Msg::Test(t) => model.screens.start.test.set(t),
        Msg::Dns => mark_dns(model),
        Msg::Start => start_car(model),
    }
}

fn start_car(model: crate::Model) {
    let car = model.screens.start.car.get_clone().trim().to_string();
    if car.is_empty() {
        model
            .screens
            .start
            .feedback
            .set(Some("Pick a car number".to_string()));
        return;
    }
    let test = model.screens.start.test.get();
    let run = model.app.runs.with(|runs| next_run(runs, test, &car));
    let record_run = RunRecord {
        uid: String::new(), // stamped at enqueue
        r#type: RUN_START.to_string(),
        test,
        car: car.clone(),
        run,
        ts: js_sys::Date::now() as i64,
        time_ds: None,
        status: Some("clean".to_string()),
        flags: None,
        official_id: Some(model.app.identity.get_clone()),
        voided: false,
    };
    crate::page::enqueue_run(model, &record_run);
    model.screens.start.feedback.set(None);
    model.screens.start.car.set(String::new());
}

/// Mark a car as a no-show for the test (a `dns` start + NOSHO score).
fn mark_dns(model: crate::Model) {
    let car = model.screens.start.car.get_clone().trim().to_string();
    if car.is_empty() {
        model
            .screens
            .start
            .feedback
            .set(Some("Pick a car number".to_string()));
        return;
    }
    let test = model.screens.start.test.get();
    let run = model.app.runs.with(|runs| next_run(runs, test, &car));
    let record_run = RunRecord {
        uid: String::new(), // stamped at enqueue
        r#type: RUN_START.to_string(),
        test,
        car: car.clone(),
        run,
        ts: js_sys::Date::now() as i64,
        time_ds: None,
        status: Some("dns".to_string()),
        flags: None,
        official_id: Some(model.app.identity.get_clone()),
        voided: false,
    };
    crate::page::enqueue_run(model, &record_run);
    // NOSHO score so the results cell reads "DNS".
    model.app.scores.update(|s| {
        crate::event::upsert_ktime(s, test, &car, crate::event::KTime::NOSHO);
    });
    crate::update(model, crate::Msg::Reload);
    model.screens.start.feedback.set(None);
    model.screens.start.car.set(String::new());
}

pub fn view(model: crate::Model) -> View {
    let sm = model.screens.start;
    let count = model.app.event.with(|e| e.stage_count());
    view! {
        div {
            h1(class="title is-4") { "Start timing" }
            (pad::test_chips(count as u8, sm.test))
            div(class="box") {
                h2(class="title is-5") { "Car" }
                (pad::keypad(sm.car, 4))
                div(class="mt-3") {
                    (move || {
                        let entries = model.app.event.with(|e| e.entries.clone());
                        pad::car_chips(entries, sm.car)
                    })
                }
                (move || match sm.feedback.get_clone() {
                    Some(f) => view! { p(class="help is-danger") { (f) } },
                    None => view! {},
                })
                div(class="field is-grouped") {
                    div(class="control is-expanded") {
                        button(
                            class="button is-success is-large is-fullwidth",
                            on:click=move |_| update(model, Msg::Start),
                        ) {
                            span(class="icon") { i(class="fa fa-play") }
                            span { "START" }
                        }
                    }
                    div(class="control") {
                        button(
                            class="button is-light is-large",
                            on:click=move |_| update(model, Msg::Dns),
                        ) {
                            "DNS"
                        }
                    }
                }
            }
            (view_last_starts(model))
        }
    }
}

fn view_last_starts(model: crate::Model) -> View {
    view! {
        div(class="box") {
            h2(class="title is-5") { "Recent starts" }
            (move || {
                let mut starts: Vec<RunRecord> = model.app.runs.with(|runs| {
                    runs.iter()
                        .filter(|r| r.r#type == RUN_START && r.status.as_deref() != Some("dns"))
                        .cloned()
                        .collect()
                });
                starts.sort_by_key(|r| std::cmp::Reverse(r.ts));
                starts.truncate(5);
                if starts.is_empty() {
                    return view! { p(class="help") { "No starts yet." } };
                }
                let views: Vec<View> = starts
                    .iter()
                    .map(|r| {
                        let label = format!("T{} #{} run {}", r.test, r.car, r.run);
                        let ts = fmt_ts(r.ts);
                        view! {
                            div(class="level") {
                                div(class="level-left") {
                                    span(class="has-text-weight-semibold") { (label) }
                                }
                                div(class="level-right") {
                                    span(class="has-text-grey") { (ts) }
                                }
                            }
                        }
                    })
                    .collect();
                let views: View = views.into();
                views
            })
        }
    }
}

fn fmt_ts(ms: i64) -> String {
    let d = js_sys::Date::new(&js_sys::Number::from(ms as f64).into());
    d.to_string().into()
}
