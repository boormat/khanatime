//! App shell: screen navigation + shared state.
//!
//! Shared, cross-screen state lives in [AppState]; each screen owns its own UI
//! state in [Screens]. Navigation goes through [update]/[show]; `enter` hooks
//! refresh a screen's data when it becomes current.

use crate::event::{EventInfo, RunRecord, ScoreData};
use crate::page;
use sycamore::prelude::*;

/// The top-level screens of the app.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Screen {
    #[default]
    Home,
    Events,
    Help,
    KhanaRules,
    Results,
    Stage,
    Start,
    Finish,
    Event,
    Entries,
    Chat,
}

impl Screen {
    /// Stable name for the URL hash (`#results`) and back/forward mapping.
    pub fn name(self) -> &'static str {
        match self {
            Screen::Home => "home",
            Screen::Events => "events",
            Screen::Help => "help",
            Screen::KhanaRules => "rules",
            Screen::Results => "results",
            Screen::Stage => "stage",
            Screen::Start => "start",
            Screen::Finish => "finish",
            Screen::Event => "event",
            Screen::Entries => "entries",
            Screen::Chat => "chat",
        }
    }

    pub fn from_name(name: &str) -> Option<Screen> {
        Some(match name {
            "home" => Screen::Home,
            "events" => Screen::Events,
            "help" => Screen::Help,
            "rules" => Screen::KhanaRules,
            "results" => Screen::Results,
            "stage" => Screen::Stage,
            "start" => Screen::Start,
            "finish" => Screen::Finish,
            "event" => Screen::Event,
            "entries" => Screen::Entries,
            "chat" => Screen::Chat,
            _ => return None,
        })
    }

    /// Screens that show a loaded event; restoring them needs a session event.
    pub fn needs_event(self) -> bool {
        matches!(
            self,
            Screen::Results
                | Screen::Stage
                | Screen::Start
                | Screen::Finish
                | Screen::Event
                | Screen::Entries
                | Screen::Chat
        )
    }
}

/// Connection state shared across screens (set by the Home page / resume).
#[derive(Clone, PartialEq)]
pub enum ConnState {
    Idle,
    Connecting,
    /// Waiting on the browser tab opened for the OAuth/SSO sign-in.
    SsoPending,
    LoggedIn(String),
    Error(String),
}

/// What a parcel export includes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ParcelMode {
    /// Full log: event setup + entries + timing (bootstrap / fresh device).
    #[default]
    Full,
    /// Timing messages only, for a receiver that already has the event.
    TimingOnly,
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
    /// QR-parcel handoff UI state (see `sync::export_parcel` / `import_parcel`).
    pub parcel_export: Signal<String>,
    pub parcel_import: Signal<String>,
    pub parcel_status: Signal<String>,
    /// Parcel for a different event: `(event id, name)` to offer open-and-import.
    pub parcel_open_event: Signal<Option<(String, String)>>,
    /// QR frames (SVG) of the exported parcel, one per displayed code.
    pub parcel_qr_svgs: Signal<Vec<String>>,
    /// Current frame shown when the exported parcel is animated across QRs.
    pub parcel_qr_index: Signal<usize>,
    /// Total QR frames of the exported parcel.
    pub parcel_qr_total: Signal<usize>,
    /// Camera scan session is live.
    pub scan_active: Signal<bool>,
    /// Status/feedback line for the scan panel.
    pub scan_status: Signal<String>,
    /// Which export variant to produce (full event vs timing-only).
    pub parcel_mode: Signal<ParcelMode>,
    /// The animated QR sequence is paused on its current frame.
    pub parcel_qr_paused: Signal<bool>,
}

/// Per-screen UI state. Kept alive across navigation so leaving and returning
/// preserves inputs, feeds, and connection details.
#[derive(Clone, Copy)]
pub struct Screens {
    pub home: page::home::Model,
    pub events: page::events::Model,
    pub setup: page::event::Model,
    pub stage: page::stage::StageModel,
    pub start: page::start::Model,
    pub finish: page::finish::Model,
    pub chat: page::chat::Model,
    pub results: page::results::Model,
    pub entries: page::entries::Model,
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
    Conn(crate::sync::Msg),
    StageMsg(page::stage::StageMsg),
    StartMsg(page::start::Msg),
    FinishMsg(page::finish::Msg),
    EventMsg(page::event::Msg),
    EventsMsg(page::events::Msg),
    ResultMsg(page::results::Msg),
    EntriesMsg(page::entries::Msg),
    /// Export the current event's log as a QR parcel.
    ExportParcel,
    /// Import a pasted/scanned QR parcel into the current event.
    ImportParcel,
    /// Open the event a mismatched parcel belongs to and import it there.
    OpenParcelEvent,
    /// Start the camera QR scanner.
    ScanStart,
    /// Stop the camera QR scanner.
    ScanStop,
    /// Pause/resume the animated QR export display.
    QrPauseToggle,
    /// Clear the QR export display.
    QrClear,
    /// Choose the parcel export variant (full event vs timing-only).
    SetParcelMode(ParcelMode),
}

impl Model {
    pub fn init() -> Model {
        let session_key = crate::event::session_event_name();
        // No real event selected yet: start with NO current event (empty id +
        // name) so the app shows the picker / sign-in screens instead of a
        // fabricated placeholder event.
        let (event_info, scores, runs) = if session_key.is_empty() {
            (
                EventInfo {
                    id: String::new(),
                    name: String::new(),
                    stages: vec![],
                    classes: vec![],
                    entries: vec![],
                    ..EventInfo::default()
                },
                vec![],
                vec![],
            )
        } else {
            crate::replay::replay(
                &crate::log::load_log(&session_key),
                &crate::log::load_pending(&session_key),
            )
        };
        let results = page::results::init(&event_info, &runs);

        let m = Model {
            screen: create_signal(Screen::Event),
            app: AppState {
                event: create_signal(event_info),
                scores: create_signal(scores),
                runs: create_signal(runs),
                identity: create_signal(String::new()),
                conn: create_signal(ConnState::Idle),
                room: create_signal(None),
                parcel_export: create_signal(String::new()),
                parcel_import: create_signal(String::new()),
                parcel_status: create_signal(String::new()),
                parcel_open_event: create_signal(None),
                parcel_qr_svgs: create_signal(Vec::new()),
                parcel_qr_index: create_signal(0),
                parcel_qr_total: create_signal(0),
                parcel_mode: create_signal(ParcelMode::default()),
                scan_active: create_signal(false),
                scan_status: create_signal(String::new()),
                parcel_qr_paused: create_signal(false),
            },
            screens: Screens {
                home: page::home::init(),
                events: page::events::init(),
                setup: page::event::init(),
                stage: page::stage::init(),
                start: page::start::init(),
                finish: page::finish::init(),
                chat: page::chat::init(),
                results,
                entries: page::entries::init(),
            },
        };
        refresh_feed(m);
        m
    }
}

/// Navigate to a screen, running per-screen setup effects on entry.
pub fn show(model: Model, screen: Screen) {
    #[cfg(target_arch = "wasm32")]
    push_screen_hash(screen);
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
            let (event, scores, runs) = crate::replay::replay(
                &crate::log::load_log(&name),
                &crate::log::load_pending(&name),
            );
            model.app.event.set(event.clone());
            model.app.scores.set(scores);
            model.app.runs.set(runs);
            crate::event::session_set_event(&name);
            model.screens.chat.expanded.set(Default::default());
            // Fresh event: reset any staged entry edits.
            model.screens.entries.staged.set(Vec::new());
            model.screens.entries.confirm.set(None);
            model.screens.entries.admin.set(false);
            model.screens.entries.show_form.set(false);
            // And any pending "open the parcel's event" offer.
            model.app.parcel_open_event.set(None);
            refresh_feed(model);
            page::event::update(model, page::event::Msg::LoadDetails);
            page::results::update(model, page::results::Msg::Reload);
            crate::sync::join_current_event(model);
        }

        Msg::Reload => {
            page::results::update(model, page::results::Msg::Reload);
        }

        Msg::StageMsg(msg) => page::stage::update(model, msg),
        Msg::StartMsg(msg) => page::start::update(model, msg),
        Msg::FinishMsg(msg) => page::finish::update(model, msg),
        Msg::EventMsg(msg) => page::event::update(model, msg),
        Msg::EventsMsg(msg) => page::events::update(model, msg),
        Msg::ResultMsg(msg) => page::results::update(model, msg),
        Msg::EntriesMsg(msg) => page::entries::update(model, msg),
        Msg::Conn(msg) => crate::sync::update(model, msg),
        Msg::ExportParcel => crate::sync::export_parcel(model),
        Msg::ImportParcel => crate::sync::import_parcel(model),
        Msg::OpenParcelEvent => crate::sync::open_parcel_event(model),
        Msg::ScanStart => {
            #[cfg(target_arch = "wasm32")]
            crate::qr_scan::start_scan(model);
            #[cfg(not(target_arch = "wasm32"))]
            let _ = model;
        }
        Msg::ScanStop => {
            model.app.scan_active.set(false);
            #[cfg(target_arch = "wasm32")]
            crate::qr_scan::stop_scan();
        }
        Msg::QrPauseToggle => crate::sync::toggle_qr_pause(model),
        Msg::QrClear => crate::sync::clear_qr(model),
        Msg::SetParcelMode(mode) => model.app.parcel_mode.set(mode),
    }
}

pub fn setup_effects(model: Model) {
    // stage command preview: re-parse whenever the input text changes
    create_effect(move || {
        let input = model.screens.stage.cmd.input.get_clone();
        let cmd = page::stage::parse_command(&input);
        model.screens.stage.preview.set(cmd);
    });
    #[cfg(target_arch = "wasm32")]
    listen_for_tab_sync(model);
    #[cfg(target_arch = "wasm32")]
    listen_for_history(model);
}

/// Screen named by the current URL hash, if it's one of ours (`#results` …).
#[cfg(target_arch = "wasm32")]
pub fn screen_from_url() -> Option<Screen> {
    let hash = web_sys::window()?.location().hash().ok()?;
    Screen::from_name(hash.strip_prefix('#')?)
}

/// Write the screen into the URL as a hash (`#stage`).  Uses pushState so a
/// history entry is created for back/forward, and no hashchange/popstate fires
/// (no echo loop with [listen_for_history]).
#[cfg(target_arch = "wasm32")]
fn push_screen_hash(screen: Screen) {
    let hash = format!("#{}", screen.name());
    let Some(window) = web_sys::window() else {
        return;
    };
    if window.location().hash().ok().as_deref() == Some(hash.as_str()) {
        return;
    }
    if let Ok(history) = window.history() {
        let _ = history.push_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&hash));
    }
}

/// Back/forward and manual hash edits: turn the URL back into a screen change
/// without pushing another history entry.
#[cfg(target_arch = "wasm32")]
fn listen_for_history(model: Model) {
    use wasm_bindgen::JsCast;
    let on_nav = move |_: web_sys::Event| {
        let Some(screen) = screen_from_url() else {
            return;
        };
        // A bare `#results` with no open event is meaningless; fall back.
        if screen.needs_event() && crate::event::session_event_name().is_empty() {
            return;
        }
        show(model, screen);
    };
    let cb = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(on_nav));
    let window = web_sys::window().expect("window exists");
    for event in ["popstate", "hashchange"] {
        let _ = window.add_event_listener_with_callback(event, cb.as_ref().unchecked_ref());
    }
    cb.forget();
}

/// Cross-tab sync.  Every write to this event's log/pending outbox lands in
/// shared localStorage, so a `storage` event in *this* tab means another tab
/// changed the current event.  Re-replay and refresh so both tabs agree — the
/// demo event never joins a room, so this is its only live sync path.  The
/// writing tab doesn't get the event, so there's no echo loop.
#[cfg(target_arch = "wasm32")]
fn listen_for_tab_sync(model: Model) {
    use wasm_bindgen::JsCast;
    let cb = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::StorageEvent)>::wrap(Box::new(
        move |ev: web_sys::StorageEvent| {
            let id = model.app.event.with(|e| e.id.clone());
            if id.is_empty() {
                return;
            }
            // Filter on the key for the event this tab currently has open.
            let key = ev.key().unwrap_or_default();
            if key != format!("log:{id}") && key != format!("pending:{id}") {
                return;
            }
            let (event, scores, runs) =
                crate::replay::replay(&crate::log::load_log(&id), &crate::log::load_pending(&id));
            model.app.event.set(event);
            model.app.scores.set(scores);
            model.app.runs.set(runs);
            crate::app::refresh_feed(model);
            crate::update(model, crate::Msg::Reload);
        },
    ));
    let window = web_sys::window().expect("window exists");
    let _ = window.add_event_listener_with_callback("storage", cb.as_ref().unchecked_ref());
    cb.forget();
}

// ------ ------
//     Log / feed helpers
// ------ ------

/// Rebuild the chat feed from the current event's stored log + pending, and
/// refresh the setup screen's "needs sync" flag (unsent setup manifest).
pub fn refresh_feed(model: Model) {
    let id = model.app.event.with(|e| e.id.clone());
    let log = crate::log::load_log(&id);
    let pending = crate::log::load_pending(&id);
    let mut feed: Vec<crate::page::chat::FeedEntry> = log
        .iter()
        .chain(pending.iter())
        .map(crate::page::chat::FeedEntry::from)
        .collect();
    feed.sort_by(|a, b| a.ts.cmp(&b.ts).then_with(|| a.mid.cmp(&b.mid)));
    model.screens.chat.feed.set(feed);
    // Only published+ events sync to a room, so only they can need a re-sync.
    let published = model
        .app
        .event
        .with(|e| e.status != crate::event::EventStatus::Draft);
    let has_unsent_setup = published
        && pending.iter().any(|m| {
            m.body
                .starts_with(crate::timing_event::TimingEvent::SETUP_PREFIX)
        });
    model.screens.setup.needs_sync.set(has_unsent_setup);
}

/// Enqueue a setup-manifest message for the current event (the durable record
/// of every edit) and refresh the feed.  Flushes to the room when connected.
pub fn enqueue_setup(model: Model) {
    let id = model.app.event.with(|e| e.id.clone());
    if id.is_empty() {
        return;
    }
    let ev = model.app.event.get_clone();
    let body = format!(
        "{}{}",
        crate::timing_event::TimingEvent::SETUP_PREFIX,
        serde_json::to_string(&ev).unwrap()
    );
    let sender = model.app.identity.get_clone();
    crate::log::enqueue_pending(&id, crate::log::LogMsg::new_pending(body, sender));
    refresh_feed(model);
    crate::sync::flush_pending(model);
}

// ------ ------
//     View
// ------ ------

pub fn view(model: Model) -> View {
    view! {
        div {
            (move || view_navbar(model))
            (move || view_content(model))
            (crate::page::view_handoff_modals(model))
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
        Screen::Chat,
        Screen::Entries,
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
                Screen::Events => page::events::view(model),
                Screen::Help => page::help::view(),
                Screen::KhanaRules => page::khana_rule::view(),
                Screen::Stage => page::stage::view(model),
                Screen::Start => page::start::view(model),
                Screen::Finish => page::finish::view(model),
                Screen::Results => page::results::view(model),
                Screen::Event => page::event::view(model),
                Screen::Entries => page::entries::view(model),
                Screen::Chat => page::chat::view(model),
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
        Screen::Chat,
        Screen::Entries,
    ];
    let mut brand: Vec<View> = vec![];
    for (screen, icon) in [
        (Screen::Home, "fa fa-home"),
        (Screen::Events, "fa fa-folder-open"),
        (Screen::Event, "fa fa-screwdriver-wrench"),
        (Screen::Start, "fa fa-flag"),
        (Screen::Finish, "fa fa-flag-checkered"),
        (Screen::Stage, "fa fa-stopwatch-20"),
        (Screen::Results, "fa fa-trophy"),
        (Screen::Entries, "fa fa-users"),
        (Screen::Chat, "fa fa-comments"),
        (Screen::Help, "fa fa-question"),
        (Screen::KhanaRules, "fa fa-book"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_name_round_trips() {
        let all = [
            Screen::Home,
            Screen::Events,
            Screen::Help,
            Screen::KhanaRules,
            Screen::Results,
            Screen::Stage,
            Screen::Start,
            Screen::Finish,
            Screen::Event,
            Screen::Entries,
            Screen::Chat,
        ];
        for screen in all {
            assert_eq!(Screen::from_name(screen.name()), Some(screen));
        }
        assert_eq!(Screen::from_name("bogus"), None);
    }
}
