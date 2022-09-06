mod event;
mod page;

use seed::{prelude::*, *};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

// const EVENT_PREFIX: &str = "EVENT:";

fn init(_: Url, _: &mut impl Orders<Msg>) -> Model {
    Model {
        page: Page::Event,
        events: event::list_events(),
        ctx: Default::default(),
        stage_model: page::stage::init(),
        event_model: page::event::init(),
        results_model: page::results::init(),
    }
}

struct Model {
    ctx: Context,
    page: Page,
    #[allow(dead_code)]
    events: HashSet<String>, // names of known/stored events (local)
    stage_model: page::stage::StageModel,
    results_model: page::results::Model,
    event_model: page::event::Model,
}

#[derive(Default)]
struct Context {
    user: Option<User>,
}
#[derive(Deserialize)]
struct User {
    // name: String,
}

#[derive(Default, Serialize, Deserialize)]
pub enum Page {
    #[default]
    Home,
    Help,
    KhanaRules,
    Results,
    Stage,
    Event,
}

pub enum Msg {
    Show(Page),
    StageMsg(page::stage::StageMsg),
    EventMsg(page::event::Msg),
    ResultMsg(page::results::Msg),
}

fn update(msg: Msg, model: &mut Model, _orders: &mut impl Orders<Msg>) {
    match msg {
        Msg::Show(p) => model.page = p,
        Msg::StageMsg(msg) => page::stage::update(msg, &mut model.stage_model),
        Msg::EventMsg(msg) => page::event::update(msg, &mut model.event_model),
        Msg::ResultMsg(msg) => page::results::update(msg, &mut model.results_model),
    }
}
// ------ ------
//     View
// ------ ------

fn view(model: &Model) -> Vec<Node<Msg>> {
    nodes![
        view_navbar(model.ctx.user.as_ref(), &model.page),
        view_content(&model),
    ]
}

// ----- view_content ------

fn view_content(model: &Model) -> Node<Msg> {
    div![
        C!["container"],
        match model.page {
            Page::Home => page::home::view(),
            Page::Help => page::help::view(),
            Page::KhanaRules => page::khana_rule::view(),
            Page::Stage => page::stage::view(&model.stage_model).map_msg(Msg::StageMsg),
            Page::Results => page::results::view(&model.results_model).map_msg(Msg::ResultMsg),
            Page::Event => page::event::view(&model.event_model).map_msg(Msg::EventMsg),
        }
    ]
}

fn view_navbar(_user: Option<&User>, page: &Page) -> Node<Msg> {
    nav![
        C!["navbar", "is-link"],
        attrs! {
            At::from("role") => "navigation",
            At::AriaLabel => "main navigation",
        },
        div![
            C!["navbar-brand"],
            i![
                C!["fa fa-bars"],
                linky2(matches!(page, Page::Home)),
                ev(Ev::Click, |_| Msg::Show(Page::Home)),
            ],
            i![
                C!["fa fa-screwdriver-wrench"],
                linky2(matches!(page, Page::Event)),
                ev(Ev::Click, |_| Msg::Show(Page::Event)),
            ],
            i![
                C!["fa fa-stopwatch-20"],
                linky2(matches!(page, Page::Stage)),
                ev(Ev::Click, |_| Msg::Show(Page::Stage)),
            ],
            i![
                C!["fa fa-trophy"],
                linky2(matches!(page, Page::Results)),
                ev(Ev::Click, |_| Msg::Show(Page::Results)),
            ],
            i![
                C!["fa fa-question"],
                linky2(matches!(page, Page::Help)),
                ev(Ev::Click, |_| Msg::Show(Page::Help)),
            ],
            i![
                C!["fa fa-book"],
                linky2(matches!(page, Page::KhanaRules)),
                ev(Ev::Click, |_| Msg::Show(Page::KhanaRules)),
            ],
        ]
    ]
}

fn linky2(active: bool) -> Attrs {
    C![
        "navbar-item",
        "has-text-weight-bold",
        "is-size-5",
        IF!(active => "is-active"),
    ]
}

#[allow(dead_code)]
fn view_event_links(model: &Model) -> Node<Msg> {
    ul![
        C!["events"],
        model.events.iter().map(|name| { view_event_link(&name) })
    ]
}

#[allow(dead_code)]
fn view_event_link(name: &String) -> Node<Msg> {
    li![a![
        attrs! {
            At::Href => format!("/{}", name)
        },
        style! {St::Cursor => "pointer"},
        format!("{}", name)
    ]]
}

// ------ ------
//     Start
// ------ ------

fn main() {
    App::start("app", init, update, view);
}
