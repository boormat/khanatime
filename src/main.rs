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
        page: Page::Stage,
        events: list_events(),
        event: Default::default(),
        ctx: Default::default(),
        stage_model: page::stage::init(),
    }
}

struct Model {
    ctx: Context,
    page: Page,
    #[allow(dead_code)]
    events: HashSet<String>, // names of known/stored events (local)
    event: EventInfo,
    stage_model: page::stage::StageModel,
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
    KhanaRules,
    NotFound,
    Stage,
    InEvent,
}

fn default_classes() -> Vec<String> {
    let classes = ["Outright", "Female", "Junior"];
    classes.map(String::from).into()
}

pub enum Msg {
    Show(Page),
    StageMsg(page::stage::StageMsg),
}

fn update(msg: Msg, model: &mut Model, _orders: &mut impl Orders<Msg>) {
    match msg {
        Msg::Show(p) => model.page = p,
        Msg::StageMsg(stage_msg) => page::stage::update(stage_msg, &mut model.stage_model),
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
    vec![
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
            Page::KhanaRules => page::khana_rule::view(),
            Page::NotFound => page::not_found::view(),
            Page::InEvent => span!("Oops"), //view_show_event(&model),
            Page::Stage => {
                page::stage::view(&model.stage_model).map_msg(Msg::StageMsg)
            }
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
                linky2(matches!(page, Page::KhanaRules)),
                "Rules",
                ev(Ev::Click, |_| Msg::Show(Page::KhanaRules)),
            ],
            a![
                linky2(matches!(page, Page::Stage)),
                "Stages",
                ev(Ev::Click, |_| Msg::Show(Page::Stage)),
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
