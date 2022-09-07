use std::collections::HashSet;

use crate::event::create_result_view;
use crate::event::load_times;
use crate::event::EventInfo;
use crate::event::KTime;
use crate::event::Pos;
use crate::event::ResultScore;
use crate::event::ResultView;
use crate::event::ScoreData;

// Results view.
// Render ResultView
// Model to sorting
// Change Class
use seed::{prelude::*, *};

pub enum Msg {
    SortStage,
    SortEvent,
    SortDriver,
    ShowClass(String),
}

pub struct Model {
    events: HashSet<String>, // names of known/stored events (local)
    results: Option<ResultView>,
}

pub fn init() -> Model {
    let name: String = match SessionStorage::get("event") {
        Ok(x) => x,
        Err(_) => "TBA".to_string(),
    };
    let events = crate::event::list_events();

    let mut model = Model {
        results: None,
        events,
    };
    load(&mut model, &name);
    model
}

fn load(model: &mut Model, name: &String) {
    let scores = crate::event::load_times(&name);
    let event = crate::event::load_event(&name);
    let class = event.classes[0].clone();
    let results = create_result_view(&event, &scores, &class);
    model.results = Some(results);
    log!("loaded", name, class);
}

pub fn update(msg: Msg, model: &mut Model) {
    match msg {
        Msg::SortStage => todo!(),
        Msg::SortEvent => todo!(),
        Msg::SortDriver => todo!(),
        Msg::ShowClass(class) => {
            //load is overkill... will do for moment.
            load(model, &class);
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
        table![C!["table is-bordered"], table_header(results), v]
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
                td!(format!("{}", rs.time)),
                td!(format!("{}", rs.stage_pos.score_ds as f32 / 10.0)),
                td!(format!("{}", rs.stage_pos.pos)),
                td!(format!("{}", or.score_ds as f32 / 10.0)),
                td!(format!("{}", or.pos)),
            ]
        }
        None => (1..=cols_per_test).map(|i| td![]).collect(),
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

fn view_time(score: &ScoreData) -> Node<Msg> {
    tr![
        td![score.stage.to_string()],
        td![view_car_number(&score.car)],
        td![view_time_score(&score.time)],
    ]
}

fn view_time_score(time: &KTime) -> Node<Msg> {
    log!(time.to_string());
    div!(time.to_string())
}

fn view_car_number(car: &String) -> Node<Msg> {
    span! {
        C!["label label-default"],
        car,
    }
}
