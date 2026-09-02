use sycamore::prelude::*;

use crate::event::TimingStyle;

/// Timing hub: stage picker → dispatches to stopwatch / start / finish.
///
/// On entering the timing page the official picks a stage.  The stage locks
/// until they leave — no switching positions mid-event.

#[derive(Clone)]
pub enum Msg {
    /// Select a stage and enter its timing view.
    EnterStage(u8),
    /// Leave the current stage and return to the stage list.
    LeaveStage,
}

#[derive(Clone, Copy)]
pub struct Model {
    /// `None` = stage list is showing.
    /// `Some(n)` = we are inside stage `n`.
    pub active_stage: Signal<Option<u8>>,
}

pub fn init() -> Model {
    let active_stage = create_signal(load_active_stage());
    Model { active_stage }
}

pub fn update(model: crate::Model, msg: Msg) {
    let tm = model.screens.timing;
    match msg {
        Msg::EnterStage(n) => {
            tm.active_stage.set(Some(n));
            save_active_stage(n);
            // Also set the stopwatch test signal so sub-views pick it up.
            model.screens.stopwatch.test.set(n);
            // Drop any stale provisional/edit state from a previous visit.
            crate::khana::page::stopwatch::reset_transient(model);
        }
        Msg::LeaveStage => {
            tm.active_stage.set(None);
            clear_active_stage();
            crate::khana::page::stopwatch::reset_transient(model);
        }
    }
}

fn save_active_stage(n: u8) {
    if let Some(st) = storage() {
        let _ = st.set_item("kt_active_stage", &n.to_string());
    }
}

fn load_active_stage() -> Option<u8> {
    storage()
        .and_then(|st| st.get_item("kt_active_stage").ok().flatten())
        .and_then(|s| s.parse().ok())
}

fn clear_active_stage() {
    if let Some(st) = storage() {
        let _ = st.remove_item("kt_active_stage");
    }
}

/// Drop any persisted stage selection and reset the stopwatch test.  Called
/// when the event is loaded/cleared/deleted so a stale stage from a previous
/// event can't stick (B12 follow-up).
pub fn reset_stage(model: crate::Model) {
    let tm = model.screens.timing;
    tm.active_stage.set(None);
    clear_active_stage();
    model.screens.stopwatch.test.set(1);
}

fn storage() -> Option<web_sys::Storage> {
    // The active stage is a "this visit" state: keep it across a refresh but
    // reset when the tab closes (sessionStorage), not a device-wide fact.
    web_sys::window()?.session_storage().ok().flatten()
}

pub fn view(model: crate::Model) -> View {
    let tm = model.screens.timing;
    let active = tm.active_stage.get_clone();
    if let Some(stage_num) = active {
        view_stage_view(model, stage_num)
    } else {
        view_stage_list(model)
    }
}

// ---------------------------------------------------------------------------
// Stage list
// ---------------------------------------------------------------------------

fn view_stage_list(model: crate::Model) -> View {
    let stages = model.khana.event.with(|e| e.stages.clone());
    let runs = model.khana.runs.with(|r| r.clone());
    let count = stages.len();
    let entry_count = model.khana.event.with(|e| e.entries.len());

    // Pre-compute owned data for each stage to avoid borrow issues in view closures
    let stage_data: Vec<(u8, String, TimingStyle, usize, usize)> = stages
        .iter()
        .enumerate()
        .map(|(i, stage)| {
            let num = (i + 1) as u8;
            let cars: std::collections::HashSet<String> = runs
                .iter()
                .filter(|r| r.test == num && !r.voided)
                .map(|r| r.car.clone())
                .collect();
            let cars_done = cars.len();
            (
                num,
                stage.name.clone(),
                stage.timing,
                cars_done,
                entry_count,
            )
        })
        .collect();

    let items: Vec<View> = stage_data
        .into_iter()
        .map(|(num, name, timing, cars_done, entry_count)| {
            let pct = cars_done
                .saturating_mul(100)
                .checked_div(entry_count)
                .unwrap_or(0)
                .min(100);
            let style_label = match timing {
                TimingStyle::Stopwatch => "Stopwatch",
                TimingStyle::Rally => "Rally",
            };
            let bar_class = if pct >= 100 {
                "is-success"
            } else if pct > 0 {
                "is-warning"
            } else {
                "is-light"
            };
            let name_display = if name.is_empty() {
                view! {}
            } else {
                view! { span(class="has-text-grey ml-2") { (format!("— {name}")) } }
            };

            view! {
                div(class="notification is-light mb-3") {
                    div(class="is-flex is-align-items-center is-justify-content-space-between") {
                        div {
                            p(class="has-text-weight-semibold is-size-5") {
                                (format!("Test {num}"))
                                (name_display)
                            }
                            p(class="is-size-7 has-text-grey") {
                                (style_label)
                                (format!(" · {}/{} cars done", cars_done, entry_count))
                            }
                        }
                        div(class="buttons is-small") {
                            (match timing {
                                TimingStyle::Stopwatch => {
                                    let n = num;
                                    view! {
                                        button(
                                            class="button is-primary",
                                            on:click=move |_| crate::update(model, crate::Msg::TimingMsg(Msg::EnterStage(n))),
                                        ) {
                                            span(class="icon") { i(class="fa fa-stopwatch") }
                                            span { "Time" }
                                        }
                                    }
                                }
                                TimingStyle::Rally => {
                                    let n = num;
                                    view! {
                                        button(
                                            class="button is-link",
                                            on:click=move |_| {
                                                crate::update(model, crate::Msg::TimingMsg(Msg::EnterStage(n)));
                                                crate::update(model, crate::Msg::StartMsg(crate::khana::page::start::Msg::Test(n)));
                                            },
                                        ) {
                                            span(class="icon is-small") { i(class="fa fa-flag") }
                                            span { "Start" }
                                        }
                                        button(
                                            class="button is-info",
                                            on:click=move |_| {
                                                crate::update(model, crate::Msg::TimingMsg(Msg::EnterStage(n)));
                                                crate::update(model, crate::Msg::FinishMsg(crate::khana::page::finish::Msg::Test(n)));
                                            },
                                        ) {
                                            span(class="icon is-small") { i(class="fa fa-flag-checkered") }
                                            span { "Finish" }
                                        }
                                    }
                                }
                            })
                        }
                    }
                    // Progress bar
                    div(class="progress-wrapper mt-2") {
                        progress(
                            class=format!("progress is-small {bar_class}"),
                            max="100",
                            value=pct.to_string(),
                        ) {}
                        p(class="is-size-7 has-text-grey") { (format!("{pct}%")) }
                    }
                }
            }
        })
        .collect();

    let stage_items: Vec<View> = if items.is_empty() {
        vec![
            view! { p(class="has-text-grey") { "No stages configured. Add stages in Event config." } },
        ]
    } else {
        items
    };
    view! {
        div {
            h1(class="title is-4") { "Timing" }
            p(class="subtitle is-6 has-text-grey") {
                (format!("{count} stage{}", if count == 1 { "" } else { "s" }))
            }
            (stage_items)
        }
    }
}

// ---------------------------------------------------------------------------
// Inside a stage — renders the appropriate sub-view
// ---------------------------------------------------------------------------

fn view_stage_view(model: crate::Model, stage_num: u8) -> View {
    let stages = model.khana.event.with(|e| e.stages.clone());
    let stage = stages.get((stage_num as usize) - 1);
    let style = stage.map(|s| s.timing).unwrap_or(TimingStyle::Stopwatch);
    let stage_name = stage.map(|s| s.name.clone()).unwrap_or_default();
    let name_display = if stage_name.is_empty() {
        view! {}
    } else {
        view! { span(class="has-text-grey ml-2 is-size-5") { (stage_name) } }
    };

    view! {
        div {
            div(class="is-flex is-align-items-center is-justify-content-space-between mb-4") {
                div {
                    h1(class="title is-4") {
                        (format!("Test {stage_num}"))
                        (name_display)
                    }
                }
                button(
                    class="button is-light",
                    on:click=move |_| crate::update(model, crate::Msg::TimingMsg(Msg::LeaveStage)),
                ) {
                    span(class="icon is-small") { i(class="fa fa-arrow-left") }
                    span { "Leave stage" }
                }
            }
            (match style {
                TimingStyle::Stopwatch => crate::khana::page::stopwatch::view(model),
                TimingStyle::Rally => {
                    // Rally: show start and finish views stacked
                    view! {
                        (crate::khana::page::start::view(model))
                        (crate::khana::page::finish::view(model))
                    }
                }
            })
        }
    }
}
