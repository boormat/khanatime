use sycamore::prelude::*;

use crate::timing_event::TimingEvent;
use crate::Model;

// Matrix sync page: connect/register, join the `#timing` room, live feed of
// room messages, and send chat / sample timing payloads.
//
// On the native (non-wasm) build there is no Matrix client, so `update` is a
// no-op; the page still renders so the layout is consistent.

#[derive(Clone)]
pub enum Msg {
    Register,
    Login,
    Logout,
    JoinRoom,
    SendChat,
    SendSampleStart,
    SendSampleFinish,
}

#[derive(Clone, PartialEq)]
pub enum ConnState {
    Idle,
    Connecting,
    LoggedIn(String),
    Error(String),
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
    pub status: Signal<ConnState>,
    pub room_id: Signal<Option<String>>,
    pub feed: Signal<Vec<FeedEntry>>,
    pub send_text: Signal<String>,
    pub busy: Signal<bool>,
}

pub fn init() -> SyncModel {
    SyncModel {
        homeserver: create_signal("http://localhost:8008".to_string()),
        username: create_signal(String::new()),
        password: create_signal(String::new()),
        status: create_signal(ConnState::Idle),
        room_id: create_signal(None),
        feed: create_signal(Vec::new()),
        send_text: create_signal(String::new()),
        busy: create_signal(false),
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
    let sm = model.sync_model;
    let Some(stored) = crate::services::matrix::load_session() else {
        return;
    };
    sm.status.set(ConnState::Connecting);
    wasm_bindgen_futures::spawn_local(async move {
        let res = async {
            let client = crate::services::matrix::new_client(&stored.homeserver).await?;
            crate::services::matrix::restore_session(&client, &stored).await?;
            crate::services::matrix::set_client(Some(client.clone()));
            let room = crate::services::matrix::join_or_create_room(&client).await?;
            crate::services::matrix::set_room(Some(room.clone()));
            crate::services::matrix::start_sync(client, sink_for(sm));
            Ok::<_, String>(room.room_id().to_string())
        }
        .await;
        match res {
            Ok(room_id) => {
                sm.status.set(ConnState::LoggedIn(stored.user_id));
                sm.room_id.set(Some(room_id));
            }
            Err(e) => sm.status.set(ConnState::Error(e)),
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub fn resume_on_load(_model: Model) {}

#[cfg(target_arch = "wasm32")]
fn update_wasm(model: Model, msg: Msg) {
    match msg {
        Msg::Register => connect(model, true),
        Msg::Login => connect(model, false),
        Msg::Logout => logout(model),
        Msg::JoinRoom => join(model),
        Msg::SendChat => send_chat(model),
        Msg::SendSampleStart => send_sample(model, "start"),
        Msg::SendSampleFinish => send_sample(model, "finish"),
    }
}

#[cfg(target_arch = "wasm32")]
fn connect(model: Model, is_register: bool) {
    let sm = model.sync_model;
    let hs = sm.homeserver.get_clone();
    let user = sm.username.get_clone();
    let pass = sm.password.get_clone();
    if user.trim().is_empty() || pass.is_empty() {
        sm.status.set(ConnState::Error(
            "Enter a username and password".to_string(),
        ));
        return;
    }
    sm.busy.set(true);
    sm.status.set(ConnState::Connecting);
    wasm_bindgen_futures::spawn_local(async move {
        let res = async {
            let client = crate::services::matrix::new_client(&hs).await?;
            if is_register {
                crate::services::matrix::register_or_login(&client, &user, &pass).await?;
            } else {
                crate::services::matrix::login(&client, &user, &pass).await?;
            }
            crate::services::matrix::save_session(&client, &hs);
            crate::services::matrix::set_client(Some(client.clone()));
            let room = crate::services::matrix::join_or_create_room(&client).await?;
            crate::services::matrix::set_room(Some(room.clone()));
            crate::services::matrix::start_sync(client, sink_for(sm));
            Ok::<_, String>(room.room_id().to_string())
        }
        .await;
        match res {
            Ok(room_id) => {
                sm.status.set(ConnState::LoggedIn(user));
                sm.room_id.set(Some(room_id));
            }
            Err(e) => sm.status.set(ConnState::Error(e)),
        }
        sm.busy.set(false);
    });
}

#[cfg(target_arch = "wasm32")]
fn logout(model: Model) {
    let sm = model.sync_model;
    let Some(client) = crate::services::matrix::client() else {
        sm.status.set(ConnState::Idle);
        return;
    };
    sm.busy.set(true);
    wasm_bindgen_futures::spawn_local(async move {
        let _ = crate::services::matrix::logout(&client).await;
        crate::services::matrix::set_client(None);
        crate::services::matrix::set_room(None);
        sm.status.set(ConnState::Idle);
        sm.room_id.set(None);
        sm.busy.set(false);
    });
}

#[cfg(target_arch = "wasm32")]
fn join(model: Model) {
    let sm = model.sync_model;
    let Some(client) = crate::services::matrix::client() else {
        sm.status.set(ConnState::Error("Log in first".to_string()));
        return;
    };
    sm.busy.set(true);
    wasm_bindgen_futures::spawn_local(async move {
        let res = crate::services::matrix::join_or_create_room(&client).await;
        match res {
            Ok(room) => {
                crate::services::matrix::set_room(Some(room.clone()));
                sm.room_id.set(Some(room.room_id().to_string()));
                crate::services::matrix::start_sync(client, sink_for(sm));
            }
            Err(e) => sm.status.set(ConnState::Error(e)),
        }
        sm.busy.set(false);
    });
}

#[cfg(target_arch = "wasm32")]
fn send_chat(model: Model) {
    let sm = model.sync_model;
    let text = sm.send_text.get_clone();
    if text.trim().is_empty() {
        return;
    }
    let Some(room) = crate::services::matrix::room() else {
        sm.status
            .set(ConnState::Error("Join the timing room first".to_string()));
        return;
    };
    sm.send_text.set(String::new());
    wasm_bindgen_futures::spawn_local(async move {
        if let Err(e) = crate::services::matrix::send_chat(&room, &text).await {
            sm.status.set(ConnState::Error(e));
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn send_sample(model: Model, r#type: &str) {
    let sm = model.sync_model;
    let Some(room) = crate::services::matrix::room() else {
        sm.status
            .set(ConnState::Error("Join the timing room first".to_string()));
        return;
    };
    let event_name = model.event.get_clone().name;
    let mut te = TimingEvent::new(r#type, &event_name, 1, "17", 1);
    te.official_id = Some(sm.username.get_clone());
    te.status = Some("clean".to_string());
    wasm_bindgen_futures::spawn_local(async move {
        if let Err(e) = crate::services::matrix::send_timing(&room, &te).await {
            sm.status.set(ConnState::Error(e));
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn sink_for(sm: SyncModel) -> std::rc::Rc<dyn Fn(crate::services::matrix::IncomingMessage)> {
    use crate::services::matrix::IncomingMessage;

    let feed = sm.feed;
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
    let sm = model.sync_model;
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
                            button(class="button is-link", disabled=busy, on:click=move |_| update(model, Msg::Register)) {
                                "Register"
                            }
                        }
                        div(class="control") {
                            button(class="button is-primary", disabled=busy, on:click=move |_| update(model, Msg::Login)) {
                                "Login"
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
            div { (move || status_html(sm.status.get_clone())) }
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
    let sm = model.sync_model;
    view! {
        div(class="box") {
            h2(class="title is-5") { "Timing room" }
            div {
                (move || match sm.room_id.get_clone() {
                    Some(id) => view! { p(class="help is-success") { ("Joined ") (id) } },
                    None => view! { p(class="help") { "Not joined. Shared room: #timing:localhost" } },
                })
            }
            button(class="button is-info", on:click=move |_| update(model, Msg::JoinRoom)) {
                "Join / create timing room"
            }
        }
    }
}

fn view_send(model: Model) -> View {
    let sm = model.sync_model;
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
    let sm = model.sync_model;
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
                        let line = format!("{} {}: {}{}", e.sender, fmt_ts(e.ts), e.body, timing);
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

fn fmt_ts(ms: i64) -> String {
    let d = js_sys::Date::new(&js_sys::Number::from(ms as f64).into());
    d.to_string().into()
}
