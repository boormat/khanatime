mod event;
mod input;
mod page;
mod services;
mod timing_event;
mod view;

use event::{EventInfo, ScoreData};
use sycamore::prelude::*;
use sycamore::render;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    Home,
    Help,
    KhanaRules,
    Results,
    Stage,
    Event,
    Sync,
}

pub enum Msg {
    Show(Page),
    SetEvent(String), // new event name to load
    Reload,           // event or score data changed (in storage)
    StageMsg(page::stage::StageMsg),
    EventMsg(page::event::Msg),
    ResultMsg(page::results::Msg),
    SyncMsg(page::sync::Msg),
}

#[derive(Clone, Copy)]
pub struct Model {
    pub page: Signal<Page>,
    pub scores: Signal<Vec<ScoreData>>,
    pub event: Signal<EventInfo>,
    pub stage_model: page::stage::StageModel,
    pub results_model: page::results::Model,
    pub event_model: page::event::Model,
    pub sync_model: page::sync::SyncModel,
}

impl Model {
    fn init() -> Model {
        let event_name = event::session_event_name();
        let scores = event::load_times(&event_name);
        let event_info = event::load_event(&event_name);
        let results_model = page::results::init(&event_info, &scores);

        Model {
            page: create_signal(Page::Event),
            scores: create_signal(scores),
            event: create_signal(event_info),
            stage_model: page::stage::init(),
            results_model,
            event_model: page::event::init(),
            sync_model: page::sync::init(),
        }
    }
}

pub fn update(model: Model, msg: Msg) {
    match msg {
        Msg::Show(Page::Results) => {
            model.page.set(Page::Results);
            let submsg = page::results::Msg::Reload;
            page::results::update(model, submsg);
        }
        Msg::Show(p) => model.page.set(p),

        Msg::StageMsg(msg) => page::stage::update(model, msg),
        Msg::EventMsg(msg) => page::event::update(model, msg),
        Msg::ResultMsg(msg) => page::results::update(model, msg),
        Msg::SyncMsg(msg) => page::sync::update(model, msg),
        Msg::SetEvent(name) => {
            let scores = event::load_times(&name);
            let event = event::load_event(&name);
            model.scores.set(scores);
            model.event.set(event);
            event::session_set_event(&name);
            page::results::update(model, page::results::Msg::Reload);
        }
        Msg::Reload => {
            page::results::update(model, page::results::Msg::Reload);
        }
    }
}

fn setup_effects(model: Model) {
    // stage command preview: re-parse whenever the input text changes
    create_effect(move || {
        let input = model.stage_model.cmd.input.get_clone();
        let cmd = page::stage::parse_command(&input);
        model.stage_model.preview.set(cmd);
    });
}

// ------ ------
//     View
// ------ ------

fn app(model: Model) -> View {
    view! {
        div {
            (move || view_navbar(model))
            (move || view_content(model))
        }
    }
}

// ----- view_content ------

fn view_content(model: Model) -> View {
    view! {
        div(class="container") {
            (match model.page.get() {
                Page::Home => page::home::view(),
                Page::Help => page::help::view(),
                Page::KhanaRules => page::khana_rule::view(),
                Page::Stage => page::stage::view(model),
                Page::Results => page::results::view(model),
                Page::Event => page::event::view(model),
                Page::Sync => page::sync::view(model),
            })
        }
    }
}

fn view_navbar(model: Model) -> View {
    let mut brand: Vec<View> = vec![];
    for (page, icon) in [
        (Page::Home, "fa fa-bars"),
        (Page::Event, "fa fa-screwdriver-wrench"),
        (Page::Stage, "fa fa-stopwatch-20"),
        (Page::Results, "fa fa-trophy"),
        (Page::Help, "fa fa-question"),
        (Page::KhanaRules, "fa fa-book"),
        (Page::Sync, "fa fa-comments"),
    ] {
        let active = model.page.get() == page;
        let class = format!(
            "navbar-item has-text-weight-bold is-size-5{}",
            if active { " is-active" } else { "" }
        );
        brand.push(view! {
            i(
                class=format!("{icon} {class}"),
                on:click=move |_| { update(model, Msg::Show(page)) },
            )
        });
    }
    view! {
        nav(class="navbar is-link is-hidden-print", role="navigation", aria-label="main navigation") {
            div(class="navbar-brand") { (brand) }
        }
    }
}

// ------ ------
//     Start
// ------ ------

fn main() {
    std::panic::set_hook(Box::new(|info| {
        let js_stack = js_sys::Reflect::get(&js_sys::Error::new("panic"), &"stack".into())
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_else(|| "no stack".to_string());
        let msg = format!("PANIC: {info}\nJS STACK:\n{js_stack}");
        khanatime::web_log(&msg);
        if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
            if let Some(body) = doc.body() {
                body.set_inner_html(&format!("<pre>{}</pre>", msg.replace('<', "&lt;")));
            }
        }
    }));
    render(move || {
        let model = Model::init();
        setup_effects(model);
        #[cfg(target_arch = "wasm32")]
        page::sync::resume_on_load(model);
        app(model)
    });
}
