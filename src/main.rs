mod event;
mod input;
mod page;
mod view;

use event::{EventInfo, ScoreData};
use seed::{prelude::*, *};
use serde::{Deserialize, Serialize};

pub struct Model {
    ctx: Context,
    page: Page,
    pub scores: Vec<ScoreData>,
    pub event: EventInfo,
    pub stage_model: page::stage::StageModel,
    pub results_model: page::results::Model,
    pub event_model: page::event::Model,
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
    SetEvent(String), // new event name to load
    Reload,           // event or score data changed (in storage)
    StageMsg(page::stage::StageMsg),
    EventMsg(page::event::Msg),
    ResultMsg(page::results::Msg),
}

fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) {
    match msg {
        Msg::Show(Page::Results) => {
            model.page = Page::Results;
            let submsg = page::results::Msg::Reload;
            page::results::update(submsg, model, orders);
        }
        Msg::Show(p) => model.page = p,

        Msg::StageMsg(msg) => page::stage::update(msg, model, orders),
        Msg::EventMsg(msg) => page::event::update(msg, model, orders),
        Msg::ResultMsg(msg) => page::results::update(msg, model, orders),
        Msg::SetEvent(name) => {
            let scores = crate::event::load_times(&name);
            let event = crate::event::load_event(&name);
            model.scores = scores;
            model.event = event;
            SessionStorage::insert("event", &name).expect("save data to SessionStorage");
            page::results::update(page::results::Msg::Reload, model, orders);
        }
        Msg::Reload => {
            page::results::update(page::results::Msg::Reload, model, orders);
        }
    }
}

fn init(_: Url, _: &mut impl Orders<Msg>) -> Model {
    let event_name: String = match SessionStorage::get("event") {
        Ok(x) => x,
        Err(_) => "TBA".to_string(),
    };

    let scores = crate::event::load_times(&event_name);
    let event = crate::event::load_event(&event_name);

    Model {
        page: Page::Event,
        ctx: Default::default(),
        results_model: page::results::init(&event, &scores),
        scores: scores.clone(),
        event,
        stage_model: page::stage::init(),
        event_model: page::event::init(),
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
            Page::Stage => page::stage::view(&model).map_msg(Msg::StageMsg),
            Page::Results => page::results::view(&model).map_msg(Msg::ResultMsg),
            Page::Event => page::event::view(&model).map_msg(Msg::EventMsg),
        }
    ]
}

fn view_navbar(_user: Option<&User>, page: &Page) -> Node<Msg> {
    nav![
        C!["navbar", "is-link", "is-hidden-print"],
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

// ------ ------
//     Start
// ------ ------

fn main() {
    App::start("app", init, update, view);
}
