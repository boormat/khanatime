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
    }
}

pub fn view(model: crate::Model) -> View {
    view! {
        (move || {
            let results = model.screens.results.results.with(|r| r.clone());
            view_results(model, &results)
        })
        (view_live_feed(model))
    }
}

// Compact copy of the Matrix live feed, so officials & competitors see times
// streaming in on the same screen as the standings.
fn view_live_feed(model: crate::Model) -> View {
    view! {
        div(class="box") {
            h2(class="title is-5") {
                "Live feed"
                span(class="tag is-light is-pulled-right") { "Matrix" }
            }
            (move || {
                let entries = model.screens.sync.feed.get_clone();
                if entries.is_empty() {
                    return view! {
                        p(class="help") {
                            "No messages yet. Connect and open an event to receive live times."
                        }
                    };
                }
                let views: Vec<View> = entries
                    .iter()
                    .rev()
                    .map(|e| {
                        let line = crate::page::sync::feed_line(e);
                        view! { div { pre { (line) } } }
                    })
                    .collect();
                views.into()
            })
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
