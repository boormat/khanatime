//! App shell: screen navigation + shared state.
//!
//! Shared, cross-screen state lives in [AppState]; each screen owns its own UI
//! state in [Screens]. Navigation goes through [update]/[show]; `enter` hooks
//! refresh a screen's data when it becomes current.

use crate::event::{EventInfo, RunRecord, ScoreData};
use crate::page;
use sycamore::prelude::*;

/// The top-level screens of the app.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Screen {
    #[default]
    Home,
    Help,
    KhanaRules,
    Results,
    Stage,
    Start,
    Finish,
    Event,
    Sync,
}

/// Connection state shared across screens (set by the Sync screen / resume).
#[derive(Clone, PartialEq)]
pub enum ConnState {
    Idle,
    Connecting,
    LoggedIn(String),
    Error(String),
}

/// Global, cross-screen state. Screens read these but never own them.
#[derive(Clone, Copy)]
pub struct AppState {
    pub event: Signal<EventInfo>,
    pub scores: Signal<Vec<ScoreData>>,
    /// Start/finish run records for the current event (run numbering,
    /// pending-starts, live feeds).
    pub runs: Signal<Vec<RunRecord>>,
    /// User id of the logged-in Matrix account (empty when not connected).
    pub identity: Signal<String>,
    pub conn: Signal<ConnState>,
    /// Joined room id (the current event's timing room).
    pub room: Signal<Option<String>>,
}

/// Per-screen UI state. Kept alive across navigation so leaving and returning
/// preserves inputs, feeds, and connection details.
#[derive(Clone, Copy)]
pub struct Screens {
    pub setup: page::event::Model,
    pub stage: page::stage::StageModel,
    pub start: page::start::Model,
    pub finish: page::finish::Model,
    pub results: page::results::Model,
    pub sync: page::sync::SyncModel,
}

#[derive(Clone, Copy)]
pub struct Model {
    pub screen: Signal<Screen>,
    pub app: AppState,
    pub screens: Screens,
}

pub enum Msg {
    Show(Screen),
    SetEvent(String), // event id to load
    Reload,           // event or score data changed (in storage)
    StageMsg(page::stage::StageMsg),
    StartMsg(page::start::Msg),
    FinishMsg(page::finish::Msg),
    EventMsg(page::event::Msg),
    ResultMsg(page::results::Msg),
    SyncMsg(page::sync::Msg),
}

impl Model {
    pub fn init() -> Model {
        let session_key = crate::event::session_event_name();
        // No real event selected yet: start with NO current event (empty id +
        // name) so the app shows the picker / sign-in screens instead of a
        // fabricated placeholder event.
        let event_info = if session_key.is_empty() {
            EventInfo {
                id: String::new(),
                name: String::new(),
                stages: vec![],
                stages_count: 0,
                classes: vec![],
                entries: vec![],
                ..EventInfo::default()
            }
        } else {
            crate::event::load_event(&session_key)
        };
        crate::event::migrate_times_if_needed(&session_key, &event_info.id);
        let storage = crate::event::storage_key(&event_info);
        let scores = crate::event::load_times(&storage);
        let runs = crate::event::load_runs(&storage);
        let results = page::results::init(&event_info, &scores);

        Model {
            screen: create_signal(Screen::Event),
            app: AppState {
                event: create_signal(event_info),
                scores: create_signal(scores),
                runs: create_signal(runs),
                identity: create_signal(String::new()),
                conn: create_signal(ConnState::Idle),
                room: create_signal(None),
            },
            screens: Screens {
                setup: page::event::init(),
                stage: page::stage::init(),
                start: page::start::init(),
                finish: page::finish::init(),
                results,
                sync: page::sync::init(),
            },
        }
    }
}

/// Navigate to a screen, running per-screen setup effects on entry.
pub fn show(model: Model, screen: Screen) {
    model.screen.set(screen);
    match screen {
        Screen::Event => page::event::update(model, page::event::Msg::LoadDetails),
        Screen::Results => page::results::update(model, page::results::Msg::Reload),
        _ => {}
    }
}

pub fn update(model: Model, msg: Msg) {
    match msg {
        Msg::Show(screen) => show(model, screen),

        Msg::SetEvent(name) => {
            let event = crate::event::load_event(&name);
            crate::event::migrate_times_if_needed(&name, &event.id);
            let storage = crate::event::storage_key(&event);
            let scores = crate::event::load_times(&storage);
            let runs = crate::event::load_runs(&storage);
            model.app.scores.set(scores);
            model.app.runs.set(runs);
            model.app.event.set(event.clone());
            crate::event::session_set_event(&storage);
            page::event::update(model, page::event::Msg::LoadDetails);
            page::results::update(model, page::results::Msg::Reload);
            page::sync::join_current_event(model);
        }

        Msg::Reload => {
            let storage = model.app.event.with(crate::event::storage_key);
            model.app.scores.set(crate::event::load_times(&storage));
            model.app.runs.set(crate::event::load_runs(&storage));
            page::results::update(model, page::results::Msg::Reload);
        }

        Msg::StageMsg(msg) => page::stage::update(model, msg),
        Msg::StartMsg(msg) => page::start::update(model, msg),
        Msg::FinishMsg(msg) => page::finish::update(model, msg),
        Msg::EventMsg(msg) => page::event::update(model, msg),
        Msg::ResultMsg(msg) => page::results::update(model, msg),
        Msg::SyncMsg(msg) => page::sync::update(model, msg),
    }
}

pub fn setup_effects(model: Model) {
    // stage command preview: re-parse whenever the input text changes
    create_effect(move || {
        let input = model.screens.stage.cmd.input.get_clone();
        let cmd = page::stage::parse_command(&input);
        model.screens.stage.preview.set(cmd);
    });
}

// ------ ------
//     View
// ------ ------

pub fn view(model: Model) -> View {
    view! {
        div {
            (move || view_navbar(model))
            (move || view_content(model))
        }
    }
}

fn view_content(model: Model) -> View {
    let screen = model.screen.get();
    // Event-dependent screens need a current event; without one, fall back to
    // the Home (sign-in / event picker) view regardless of navigation.
    let needs_event = [
        Screen::Start,
        Screen::Finish,
        Screen::Stage,
        Screen::Results,
        Screen::Sync,
    ];
    let effective = if needs_event.contains(&screen) && model.app.event.with(|e| e.is_null()) {
        Screen::Home
    } else {
        screen
    };
    view! {
        div(class="container") {
            (match effective {
                Screen::Home => page::home::view(model),
                Screen::Help => page::help::view(),
                Screen::KhanaRules => page::khana_rule::view(),
                Screen::Stage => page::stage::view(model),
                Screen::Start => page::start::view(model),
                Screen::Finish => page::finish::view(model),
                Screen::Results => page::results::view(model),
                Screen::Event => page::event::view(model),
                Screen::Sync => page::sync::view(model),
            })
        }
    }
}

fn view_navbar(model: Model) -> View {
    let has_event = !model.app.event.with(|e| e.is_null());
    // Screens that need a current event: hidden/disabled until one is picked.
    // (Event itself stays enabled so the first event can be created.)
    let needs_event = [
        Screen::Start,
        Screen::Finish,
        Screen::Stage,
        Screen::Results,
        Screen::Sync,
    ];
    let mut brand: Vec<View> = vec![];
    for (screen, icon) in [
        (Screen::Home, "fa fa-home"),
        (Screen::Event, "fa fa-screwdriver-wrench"),
        (Screen::Start, "fa fa-flag"),
        (Screen::Finish, "fa fa-flag-checkered"),
        (Screen::Stage, "fa fa-stopwatch-20"),
        (Screen::Results, "fa fa-trophy"),
        (Screen::Help, "fa fa-question"),
        (Screen::KhanaRules, "fa fa-book"),
        (Screen::Sync, "fa fa-comments"),
    ] {
        let active = model.screen.get() == screen;
        let disabled = !has_event && needs_event.contains(&screen);
        let class = format!(
            "{icon} navbar-item has-text-weight-bold is-size-5{}",
            if active { " is-active" } else { "" },
        );
        let item_class = if disabled {
            format!("{class} has-text-grey-light")
        } else {
            class
        };
        if disabled {
            brand.push(view! { i(class=item_class) });
        } else {
            brand.push(view! {
                i(class=item_class, on:click=move |_| { update(model, Msg::Show(screen)) })
            });
        }
    }
    view! {
        nav(class="navbar is-link is-hidden-print", role="navigation", aria-label="main navigation") {
            div(class="navbar-brand") { (brand) }
        }
    }
}
