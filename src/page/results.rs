use std::collections::HashSet;

use crate::event::*;
use crate::view as show;
use sycamore::prelude::*;

// Results view.
// Render ResultView
// Model to sorting
// Change Class

pub enum Msg {
    Reload,
    SortStage,
    SortEvent,
    SortDriver,
    ShowClass(String),
}

#[derive(Clone, Copy)]
pub struct Model {
    pub events: Signal<HashSet<String>>, // names of known/stored events (local)
    pub results: Signal<Option<ResultView>>,
}

pub fn init(event: &EventInfo, scores: &[ScoreData]) -> Model {
    let events = create_signal(crate::event::list_events());
    let class = event.classes[0].clone();
    let results = create_signal(Some(create_result_view(event, scores, &class)));
    Model { events, results }
}

fn load_class(model: crate::Model, class: &str) {
    let event = model.event.get_clone();
    let scores = model.scores.get_clone();
    let results = create_result_view(&event, &scores, class);
    model.results_model.results.set(Some(results));
}

pub fn update(model: crate::Model, msg: Msg) {
    match msg {
        Msg::SortStage => todo!(),
        Msg::SortEvent => todo!(),
        Msg::SortDriver => todo!(),
        Msg::ShowClass(class) => {
            load_class(model, &class);
        }

        Msg::Reload => {
            let class = model
                .results_model
                .results
                .with(|r| r.as_ref().map(|rv| rv.class.clone()));
            if let Some(class) = class {
                load_class(model, &class);
            }
        }
    }
}

pub fn view(model: crate::Model) -> View {
    view! {
        (move || {
            let results = model.results_model.results.with(|r| r.clone());
            match results {
                Some(results) => view_results(model, &results),
                None => view_event_links(model),
            }
        })
    }
}

fn view_event_links(model: crate::Model) -> View {
    let events = model.results_model.events.get_clone();
    let btns = events
        .iter()
        .map(|event| {
            let event = event.clone();
            view! { button { (event) } }
        })
        .collect::<Vec<View>>();
    view! { div { (btns) } }
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
    let stages_count = results.event.stages_count;
    let mut head: Vec<View> = vec![];
    head.push(view! {
        tr {
            th(colspan="3") { "Entry" }
            ((1..=stages_count)
                .map(|stage| view! { th(colspan="5") { (format!("Test {stage}")) } })
                .collect::<Vec<View>>())
        }
    });
    //Time	Flags	Score	Pos	Total	Out
    head.push(view! {
        tr {
            th { "#" }
            th { "Driver" }
            th { "O/R pos" }
            ((1..=stages_count)
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
