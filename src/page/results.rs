use std::collections::HashSet;

use crate::event::*;
use crate::view as show;

// Results view.
// Render ResultView
// Model to sorting
// Change Class
use seed::{prelude::*, *};

pub enum Msg {
    SetEvent(String),
    Reload,
    SortStage,
    SortEvent,
    SortDriver,
    ShowClass(String),
}

pub struct Model {
    events: HashSet<String>, // names of known/stored events (local)
    results: Option<ResultView>,
    // event: String, // current event
    // class: String, // class to show
}

pub fn init(event: &String) -> Model {
    let events = crate::event::list_events();
    let mut model = Model {
        results: None,
        events,
    };
    load(&mut model, &event);
    model
}

fn load(model: &mut Model, name: &String) {
    let event = crate::event::load_event(name);
    let class = event.classes[0].clone();
    load_class(model, name, &class);
}

fn load_class(model: &mut Model, name: &String, class: &String) {
    let scores = crate::event::load_times(name);
    let event = crate::event::load_event(name);
    let results = create_result_view(&event, &scores, &class);
    model.results = Some(results);
    log!("loaded", name, class);
}

pub fn update(msg: Msg, model: &mut Model, _orders: &mut impl Orders<crate::Msg>) {
    match msg {
        Msg::SortStage => todo!(),
        Msg::SortEvent => todo!(),
        Msg::SortDriver => todo!(),
        Msg::ShowClass(class) => {
            //load is overkill... will do for moment.
            if let Some(results) = &model.results {
                let name = results.event.name.clone();
                load_class(model, &name, &class);
            }
        }
        Msg::SetEvent(name) => {
            load(model, &name);
        }
        Msg::Reload => {
            if let Some(results) = &model.results {
                let name = results.event.name.clone();
                let class = results.class.clone();
                load_class(model, &name, &class);
            }
        }
    }
}

pub fn view(model: &Model) -> Node<Msg> {
    if let Some(results) = &model.results {
        view_results(&results)
    } else {
        div![view_event_links(model)]
    }
}

fn view_event_links(model: &Model) -> Vec<Node<Msg>> {
    model
        .events
        .iter()
        .map(|event| button![format!("{}", event),])
        .collect()
}

fn view_results(results: &ResultView) -> Node<Msg> {
    let mut v = vec![];
    for rr in results.rows.values() {
        v.push(tr![
            td!(&rr.entry.car),
            td!(&rr.entry.name),
            td!(&"TBA"),
            rr.columns.iter().map(|rs| show_rs(&rs))
        ]);
    }

    div![
        clasess(results),
        div![
            C!["table-container"],
            table![C!["table is-bordered is-narrow "], table_header(results), v]
        ]
    ]
}

fn show_rs(rso: &Option<ResultScore>) -> Vec<Node<Msg>> {
    let cols_per_test = 5;
    match rso {
        Some(rs) => {
            // let t = Pos::default();
            let or: Pos = match &rs.cum_pos {
                Some(pos) => pos.clone(),
                None => Pos::default(),
            };
            nodes![
                td!(show::ktime(&rs.time)),
                td!(format!("{}", rs.stage_pos.score_ds as f32 / 10.0)),
                td!(format!("{}", rs.stage_pos.pos)),
                td!(format!("{}", or.score_ds as f32 / 10.0)),
                td!(format!("{}", or.pos)),
            ]
        }
        None => (1..=cols_per_test).map(|_| td![]).collect(),
    }
}

fn clasess(results: &ResultView) -> Vec<Node<Msg>> {
    results
        .event
        .classes
        .iter()
        .map(|class| {
            let class = class.to_owned();
            button![
                C!["button is-primary"],
                &class,
                ev(Ev::Click, |_| Msg::ShowClass(class))
            ]
        })
        .collect()
}

fn table_header(results: &ResultView) -> Vec<Node<Msg>> {
    let cols_per_test = 5;
    nodes![
        tr![
            th!["Entry", attrs! {At::ColSpan => 3}],
            (1..=results.event.stages_count).map(|stage| {
                th![
                    format!("Test {stage}"),
                    attrs! {At::ColSpan => cols_per_test,},
                ]
            }),
        ],
        //Time	Flags	Score	Pos	Total	Out
        tr![
            th!["#"],
            th!["Driver"],
            th!["O/R pos"],
            (1..=results.event.stages_count).map(|_| {
                nodes![
                    th!["Time"],
                    th!["Score"],
                    th!["Pos"],
                    th!["Cum"],
                    th!["O/R"],
                ]
            }),
        ],
    ]
}
