use sycamore::prelude::*;

use crate::app::ConnState;
use crate::timing_event::TimingEvent;
use crate::Model;

// Matrix sync page: connect/register to any homeserver (localhost dev server
// self-registers), join the current event's timing room, live feed of room
// messages, and send chat / sample timing payloads.
//
// On the native (non-wasm) build there is no Matrix client, so `update` is a
// no-op; the page still renders so the layout is consistent.

#[derive(Clone)]
pub enum Msg {
    Connect,
    Logout,
    SendChat,
    SendSampleStart,
    SendSampleFinish,
}

#[derive(Clone)]
pub struct FeedEntry {
    pub ts: i64,
    pub sender: String,
    pub body: String,
    pub timing: Option<TimingEvent>,
}

#[derive(Clone, Copy)]
pub struct SyncModel {
    pub homeserver: Signal<String>,
    pub username: Signal<String>,
    pub password: Signal<String>,
    pub feed: Signal<Vec<FeedEntry>>,
    pub send_text: Signal<String>,
    pub busy: Signal<bool>,
    /// Event-selector list open/closed on the home page.
    pub show_events: Signal<bool>,
}

pub fn init() -> SyncModel {
    SyncModel {
        homeserver: create_signal("http://localhost:8008".to_string()),
        username: create_signal(String::new()),
        password: create_signal(String::new()),
        feed: create_signal(Vec::new()),
        send_text: create_signal(String::new()),
        busy: create_signal(false),
        show_events: create_signal(false),
    }
}

pub fn update(model: Model, msg: Msg) {
    #[cfg(target_arch = "wasm32")]
    update_wasm(model, msg);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (model, msg);
}

/// Resume a persisted Matrix session on app load (wasm only).
#[cfg(target_arch = "wasm32")]
pub fn resume_on_load(model: Model) {
    let Some(stored) = crate::services::matrix::load_session() else {
        return;
    };
    model.app.conn.set(ConnState::Connecting);
    wasm_bindgen_futures::spawn_local(async move {
        let res = async {
            let client = crate::services::matrix::new_client(&stored.homeserver).await?;
            crate::services::matrix::restore_session(&client, &stored).await?;
            crate::services::matrix::set_client(Some(client.clone()));
            let room =
                crate::services::matrix::join_room_for_event(&client, &model.app.event.get_clone())
                    .await?;
            crate::services::matrix::set_room(Some(room.clone()));
            crate::services::matrix::start_sync(client, sink_for(model));
            Ok::<_, String>(room.room_id().to_string())
        }
        .await;
        match res {
            Ok(room_id) => {
                model.app.identity.set(stored.user_id.clone());
                model.app.conn.set(ConnState::LoggedIn(stored.user_id));
                model.app.room.set(Some(room_id));
            }
            Err(e) => model.app.conn.set(ConnState::Error(e)),
        }
    });
}

/// Re-join the room for the currently selected event (after switching event).
pub fn join_current_event(model: Model) {
    #[cfg(target_arch = "wasm32")]
    join_current_event_wasm(model);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = model;
}

#[cfg(target_arch = "wasm32")]
fn join_current_event_wasm(model: Model) {
    let Some(client) = crate::services::matrix::client() else {
        return;
    };
    wasm_bindgen_futures::spawn_local(async move {
        match crate::services::matrix::join_room_for_event(&client, &model.app.event.get_clone())
            .await
        {
            Ok(room) => {
                crate::services::matrix::set_room(Some(room.clone()));
                model.app.room.set(Some(room.room_id().to_string()));
            }
            Err(e) => model.app.conn.set(ConnState::Error(e)),
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub fn resume_on_load(_model: Model) {}

#[cfg(target_arch = "wasm32")]
fn update_wasm(model: Model, msg: Msg) {
    match msg {
        Msg::Connect => connect(model),
        Msg::Logout => logout(model),
        Msg::SendChat => send_chat(model),
        Msg::SendSampleStart => send_sample(model, "start"),
        Msg::SendSampleFinish => send_sample(model, "finish"),
    }
}

/// True when `hs` is the local dev homeserver, which allows self-registration.
#[cfg(target_arch = "wasm32")]
fn is_dev_server(hs: &str) -> bool {
    let host = hs
        .strip_prefix("http://")
        .or_else(|| hs.strip_prefix("https://"))
        .and_then(|rest| rest.split('/').next())
        .and_then(|rest| rest.split(':').next())
        .unwrap_or("");
    host == "localhost" || host == "127.0.0.1" || host == "::1"
}

#[cfg(target_arch = "wasm32")]
fn connect(model: Model) {
    let sm = model.screens.sync;
    let hs = sm.homeserver.get_clone();
    let user = sm.username.get_clone();
    let pass = sm.password.get_clone();
    if user.trim().is_empty() || pass.is_empty() {
        model.app.conn.set(ConnState::Error(
            "Enter a username and password".to_string(),
        ));
        return;
    }
    let dev = is_dev_server(&hs);
    sm.busy.set(true);
    model.app.conn.set(ConnState::Connecting);
    wasm_bindgen_futures::spawn_local(async move {
        let res = async {
            let client = crate::services::matrix::new_client(&hs).await?;
            if dev {
                crate::services::matrix::register_or_login(&client, &user, &pass).await?;
            } else {
                crate::services::matrix::login(&client, &user, &pass).await?;
            }
            crate::services::matrix::save_session(&client, &hs);
            crate::services::matrix::set_client(Some(client.clone()));
            let room =
                crate::services::matrix::join_room_for_event(&client, &model.app.event.get_clone())
                    .await?;
            crate::services::matrix::set_room(Some(room.clone()));
            crate::services::matrix::start_sync(client, sink_for(model));
            Ok::<_, String>(room.room_id().to_string())
        }
        .await;
        match res {
            Ok(room_id) => {
                model.app.identity.set(user.clone());
                model.app.conn.set(ConnState::LoggedIn(user));
                model.app.room.set(Some(room_id));
                // Land on the Home dashboard (event status hub) once connected.
                crate::update(model, crate::Msg::Show(crate::Screen::Home));
            }
            Err(e) => model.app.conn.set(ConnState::Error(e)),
        }
        sm.busy.set(false);
    });
}

#[cfg(target_arch = "wasm32")]
fn logout(model: Model) {
    let sm = model.screens.sync;
    let Some(client) = crate::services::matrix::client() else {
        model.app.conn.set(ConnState::Idle);
        return;
    };
    sm.busy.set(true);
    wasm_bindgen_futures::spawn_local(async move {
        let _ = crate::services::matrix::logout(&client).await;
        crate::services::matrix::set_client(None);
        crate::services::matrix::set_room(None);
        model.app.identity.set(String::new());
        model.app.conn.set(ConnState::Idle);
        model.app.room.set(None);
        sm.busy.set(false);
    });
}

#[cfg(target_arch = "wasm32")]
fn send_chat(model: Model) {
    let sm = model.screens.sync;
    let text = sm.send_text.get_clone();
    if text.trim().is_empty() {
        return;
    }
    let Some(room) = crate::services::matrix::room() else {
        model
            .app
            .conn
            .set(ConnState::Error("Join the timing room first".to_string()));
        return;
    };
    sm.send_text.set(String::new());
    wasm_bindgen_futures::spawn_local(async move {
        if let Err(e) = crate::services::matrix::send_chat(&room, &text).await {
            model.app.conn.set(ConnState::Error(e));
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn send_sample(model: Model, r#type: &str) {
    let Some(room) = crate::services::matrix::room() else {
        model
            .app
            .conn
            .set(ConnState::Error("Join the timing room first".to_string()));
        return;
    };
    let event_id = model.app.event.get_clone().id;
    let mut te = TimingEvent::new(r#type, &event_id, 1, "17", 1);
    te.official_id = Some(model.app.identity.get_clone());
    te.status = Some("clean".to_string());
    wasm_bindgen_futures::spawn_local(async move {
        if let Err(e) = crate::services::matrix::send_timing(&room, &te).await {
            model.app.conn.set(ConnState::Error(e));
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn sink_for(model: Model) -> std::rc::Rc<dyn Fn(crate::services::matrix::IncomingMessage)> {
    use crate::services::matrix::IncomingMessage;

    let feed = model.screens.sync.feed;
    std::rc::Rc::new(move |msg: IncomingMessage| {
        let m = msg.clone();
        feed.update(|v| {
            v.push(FeedEntry {
                ts: m.ts,
                sender: m.sender,
                body: m.body,
                timing: m.timing,
            });
        });
        // Merge incoming timing into local state. Scoped by room: the app only
        // ever joins the selected event's timing room, and the room id check
        // drops stragglers from a previous event.
        let Some(te) = msg.timing else {
            return;
        };
        let Some(room) = crate::services::matrix::room() else {
            return;
        };
        if msg.room != room.room_id().to_string() {
            return;
        }
        if model.app.event.with(|e| e.id.is_empty()) {
            return;
        }
        if te.r#type == "start" || te.r#type == "finish" {
            // Mirror the remote run into local state (run numbering,
            // pending-starts) so Start/Finish screens stay live.
            let run = crate::event::RunRecord {
                r#type: te.r#type.clone(),
                test: te.test,
                car: te.car.clone(),
                run: te.run,
                ts: te.ts,
                time_ds: te.time_ds,
                status: te.status.clone(),
                flags: te.flags,
                official_id: te.official_id.clone(),
            };
            model.app.runs.update(|runs| {
                crate::event::add_run(runs, run);
            });
            let key = model.app.event.with(|e| crate::event::storage_key(e));
            let runs = model.app.runs.get_clone();
            crate::event::save_runs(&key, &runs);
        }
        if te.r#type == "finish" {
            let Some(time_ds) = te.time_ds else {
                return;
            };
            model
                .app
                .scores
                .update(|s| crate::event::upsert_time(s, te.test, &te.car, time_ds));
            let key = model.app.event.with(|e| crate::event::storage_key(e));
            let scores = model.app.scores.get_clone();
            crate::event::save_times(&key, &scores);
        }
        crate::update(model, crate::Msg::Reload);
    })
}

pub fn view(model: Model) -> View {
    view! {
        div {
            h1 { "Sync" }
            (view_connection(model))
            (view_room(model))
            (view_send(model))
            (view_feed(model))
        }
    }
}

fn view_connection(model: Model) -> View {
    let sm = model.screens.sync;
    view! {
        div(class="box") {
            h2(class="title is-5") { "Connection" }
            div(class="field") {
                label(class="label") { "Homeserver" }
                div(class="control") {
                    input(class="input", placeholder="http://localhost:8008", bind:value=sm.homeserver)
                }
            }
            div(class="field") {
                label(class="label") { "Username" }
                div(class="control") {
                    input(class="input", placeholder="app-a", bind:value=sm.username)
                }
            }
            div(class="field") {
                label(class="label") { "Password" }
                div(class="control") {
                    input(class="input", r#type="password", placeholder="password", bind:value=sm.password)
                }
            }
            (move || {
                let busy = sm.busy.get();
                view! {
                    div(class="field is-grouped") {
                        div(class="control") {
                            button(class="button is-link", disabled=busy, on:click=move |_| update(model, Msg::Connect)) {
                                "Connect"
                            }
                        }
                        div(class="control") {
                            button(class="button is-light", on:click=move |_| update(model, Msg::Logout)) {
                                "Logout"
                            }
                        }
                    }
                }
            })
            div { (move || status_html(model.app.conn.get_clone())) }
            p(class="help") { "Any homeserver works by logging in. The localhost dev server will register a new account for you." }
        }
    }
}

fn status_html(state: ConnState) -> View {
    match state {
        ConnState::Idle => view! { p(class="help") { "Not connected." } },
        ConnState::Connecting => view! { p(class="help") { "Connecting..." } },
        ConnState::LoggedIn(user) => {
            view! { p(class="help is-success") { ("Logged in as ") (user) } }
        }
        ConnState::Error(e) => view! { p(class="help is-danger") { (e) } },
    }
}

fn view_room(model: Model) -> View {
    view! {
        div(class="box") {
            h2(class="title is-5") { "Timing room" }
            div {
                (move || match model.app.room.get_clone() {
                    Some(id) => view! { p(class="help is-success") { ("Joined ") (id) } },
                    None => view! { p(class="help") { "Not joined. Connect to join the current event's timing room." } },
                })
            }
        }
    }
}

fn view_send(model: Model) -> View {
    let sm = model.screens.sync;
    view! {
        div(class="box") {
            h2(class="title is-5") { "Send" }
            div(class="field has-addons") {
                div(class="control is-expanded") {
                    input(
                        class="input",
                        placeholder="Message...",
                        bind:value=sm.send_text,
                        on:keydown=move |ev: web_sys::KeyboardEvent| {
                            if ev.key_code() == 13 {
                                update(model, Msg::SendChat);
                            }
                        },
                    )
                }
                div(class="control") {
                    button(class="button is-primary", on:click=move |_| update(model, Msg::SendChat)) {
                        "Send"
                    }
                }
            }
            div(class="field is-grouped") {
                div(class="control") {
                    button(class="button is-small", on:click=move |_| update(model, Msg::SendSampleStart)) {
                        "Send sample start"
                    }
                }
                div(class="control") {
                    button(class="button is-small", on:click=move |_| update(model, Msg::SendSampleFinish)) {
                        "Send sample finish"
                    }
                }
            }
        }
    }
}

fn view_feed(model: Model) -> View {
    let sm = model.screens.sync;
    view! {
        div(class="box") {
            h2(class="title is-5") { "Live feed" }
            (move || {
                let entries = sm.feed.get_clone();
                if entries.is_empty() {
                    return view! { p(class="help") { "No messages yet." } };
                }
                let views: Vec<View> = entries
                    .iter()
                    .rev()
                    .map(|e| {
                        let line = feed_line(e);
                        view! {
                            div {
                                pre { (line) }
                            }
                        }
                    })
                    .collect();
                views.into()
            })
        }
    }
}

/// Single feed line shared by the Sync page and the Results live-feed panel.
pub fn feed_line(e: &FeedEntry) -> String {
    let timing = e
        .timing
        .as_ref()
        .map(|t| {
            format!(
                "  [KT {} test={} car={} run={}]",
                t.r#type, t.test, t.car, t.run
            )
        })
        .unwrap_or_default();
    format!("{} {}: {}{}", e.sender, fmt_ts(e.ts), e.body, timing)
}

fn fmt_ts(ms: i64) -> String {
    let d = js_sys::Date::new(&js_sys::Number::from(ms as f64).into());
    d.to_string().into()
}
