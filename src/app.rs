//! App shell: screen navigation + shared state.
//!
//! Shared, cross-screen state lives in [KhanaState] and [SyncState]; each screen owns its own UI
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
    Accounts,
    Help,
    KhanaRules,
    Results,
    Stage,
    Start,
    Finish,
    Event,
    Entries,
    Chat,
    Stopwatch,
}

impl Screen {
    /// Stable name for the URL hash (`#results`) and back/forward mapping.
    pub fn name(self) -> &'static str {
        match self {
            Screen::Home => "home",
            Screen::Events => "events",
            Screen::Accounts => "accounts",
            Screen::Help => "help",
            Screen::KhanaRules => "rules",
            Screen::Results => "results",
            Screen::Stage => "stage",
            Screen::Start => "start",
            Screen::Finish => "finish",
            Screen::Event => "event",
            Screen::Entries => "entries",
            Screen::Chat => "chat",
            Screen::Stopwatch => "stopwatch",
        }
    }

    pub fn from_name(name: &str) -> Option<Screen> {
        Some(match name {
            "home" => Screen::Home,
            "events" => Screen::Events,
            "accounts" => Screen::Accounts,
            "help" => Screen::Help,
            "rules" => Screen::KhanaRules,
            "results" => Screen::Results,
            "stage" => Screen::Stage,
            "start" => Screen::Start,
            "finish" => Screen::Finish,
            "event" => Screen::Event,
            "entries" => Screen::Entries,
            "chat" => Screen::Chat,
            "stopwatch" => Screen::Stopwatch,
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
                | Screen::Stopwatch
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

/// App-level UI mode: controls which screens appear in the navbar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Testing,
    Organiser,
    Spectator,
    Official,
    Competitor,
}

impl Mode {
    /// All modes in display order.
    pub const ALL: [Mode; 5] = [
        Mode::Testing,
        Mode::Organiser,
        Mode::Spectator,
        Mode::Official,
        Mode::Competitor,
    ];

    /// Human-readable label for the mode picker.
    pub fn label(self) -> &'static str {
        match self {
            Mode::Testing => "Testing",
            Mode::Organiser => "Organiser",
            Mode::Spectator => "Spectator",
            Mode::Official => "Official",
            Mode::Competitor => "Competitor",
        }
    }

    /// Screens visible in the navbar for this mode (canonical order).
    pub fn visible_screens(self) -> &'static [Screen] {
        use Screen::*;
        match self {
            Mode::Testing => &[
                Home, Events, Accounts, Help, KhanaRules, Results, Stage, Start, Finish, Event,
                Entries, Chat, Stopwatch,
            ],
            Mode::Organiser => &[
                Home, Events, Accounts, Event, Start, Finish, Stage, Stopwatch, Results, Entries,
                Chat, Help, KhanaRules,
            ],
            Mode::Spectator => &[Home, Results],
            Mode::Official => &[
                Home, Events, Start, Finish, Stage, Stopwatch, Results, Entries, Chat, Help,
                KhanaRules,
            ],
            Mode::Competitor => &[Home, Events, Results, Entries, Help, KhanaRules],
        }
    }

    /// Does this mode include the given screen?
    pub fn has_screen(self, screen: Screen) -> bool {
        self.visible_screens().contains(&screen)
    }

    /// Load from localStorage (`kt_mode`), defaulting to Competitor.
    pub fn from_storage() -> Self {
        let name = storage()
            .and_then(|st| st.get_item("kt_mode").ok().flatten())
            .unwrap_or_default();
        Self::ALL
            .iter()
            .copied()
            .find(|m| m.label() == name)
            .unwrap_or(Mode::Competitor)
    }

    /// Persist to localStorage.
    pub fn save(self) {
        if let Some(st) = storage() {
            let _ = st.set_item("kt_mode", self.label());
        }
    }
}

/// localStorage helper (same pattern as `event::storage()`).
fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

/// Khanacross domain state: event data, scores, and run records.
#[derive(Clone, Copy)]
pub struct KhanaState {
    pub event: Signal<EventInfo>,
    pub scores: Signal<Vec<ScoreData>>,
    /// Start/finish run records for the current event (run numbering,
    /// pending-starts, live feeds).
    pub runs: Signal<Vec<RunRecord>>,
}

/// Shared Matrix/QR infrastructure: connection, identity, parcel handoff.
#[derive(Clone, Copy)]
pub struct SyncState {
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
    /// A join invite waiting on login (public HS / SSO-only path).
    pub pending_join: Signal<Option<crate::event::Invite>>,
    /// The animated QR sequence is paused on its current frame.
    pub parcel_qr_paused: Signal<bool>,
}

/// Per-screen UI state. Kept alive across navigation so leaving and returning
/// preserves inputs, feeds, and connection details.
#[derive(Clone, Copy)]
pub struct Screens {
    pub home: page::home::Model,
    pub events: page::events::Model,
    pub accounts: page::accounts::Model,
    pub setup: crate::khana::page::event::Model,
    pub stage: crate::khana::page::stage::StageModel,
    pub start: crate::khana::page::start::Model,
    pub finish: crate::khana::page::finish::Model,
    pub stopwatch: crate::khana::page::stopwatch::Model,
    pub chat: page::chat::Model,
    pub results: crate::khana::page::results::Model,
    pub entry_app: crate::entry_app::Model,
}

/// Domain state for the entry app (independent of khanacross timing).
#[derive(Clone, Copy)]
pub struct EntryAppState {
    pub event: Signal<crate::entry_app::types::EntryEvent>,
}

#[derive(Clone, Copy)]
pub struct Model {
    pub screen: Signal<Screen>,
    pub mode: Signal<Mode>,
    pub khana: KhanaState,
    pub sync: SyncState,
    pub entry_app: EntryAppState,
    pub screens: Screens,
}

pub enum Msg {
    Show(Screen),
    SetMode(Mode),
    SetEvent(String), // event id to load
    Reload,           // event or score data changed (in storage)
    Conn(crate::sync::Msg),
    StageMsg(crate::khana::page::stage::StageMsg),
    StartMsg(crate::khana::page::start::Msg),
    FinishMsg(crate::khana::page::finish::Msg),
    StopwatchMsg(crate::khana::page::stopwatch::Msg),
    EventMsg(crate::khana::page::event::Msg),
    EventsMsg(page::events::Msg),
    ResultMsg(crate::khana::page::results::Msg),
    EntryAppMsg(crate::entry_app::Msg),
    /// Export the current event's log as a QR parcel.
    ExportParcel,
    /// Import a pasted/scanned QR parcel into the current event.
    ImportParcel,
    /// Open the event a mismatched parcel belongs to and import it there.
    OpenParcelEvent,
    /// Void a timing observation by uid (used by the shared timing log).
    VoidObservation(String),
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
    /// Join an event from a scanned invite link (connect + adopt).
    Join(crate::event::Invite),
    /// Join an event from a pasted invite URL (parsed on the Home page).
    JoinUrl,
    /// Create and open the local demo event.
    LoadDemo,
    /// Reset the demo event to its pristine template and open it.
    ResetDemo,
    /// Open an event saved on this device.
    OpenSaved(String),
    /// Delete a saved event from local storage (confirm is the caller's job).
    DeleteEvent(String),
    /// Close the current event and return to the no-event picker.
    ClearEvent,
    /// Import an account shared via URL QR.
    ImportAccount {
        homeserver: String,
        user_id: String,
        password: String,
    },
    /// Import a contact shared via URL QR.
    ImportContact {
        user_id: String,
        name: String,
        description: String,
        phone: Option<String>,
    },
}

impl Model {
    pub fn init() -> Model {
        // Migrate old kt_sync_sessions into the new homeservers/accounts model.
        #[cfg(target_arch = "wasm32")]
        crate::services::matrix::migrate_session_storage();

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
        let results = crate::khana::page::results::init(&event_info, &runs);

        let m = Model {
            screen: create_signal(Screen::Event),
            mode: create_signal(Mode::from_storage()),
            khana: KhanaState {
                event: create_signal(event_info),
                scores: create_signal(scores),
                runs: create_signal(runs),
            },
            sync: SyncState {
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
                pending_join: create_signal(None),
                scan_active: create_signal(false),
                scan_status: create_signal(String::new()),
                parcel_qr_paused: create_signal(false),
            },
            entry_app: EntryAppState {
                event: create_signal(crate::entry_app::types::EntryEvent::default()),
            },
            screens: Screens {
                home: page::home::init(),
                events: page::events::init(),
                accounts: page::accounts::init(),
                setup: crate::khana::page::event::init(),
                stage: crate::khana::page::stage::init(),
                start: crate::khana::page::start::init(),
                finish: crate::khana::page::finish::init(),
                stopwatch: crate::khana::page::stopwatch::init(),
                chat: page::chat::init(),
                results,
                entry_app: crate::entry_app::init(),
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
    if screen == Screen::Results {
        crate::khana::page::results::update(model, crate::khana::page::results::Msg::Reload);
    }
}

pub fn update(model: Model, msg: Msg) {
    match msg {
        Msg::Show(screen) => show(model, screen),
        Msg::SetMode(m) => {
            m.save();
            model.mode.set(m);
            let current = model.screen.get();
            if !m.has_screen(current) {
                show(model, Screen::Home);
            }
        }

        Msg::LoadDemo => {
            crate::event::ensure_demo();
            crate::update(
                model,
                Msg::SetEvent(crate::event::DEMO_EVENT_ID.to_string()),
            );
            crate::update(model, Msg::Show(Screen::Home));
        }
        Msg::ResetDemo => {
            crate::event::reset_demo();
            model
                .screens
                .home
                .refresh
                .set(model.screens.home.refresh.get() + 1);
            crate::update(
                model,
                Msg::SetEvent(crate::event::DEMO_EVENT_ID.to_string()),
            );
            crate::update(model, Msg::Show(Screen::Home));
        }
        Msg::OpenSaved(id) => {
            let e = crate::event::load_event(&id);
            // A published event connects like an invite (reuse account /
            // auto-register / SSO per its registration mode).
            if let Some(inv) = e.invite() {
                crate::update(model, Msg::Join(inv));
            } else {
                crate::update(model, Msg::SetEvent(id));
                crate::update(model, Msg::Show(Screen::Home));
            }
        }
        Msg::ClearEvent => {
            let current = model.khana.event.with(|e| e.id.clone());
            crate::event::session_set_recent(&current);
            crate::event::session_clear_event();
            model.khana.event.set(EventInfo {
                id: String::new(),
                name: String::new(),
                stages: vec![],
                classes: vec![],
                entries: vec![],
                ..EventInfo::default()
            });
            model.khana.scores.set(Vec::new());
            model.khana.runs.set(Vec::new());
            model.screens.chat.feed.set(Vec::new());
            model.sync.room.set(None);
            #[cfg(target_arch = "wasm32")]
            crate::services::matrix::set_room(None);
            crate::update(model, Msg::Show(Screen::Home));
        }
        Msg::DeleteEvent(id) => {
            crate::log::remove_event_log(&id);
            if model.khana.event.with(|e| e.id == id) {
                crate::event::session_clear_event();
                model.khana.event.set(EventInfo {
                    id: String::new(),
                    name: String::new(),
                    stages: vec![],
                    classes: vec![],
                    entries: vec![],
                    ..EventInfo::default()
                });
                model.khana.scores.set(Vec::new());
                model.khana.runs.set(Vec::new());
                model.screens.chat.feed.set(Vec::new());
                model.sync.room.set(None);
                model.sync.conn.set(crate::app::ConnState::Idle);
                #[cfg(target_arch = "wasm32")]
                crate::services::matrix::set_room(None);
            }
            model
                .screens
                .home
                .refresh
                .set(model.screens.home.refresh.get() + 1);
            crate::app::refresh_feed(model);
        }

        Msg::SetEvent(name) => {
            let (event, scores, runs) = crate::replay::replay(
                &crate::log::load_log(&name),
                &crate::log::load_pending(&name),
            );
            model.khana.event.set(event.clone());
            model.khana.scores.set(scores);
            model.khana.runs.set(runs);
            crate::event::session_set_event(&name);
            crate::event::session_set_recent(&name);
            model.screens.chat.expanded.set(Default::default());
            // Fresh event: reset any staged entry edits.
            model.screens.entry_app.staged.set(Vec::new());
            model.screens.entry_app.confirm.set(None);
            model.screens.entry_app.admin.set(false);
            model.screens.entry_app.show_form.set(false);
            // And any pending "open the parcel's event" offer.
            model.sync.parcel_open_event.set(None);
            refresh_feed(model);
            crate::khana::page::results::update(model, crate::khana::page::results::Msg::Reload);
            crate::sync::join_current_event(model);
        }

        Msg::Reload => {
            crate::khana::page::results::update(model, crate::khana::page::results::Msg::Reload);
        }

        Msg::StageMsg(msg) => crate::khana::page::stage::update(model, msg),
        Msg::StartMsg(msg) => crate::khana::page::start::update(model, msg),
        Msg::FinishMsg(msg) => crate::khana::page::finish::update(model, msg),
        Msg::StopwatchMsg(msg) => crate::khana::page::stopwatch::update(model, msg),
        Msg::EventMsg(msg) => crate::khana::page::event::update(model, msg),
        Msg::EventsMsg(msg) => page::events::update(model, msg),
        Msg::ResultMsg(msg) => crate::khana::page::results::update(model, msg),
        Msg::EntryAppMsg(msg) => crate::entry_app::update(model, msg),
        Msg::Conn(msg) => crate::sync::update(model, msg),
        Msg::ExportParcel => crate::sync::export_parcel(model),
        Msg::ImportParcel => crate::sync::import_parcel(model),
        Msg::OpenParcelEvent => crate::sync::open_parcel_event(model),
        Msg::VoidObservation(uid) => {
            // Determine the test from the run record, then delegate to enqueue_void.
            let test = model.khana.runs.with(|runs| {
                runs.iter()
                    .find(|r| r.uid == uid)
                    .map(|r| r.test)
                    .unwrap_or(1)
            });
            let car = model.khana.runs.with(|runs| {
                runs.iter()
                    .find(|r| r.uid == uid)
                    .map(|r| r.car.clone())
                    .unwrap_or_default()
            });
            crate::khana::helpers::enqueue_void(model, &uid, test, &car);
        }
        Msg::ScanStart => {
            #[cfg(target_arch = "wasm32")]
            crate::qr_scan::start_scan(model);
            #[cfg(not(target_arch = "wasm32"))]
            let _ = model;
        }
        Msg::ScanStop => {
            model.sync.scan_active.set(false);
            #[cfg(target_arch = "wasm32")]
            crate::qr_scan::stop_scan();
        }
        Msg::QrPauseToggle => crate::sync::toggle_qr_pause(model),
        Msg::QrClear => crate::sync::clear_qr(model),
        Msg::SetParcelMode(mode) => model.sync.parcel_mode.set(mode),
        Msg::Join(link) => {
            #[cfg(target_arch = "wasm32")]
            crate::sync::join_via_link(model, link);
            #[cfg(not(target_arch = "wasm32"))]
            let _ = link;
        }
        Msg::JoinUrl => {
            let text = model.screens.home.join_url.get_clone();
            let complete = crate::event::Invite::from_url(&text).filter(|inv| {
                !inv.homeserver.is_empty()
                    && !inv.event.is_empty()
                    && !inv.sid.is_empty()
                    && !inv.tid.is_empty()
            });
            match complete {
                Some(inv) => {
                    model.screens.home.join_msg.set(String::new());
                    crate::update(model, Msg::Join(inv));
                }
                None => {
                    model
                        .screens
                        .home
                        .join_msg
                        .set("That doesn't look like a valid join link.".to_string());
                }
            }
        }
        Msg::ImportAccount {
            homeserver,
            user_id,
            password,
        } => {
            #[cfg(target_arch = "wasm32")]
            if !homeserver.is_empty() && !user_id.is_empty() {
                let account = crate::services::matrix::Account {
                    homeserver,
                    user_id,
                    description: String::new(),
                    account_type: crate::services::matrix::AccountType::EventShared,
                    kind: crate::services::matrix::StoredAuth::Matrix {
                        device_id: String::new(),
                        access_token: String::new(),
                        refresh_token: None,
                        password,
                    },
                    active: false,
                    event_uid: None,
                };
                crate::services::matrix::save_account(&account);
                model
                    .screens
                    .accounts
                    .refresh
                    .set(model.screens.accounts.refresh.get() + 1);
                model
                    .screens
                    .accounts
                    .feedback
                    .set("Account imported.".into());
                show(model, Screen::Accounts);
            }
            #[cfg(not(target_arch = "wasm32"))]
            let _ = (homeserver, user_id, password);
        }
        Msg::ImportContact {
            user_id,
            name,
            description,
            phone,
        } => {
            #[cfg(target_arch = "wasm32")]
            if !user_id.is_empty() {
                let contact = crate::services::matrix::Contact {
                    user_id,
                    name,
                    description,
                    phone,
                    signing_key: None,
                };
                crate::services::matrix::save_contact(&contact);
                model
                    .screens
                    .accounts
                    .refresh
                    .set(model.screens.accounts.refresh.get() + 1);
                model
                    .screens
                    .accounts
                    .feedback
                    .set("Contact imported.".into());
                show(model, Screen::Accounts);
            }
            #[cfg(not(target_arch = "wasm32"))]
            let _ = (user_id, name, description, phone);
        }
    }
}

pub fn setup_effects(model: Model) {
    // stage command preview: re-parse whenever the input text changes
    create_effect(move || {
        let input = model.screens.stage.cmd.input.get_clone();
        let cmd = crate::khana::page::stage::parse_command(&input);
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
            let id = model.khana.event.with(|e| e.id.clone());
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
            model.khana.event.set(event);
            model.khana.scores.set(scores);
            model.khana.runs.set(runs);
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
    let id = model.khana.event.with(|e| e.id.clone());
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
        .khana
        .event
        .with(|e| e.status != crate::event::EventStatus::Draft);
    let has_unsent_setup = published
        && pending.iter().any(|m| {
            m.body
                .starts_with(crate::timing_event::TimingEvent::SETUP_PREFIX)
        });
    model.screens.setup.needs_sync.set(has_unsent_setup);
}

pub fn refresh_entry_app(model: Model) {
    let id = model.entry_app.event.with(|e| e.id.clone());
    if id.is_empty() {
        return;
    }
    let log = crate::log::load_log(&id);
    let pending = crate::log::load_pending(&id);
    let mut entries = Vec::new();
    for msg in log.iter().chain(pending.iter()) {
        if let Some((entry, delete)) = crate::entry_app::sync::parse_entry_body(&msg.body) {
            if delete {
                entries.retain(|e: &crate::entry_app::types::Entry| e.entry_no != entry.entry_no);
            } else if let Some(existing) = entries.iter_mut().find(|e| e.entry_no == entry.entry_no)
            {
                *existing = entry;
            } else {
                entries.push(entry);
            }
        }
    }
    model.entry_app.event.update(|ev| ev.entries = entries);
}

/// Enqueue a setup-manifest message for the current event (the durable record
/// of every edit) and refresh the feed.  Flushes to the room when connected.
pub fn enqueue_setup(model: Model) {
    let id = model.khana.event.with(|e| e.id.clone());
    if id.is_empty() {
        return;
    }
    let ev = model.khana.event.get_clone();
    let body = format!(
        "{}{}",
        crate::timing_event::TimingEvent::SETUP_PREFIX,
        serde_json::to_string(&ev).unwrap()
    );
    let sender = model.sync.identity.get_clone();
    // Setup is last-writer-wins: replace any superseded setup in the outbox so
    // a draft's Save Local history never gets flushed into the room on publish.
    crate::log::enqueue_setup_pending(&id, crate::log::LogMsg::new_pending(body, sender));
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
            (crate::khana::helpers::view_handoff_modals(model))
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
        Screen::Stopwatch,
        Screen::Results,
        Screen::Chat,
        Screen::Entries,
    ];
    let effective = if needs_event.contains(&screen) && model.khana.event.with(|e| e.is_null()) {
        Screen::Home
    } else {
        screen
    };
    view! {
        div(class="container") {
            (match effective {
                Screen::Home => page::home::view(model),
                Screen::Events => page::events::view(model),
                Screen::Accounts => page::accounts::view(model),
                Screen::Help => page::help::view(),
                Screen::KhanaRules => crate::khana::page::khana_rule::view(),
                Screen::Stage => crate::khana::page::stage::view(model),
                Screen::Start => crate::khana::page::start::view(model),
                Screen::Finish => crate::khana::page::finish::view(model),
                Screen::Stopwatch => crate::khana::page::stopwatch::view(model),
                Screen::Results => crate::khana::page::results::view(model),
                Screen::Event => crate::khana::page::event::view(model),
                Screen::Entries => crate::entry_app::view(model),
                Screen::Chat => page::chat::view(model),
            })
        }
    }
}

fn view_navbar(model: Model) -> View {
    let has_event = !model.khana.event.with(|e| e.is_null());
    let mode = model.mode.get();
    // Screens that need a current event: hidden/disabled until one is picked.
    // (Event itself stays enabled so the first event can be created.)
    let needs_event = [
        Screen::Start,
        Screen::Finish,
        Screen::Stage,
        Screen::Stopwatch,
        Screen::Results,
        Screen::Chat,
        Screen::Entries,
    ];
    // Top tabs: always visible, filtered by mode.
    let all_top_tabs = [
        (Screen::Home, "fa fa-home"),
        (Screen::Stopwatch, "fa fa-stopwatch"),
        (Screen::Results, "fa fa-trophy"),
        (Screen::Chat, "fa fa-comments"),
    ];
    let top_tabs: Vec<_> = all_top_tabs
        .iter()
        .filter(|(s, _)| mode.has_screen(*s))
        .copied()
        .collect();
    // Burger menu items: admin/less-frequent screens, filtered by mode.
    let all_burger_items = [
        (Screen::Events, "fa fa-folder-open", "Events"),
        (Screen::Event, "fa fa-screwdriver-wrench", "Event config"),
        (Screen::Entries, "fa fa-users", "Entries"),
        (Screen::Stage, "fa fa-stopwatch-20", "Manual timing"),
        (Screen::Start, "fa fa-flag", "Start flag"),
        (Screen::Finish, "fa fa-flag-checkered", "Finish flag"),
        (Screen::Accounts, "fa fa-user-gear", "Accounts"),
        (Screen::Help, "fa fa-question", "Help"),
        (Screen::KhanaRules, "fa fa-book", "Rules"),
    ];
    let burger_items: Vec<_> = all_burger_items
        .iter()
        .filter(|(s, _, _)| mode.has_screen(*s))
        .copied()
        .collect();
    let mut brand: Vec<View> = vec![];
    for (screen, icon) in top_tabs {
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
    // Burger dropdown items.
    let mut burger_menu_items: Vec<View> = vec![];
    for (screen, icon, label) in burger_items {
        let disabled = !has_event && needs_event.contains(&screen);
        let label_owned = label.to_string();
        if disabled {
            burger_menu_items.push(view! {
                a(class="navbar-item has-text-grey-light", title=label_owned) {
                    span(class="icon") { i(class=icon) }
                    span { (label) }
                }
            });
        } else {
            burger_menu_items.push(view! {
                a(class="navbar-item", on:click=move |_| {
                    update(model, Msg::Show(screen));
                    model.screens.home.burger_open.set(false);
                }) {
                    span(class="icon") { i(class=icon) }
                    span { (label) }
                }
            });
        }
    }
    view! {
        nav(class="navbar is-link is-hidden-print", role="navigation", aria-label="main navigation") {
            div(class="navbar-brand") {
                (brand)
                // Mode picker dropdown.
                div(class="navbar-item has-dropdown is-hoverable") {
                    a(class="navbar-link") {
                        span { (mode.label()) }
                    }
                    div(class="navbar-dropdown") {
                        (Mode::ALL.iter().map(|&m| {
                            let is_active = m == mode;
                            let cls = if is_active { "navbar-item is-active" } else { "navbar-item" };
                            view! {
                                a(class=cls, on:click=move |_| { update(model, Msg::SetMode(m)); }) {
                                    (m.label())
                                }
                            }
                        }).collect::<Vec<_>>())
                    }
                }
                (move || {
                    let open = model.screens.home.burger_open.get();
                    let cls = if open { "navbar-burger is-active" } else { "navbar-burger" };
                    view! {
                        a(
                            class=cls,
                            on:click=move |_| {
                                let cur = model.screens.home.burger_open.get();
                                model.screens.home.burger_open.set(!cur);
                            },
                        ) {
                            span {}
                            span {}
                            span {}
                        }
                    }
                })
                div(class=if model.screens.home.burger_open.get() { "navbar-menu is-active" } else { "navbar-menu" }) {
                    div(class="navbar-end") { (burger_menu_items) }
                }
            }
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
            Screen::Stopwatch,
            Screen::Accounts,
        ];
        for screen in all {
            assert_eq!(Screen::from_name(screen.name()), Some(screen));
        }
        assert_eq!(Screen::from_name("bogus"), None);
    }

    #[test]
    fn mode_visible_screens_contains_all() {
        // Every screen should appear in at least one mode.
        for screen in [
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
            Screen::Stopwatch,
            Screen::Accounts,
        ] {
            assert!(
                Mode::ALL.iter().any(|m| m.has_screen(screen)),
                "{screen:?} not visible in any mode"
            );
        }
    }

    #[test]
    fn mode_label_round_trips() {
        for &m in &Mode::ALL {
            let label = m.label();
            let expected = match m {
                Mode::Testing => "Testing",
                Mode::Organiser => "Organiser",
                Mode::Spectator => "Spectator",
                Mode::Official => "Official",
                Mode::Competitor => "Competitor",
            };
            assert_eq!(label, expected);
        }
    }
}
