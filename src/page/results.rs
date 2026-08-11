use crate::event::*;
use crate::view as show;
use sycamore::prelude::*;

// Results view.
// Render ResultView
// Model to sorting
// Change Class

pub enum Msg {
    Reload,
    ShowClass(String),
    Publish,
}

#[derive(Clone, Copy)]
pub struct Model {
    pub results: Signal<ResultView>,
}

pub fn init(event: &EventInfo, scores: &[ScoreData]) -> Model {
    let class = event.classes.first().cloned().unwrap_or_default();
    let results = create_signal(create_result_view(event, scores, &class));
    Model { results }
}

fn load_class(model: crate::Model, class: &str) {
    let event = model.app.event.get_clone();
    let scores = model.app.scores.get_clone();
    let results = create_result_view(&event, &scores, class);
    model.screens.results.results.set(results);
}

pub fn update(model: crate::Model, msg: Msg) {
    match msg {
        Msg::ShowClass(class) => load_class(model, &class),
        Msg::Reload => {
            let class = model.screens.results.results.with(|r| r.class.clone());
            load_class(model, &class);
        }
        Msg::Publish => {
            #[cfg(target_arch = "wasm32")]
            publish(model);
            #[cfg(not(target_arch = "wasm32"))]
            let _ = model;
        }
    }
}

/// Broadcast a results snapshot to the timing room (audit trail; each publish
/// is a new message, older versions stay in history).
#[cfg(target_arch = "wasm32")]
fn publish(model: crate::Model) {
    let Some(room) = crate::services::matrix::room() else {
        return;
    };
    let event = model.app.event.get_clone();
    let scores = model.app.scores.get_clone();
    wasm_bindgen_futures::spawn_local(async move {
        if let Err(e) = crate::services::matrix::send_result(&room, &event, &scores).await {
            model.app.conn.set(crate::app::ConnState::Error(e));
        }
    });
}

pub fn view(model: crate::Model) -> View {
    view! {
        (view_publish(model))
        (move || {
            let results = model.screens.results.results.with(|r| r.clone());
            view_results(model, &results)
        })
    }
}

fn view_publish(model: crate::Model) -> View {
    let joined = model.app.room.with(|r| r.is_some());
    view! {
        div(class="box") {
            div(class="level") {
                div(class="level-left") {
                    div(class="level-item") {
                        h2(class="title is-5") { "Results" }
                    }
                }
                div(class="level-right") {
                    div(class="level-item") {
                        button(
                            class=format!("button {}", if joined { "is-primary" } else { "is-light" }),
                            disabled=!joined,
                            on:click=move |_| crate::update(model, crate::Msg::ResultMsg(Msg::Publish)),
                        ) {
                            span(class="icon") { i(class="fa fa-paper-plane") }
                            span { "Publish results" }
                        }
                    }
                }
            }
            p(class="help") { "Publishes a results snapshot to the timing room for the official record." }
        }
    }
}

fn view_results(model: crate::Model, results: &ResultView) -> View {
    let class_btns = clasess(model, results);
    let header = table_header(results);
    let rows = results.rows.values().map(view_row).collect::<Vec<View>>();

    view! {
        div {
            (class_btns)
            div(class="table-container") {
                table(class="table is-bordered is-narrow") {
                    (header)
                    (rows)
                }
            }
        }
    }
}

const COLS_PER_TEST: usize = 5;

fn view_row(rr: &ResultRow) -> View {
    let car = rr.entry.car.clone();
    let name = rr.entry.name.clone();
    let columns = rr.columns.iter().map(show_rs).collect::<Vec<View>>();
    view! {
        tr {
            td { (car) }
            td { (name) }
            td { "TBA" }
            (columns)
        }
    }
}

fn show_rs(rso: &Option<ResultScore>) -> View {
    match rso {
        Some(rs) => {
            // let t = Pos::default();
            let or: Pos = match &rs.cum_pos {
                Some(pos) => pos.clone(),
                None => Pos::default(),
            };
            let time = show::ktime(&rs.time);
            let stage_score = format!("{}", rs.stage_pos.score_ds as f32 / 10.0);
            let stage_pos = format!("{}", rs.stage_pos.pos);
            let cum_score = format!("{}", or.score_ds as f32 / 10.0);
            let cum_pos = format!("{}", or.pos);
            view! {
                td { (time) }
                td { (stage_score) }
                td { (stage_pos) }
                td { (cum_score) }
                td { (cum_pos) }
            }
        }
        None => {
            let tds = (0..COLS_PER_TEST)
                .map(|_| view! { td {} })
                .collect::<Vec<View>>();
            view! { (tds) }
        }
    }
}

fn clasess(model: crate::Model, results: &ResultView) -> View {
    let classes = results.event.classes.clone();
    let btns = classes
        .iter()
        .map(|class| {
            let class = class.clone();
            let class_disp = class.clone();
            view! {
                button(
                    class="button is-primary",
                    on:click=move |_| crate::update(model, crate::Msg::ResultMsg(Msg::ShowClass(class.clone()))),
                ) {
                    (class_disp)
                }
            }
        })
        .collect::<Vec<View>>();
    view! { (btns) }
}

fn table_header(results: &ResultView) -> View {
    let stages_count = results.event.stage_count();
    // Precompute the per-test labels (stage names, falling back to "Test N").
    let labels: Vec<String> = (0..stages_count)
        .map(|i| {
            let name = results.event.stage(i).name;
            if name.is_empty() {
                format!("Test {}", i + 1)
            } else {
                name
            }
        })
        .collect();
    let mut first_row: Vec<View> = vec![view! { th(colspan="3") { "Entry" } }];
    for label in labels {
        first_row.push(view! { th(colspan="5") { (label) } });
    }
    let mut head: Vec<View> = vec![];
    head.push(view! {
        tr {
            (first_row)
        }
    });
    //Time	Flags	Score	Pos	Total	Out
    head.push(view! {
        tr {
            th { "#" }
            th { "Driver" }
            th { "O/R pos" }
            ((0..stages_count)
                .map(|_| {
                    view! {
                        th { "Time" }
                        th { "Score" }
                        th { "Pos" }
                        th { "Cum" }
                        th { "O/R" }
                    }
                })
                .collect::<Vec<View>>())
        }
    });
    view! { (head) }
}
