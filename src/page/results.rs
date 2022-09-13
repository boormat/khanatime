use std::collections::HashSet;

use crate::event::*;
use crate::view as show;

// Results view.
// Render ResultView
// Model to sorting
// Change Class
use seed::{prelude::*, *};

pub enum Msg {
    Reload,
    SortStage,
    SortEvent,
    SortDriver,
    ShowClass(String),
}

pub struct Model {
    events: HashSet<String>, // names of known/stored events (local)
    results: Option<ResultView>,
}

pub fn init(event: &EventInfo, scores: &Vec<ScoreData>) -> Model {
    let events = crate::event::list_events();
    let class = event.classes[0].clone();

    let results = Some(create_result_view(event, scores, &class));
    let submodel = Model { results, events };
    submodel
}

fn load_class(model: &mut crate::Model, class: &String) {
    let results = create_result_view(&model.event, &model.scores, &class);
    model.results_model.results = Some(results);
}

pub fn update(msg: Msg, model: &mut crate::Model, _orders: &mut impl Orders<crate::Msg>) {
    match msg {
        Msg::SortStage => todo!(),
        Msg::SortEvent => todo!(),
        Msg::SortDriver => todo!(),
        Msg::ShowClass(class) => {
            //load is overkill... will do for moment.
            if let Some(_results) = &model.results_model.results {
                load_class(model, &class);
            }
        }

        Msg::Reload => {
            if let Some(results) = &model.results_model.results {
                let class = results.class.clone();
                load_class(model, &class);
            }
        }
    }
}

pub fn view(model: &crate::Model) -> Node<Msg> {
    if let Some(results) = &model.results_model.results {
        view_results(&results)
    } else {
        div![view_event_links(&model.results_model)]
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
