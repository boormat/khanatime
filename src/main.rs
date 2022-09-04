mod event;
mod page;

use event::EventInfo;
use indexmap::IndexMap;
use seed::{prelude::*, *};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const EVENT_PREFIX: &str = "EVENT:";

fn init(_: Url, _: &mut impl Orders<Msg>) -> Model {
    Model {
        page: Page::Event,
        events: list_events(),
        event: Default::default(),
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
    event: EventInfo,
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

    if !model.event.name.is_empty() {
        let key = format!("{}{}", EVENT_PREFIX, model.event.name);
        LocalStorage::insert(key, &model.event).expect("save data to LocalStorage");
    }
}

/// list of known events in storage.  String is storage key, is the event name
/// if it fails .. empty is fine
fn list_events() -> HashSet<String> {
    let len = LocalStorage::len().unwrap_or_default();
    let mut out: HashSet<String> = Default::default();
    // ugly it up with map?
    // out.push("dog".to_string());
    (0..len).for_each(|i| {
        if let Ok(name) = LocalStorage::key(i) {
            if name.starts_with(EVENT_PREFIX) {
                out.insert(name[EVENT_PREFIX.len()..].to_string());
            }
        }
    });
    return out;
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
            a![
                linky2(matches!(page, Page::Home)),
                "Home",
                ev(Ev::Click, |_| Msg::Show(Page::Home)),
            ],
            a![
                linky2(matches!(page, Page::Help)),
                "Help",
                ev(Ev::Click, |_| Msg::Show(Page::Help)),
            ],
            a![
                linky2(matches!(page, Page::KhanaRules)),
                "Rules",
                ev(Ev::Click, |_| Msg::Show(Page::KhanaRules)),
            ],
            a![
                linky2(matches!(page, Page::Stage)),
                "Timing",
                ev(Ev::Click, |_| Msg::Show(Page::Stage)),
            ],
            a![
                linky2(matches!(page, Page::Event)),
                "Event",
                ev(Ev::Click, |_| Msg::Show(Page::Event)),
            ],
            a![
                linky2(matches!(page, Page::Results)),
                "Results",
                ev(Ev::Click, |_| Msg::Show(Page::Results)),
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
