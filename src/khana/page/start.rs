use sycamore::prelude::*;

use crate::event::{RunRecord, RUN_START};
use crate::khana::page::pad;

// Big-button start timing: pick a car, press START.  Records a `start` run
// (for pending-starts / run numbering) to the pending outbox.

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
    pub comment: Signal<String>,
    pub feedback: Signal<Option<String>>,
}

pub fn init() -> Model {
    Model {
        test: create_signal(1),
        car: create_signal(String::new()),
        comment: create_signal(String::new()),
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
    let sm = model.screens.start;
    let car = sm.car.get_clone().trim().to_string();
    if car.is_empty() {
        sm.feedback.set(Some("Pick a car number".to_string()));
        return;
    }
    if crate::khana::helpers::check_unknown_comment(&car, &sm.comment.get_clone(), &sm.feedback) {
        return;
    }
    let test = sm.test.get();
    let comment = sm.comment.get_clone();
    let comment_opt = if comment.trim().is_empty() {
        None
    } else {
        Some(comment)
    };
    let record_run = RunRecord {
        uid: String::new(), // stamped at enqueue
        r#type: RUN_START.to_string(),
        test,
        car: car.clone(),
        ts: js_sys::Date::now() as i64,
        time_ds: None,
        status: Some("clean".to_string()),
        flags: None,
        official_id: Some(model.sync.identity.get_clone()),
        voided: false,
        comment: comment_opt,
        refs: vec![],
    };
    crate::khana::helpers::enqueue_run(model, &record_run);
    sm.feedback.set(None);
    sm.car.set(String::new());
    sm.comment.set(String::new());
}

/// Mark a car as a no-show for the test (a `dns` start + NOSHO score).
fn mark_dns(model: crate::Model) {
    let sm = model.screens.start;
    let car = sm.car.get_clone().trim().to_string();
    if car.is_empty() {
        sm.feedback.set(Some("Pick a car number".to_string()));
        return;
    }
    let test = sm.test.get();
    let record_run = RunRecord {
        uid: String::new(), // stamped at enqueue
        r#type: RUN_START.to_string(),
        test,
        car: car.clone(),
        ts: js_sys::Date::now() as i64,
        time_ds: None,
        status: Some("dns".to_string()),
        flags: None,
        official_id: Some(model.sync.identity.get_clone()),
        voided: false,
        comment: None,
        refs: vec![],
    };
    crate::khana::helpers::enqueue_run(model, &record_run);
    // NOSHO score so the results cell reads "DNS".
    model.khana.scores.update(|s| {
        crate::event::upsert_ktime(s, test, &car, crate::event::KTime::NOSHO);
    });
    crate::update(model, crate::Msg::Reload);
    sm.feedback.set(None);
    sm.car.set(String::new());
    sm.comment.set(String::new());
}

pub fn view(model: crate::Model) -> View {
    let sm = model.screens.start;
    let count = model.khana.event.with(|e| e.stage_count());
    view! {
        div {
            h1(class="title is-4") { "Start timing" }
            (pad::test_chips(count as u8, sm.test))
            div(class="box") {
                (move || {
                    let car = sm.car.get_clone();
                    if car.is_empty() {
                        view! {}
                    } else {
                        view! { p(class="mb-2") { (crate::view::car_tag(&car)) } }
                    }
                })
                div(class="kt-car-chips") {
                    (move || {
                        let entries = model.khana.event.with(|e| e.entries.clone());
                        pad::car_chips(entries, sm.car)
                    })
                }
                (view_comment_input(model))
                (move || match sm.feedback.get_clone() {
                    Some(f) => view! { p(class="help is-danger") { (f) } },
                    None => view! {},
                })
                div(class="field is-grouped mt-3") {
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
            (crate::khana::helpers::view_timing_log(model, sm.test.get(), None))
        }
    }
}

fn view_comment_input(model: crate::Model) -> View {
    let sm = model.screens.start;
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
