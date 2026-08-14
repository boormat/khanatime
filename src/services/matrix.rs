//! Matrix sync transport (browser only).
//!
//! Thin wrapper around `matrix-sdk` 0.18 for the shared `#timing` room:
//! register/login, session persistence, join-or-create the room, send chat /
//! timing payloads and stream incoming messages from the sync loop.
//!
//! Compiled only for `wasm32` — see the `[target.'cfg(target_arch = "wasm32")']`
//! section in `Cargo.toml`.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use futures::StreamExt;
use matrix_sdk::{
    authentication::{matrix::MatrixSession, AuthSession, SessionTokens},
    config::SyncSettings,
    deserialized_responses::{TimelineEvent, TimelineEventKind},
    Client, Room, SessionMeta,
};
use ruma::{
    api::{
        client::{
            account::register::{self, RegistrationKind},
            directory::get_public_rooms_filtered,
            message::get_message_events,
            room::{
                create_room::{
                    self,
                    v3::{CreationContent, RoomPreset},
                },
                Visibility,
            },
            uiaa::{AuthData, AuthType, Dummy},
        },
        error::ErrorKind,
        Direction,
    },
    events::{
        room::topic::RoomTopicEventContent, space::child::SpaceChildEventContent,
        space::parent::SpaceParentEventContent,
    },
    room::RoomType,
    serde::Raw,
    uint, OwnedRoomAliasId, OwnedRoomId, OwnedServerName,
};
use serde::{Deserialize, Serialize};

use crate::timing_event::TimingEvent;

pub const ROOM_ALIAS: &str = "#timing:localhost";
const SESSION_KEY: &str = "kt_sync_session";
const STORE_NAME: &str = "khanatime_sync";
const DEVICE_NAME: &str = "khanatime-wasm";

/// A message that arrived over the sync loop.
#[derive(Debug, Clone)]
pub struct IncomingMessage {
    #[allow(dead_code)] // wire metadata, surfaced if multi-room support lands
    pub room: String,
    /// Matrix event id, used to dedupe feed entries across live sync + backfill.
    pub mid: String,
    pub sender: String,
    pub body: String,
    pub ts: i64,
    pub timing: Option<TimingEvent>,
    /// Full raw `m.room.message` event JSON, for pretty-printing on demand.
    pub raw: String,
}

// Single-writer module state: the logged-in client + its joined room. Kept out
// of the sycamore `Model` so it stays `Copy`-friendly (single-threaded wasm).
thread_local! {
    static CLIENT: RefCell<Option<Client>> = const { RefCell::new(None) };
    static ROOM: RefCell<Option<Room>> = const { RefCell::new(None) };
}

pub fn set_client(client: Option<Client>) {
    CLIENT.with(|c| *c.borrow_mut() = client);
}

pub fn client() -> Option<Client> {
    CLIENT.with(|c| c.borrow().clone())
}

pub fn set_room(room: Option<Room>) {
    ROOM.with(|r| *r.borrow_mut() = room);
}

pub fn room() -> Option<Room> {
    ROOM.with(|r| r.borrow().clone())
}

// ----- session persistence (localStorage) -----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredSession {
    pub homeserver: String,
    pub user_id: String,
    pub device_id: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

pub fn save_session(client: &Client, homeserver: &str) {
    let Some(AuthSession::Matrix(ms)) = client.session() else {
        return;
    };
    let stored = StoredSession {
        homeserver: homeserver.to_string(),
        user_id: ms.meta.user_id.to_string(),
        device_id: ms.meta.device_id.to_string(),
        access_token: ms.tokens.access_token,
        refresh_token: ms.tokens.refresh_token,
    };
    if let Some(st) = storage() {
        if let Ok(json) = serde_json::to_string(&stored) {
            let _ = st.set_item(SESSION_KEY, &json);
        }
    }
}

pub fn load_session() -> Option<StoredSession> {
    storage()?
        .get_item(SESSION_KEY)
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
}

pub fn clear_session() {
    if let Some(st) = storage() {
        let _ = st.remove_item(SESSION_KEY);
    }
}

// ----- client lifecycle -----

pub async fn new_client(homeserver: &str) -> Result<Client, String> {
    Client::builder()
        .server_name_or_homeserver_url(homeserver)
        .indexeddb_store(STORE_NAME, None)
        .build()
        .await
        .map_err(|e| e.to_string())
}

/// Why a registration attempt failed.
enum RegisterError {
    /// The username is already taken — fall back to logging in.
    UserInUse,
    /// Any other failure.
    Other(String),
}

impl From<String> for RegisterError {
    fn from(s: String) -> Self {
        RegisterError::Other(s)
    }
}

impl From<matrix_sdk::Error> for RegisterError {
    fn from(e: matrix_sdk::Error) -> Self {
        if let matrix_sdk::Error::Http(he) = &e {
            if let Some(ErrorKind::UserInUse) =
                he.as_client_api_error().and_then(|e| e.error_kind())
            {
                return RegisterError::UserInUse;
            }
        }
        RegisterError::Other(e.to_string())
    }
}

async fn register(client: &Client, username: &str, password: &str) -> Result<(), RegisterError> {
    let mut request = register::v3::Request::default();
    request.username = Some(username.to_string());
    request.password = Some(password.to_string());
    request.initial_device_display_name = Some(DEVICE_NAME.to_string());
    request.kind = RegistrationKind::User;
    request.inhibit_login = false;

    // Synapse answers the first attempt with a 401 + UIAA session; complete it
    // with the `m.login.dummy` flow the server advertised.
    let response = match client.matrix_auth().register(request.clone()).await {
        Ok(response) => response,
        Err(matrix_sdk::Error::Http(error)) if error.as_uiaa_response().is_some() => {
            let info = error.as_uiaa_response().expect("checked above");
            let supports_dummy = info
                .flows
                .iter()
                .any(|flow| flow.stages.contains(&AuthType::Dummy));
            if !supports_dummy {
                return Err(RegisterError::Other(
                    "registration requires an unsupported auth flow".to_string(),
                ));
            }
            let mut dummy = Dummy::new();
            dummy.session = info.session.clone();
            request.auth = Some(AuthData::Dummy(dummy));
            client
                .matrix_auth()
                .register(request)
                .await
                .map_err(RegisterError::from)?
        }
        Err(error) => return Err(RegisterError::from(error)),
    };
    if response.access_token.is_none() {
        login(client, username, password).await?;
    }
    Ok(())
}

pub async fn login(client: &Client, username: &str, password: &str) -> Result<(), String> {
    try_login(client, username, password)
        .await
        .map_err(|e| describe_login_failure(&e, username))
}

async fn try_login(
    client: &Client,
    username: &str,
    password: &str,
) -> Result<(), matrix_sdk::Error> {
    client
        .matrix_auth()
        .login_username(username, password)
        .initial_device_display_name(DEVICE_NAME)
        .send()
        .await
        .map(|_| ())
}

/// The Matrix error code of a client-server API error, if it is one.
fn error_kind(e: &matrix_sdk::Error) -> Option<&ErrorKind> {
    match e {
        matrix_sdk::Error::Http(he) => he.as_client_api_error().and_then(|e| e.error_kind()),
        _ => None,
    }
}

fn is_forbidden(e: &matrix_sdk::Error) -> bool {
    matches!(error_kind(e), Some(ErrorKind::Forbidden))
}

fn describe_login_failure(e: &matrix_sdk::Error, username: &str) -> String {
    if is_forbidden(e) {
        return format!(
            "Incorrect username or password for '{username}' — if you don't have an account yet, use Register"
        );
    }
    e.to_string()
}

fn describe_taken_login_failure(e: &matrix_sdk::Error, username: &str) -> String {
    if is_forbidden(e) {
        return format!(
            "Username '{username}' is already registered and that password doesn't match — switch to Log in and use the correct password"
        );
    }
    e.to_string()
}

pub async fn register_or_login(
    client: &Client,
    username: &str,
    password: &str,
) -> Result<(), String> {
    match register(client, username, password).await {
        Ok(()) => Ok(()),
        Err(RegisterError::UserInUse) => {
            khanatime::log!("username taken; trying login instead");
            try_login(client, username, password)
                .await
                .map_err(|e| describe_taken_login_failure(&e, username))
        }
        Err(RegisterError::Other(e)) => Err(e),
    }
}

pub async fn restore_session(client: &Client, stored: &StoredSession) -> Result<(), String> {
    let session = AuthSession::Matrix(MatrixSession {
        meta: SessionMeta {
            user_id: ruma::UserId::parse(&stored.user_id).map_err(|e| e.to_string())?,
            device_id: stored.device_id.as_str().into(),
        },
        tokens: SessionTokens {
            access_token: stored.access_token.clone(),
            refresh_token: stored.refresh_token.clone(),
        },
    });
    client
        .restore_session(session)
        .await
        .map_err(|e| e.to_string())
}

pub async fn logout(client: &Client) -> Result<(), String> {
    let res = client.logout().await.map_err(|e| e.to_string());
    clear_session();
    res
}

// ----- room -----

pub async fn join_or_create_room(client: &Client) -> Result<Room, String> {
    let alias: ruma::OwnedRoomOrAliasId = ROOM_ALIAS
        .parse()
        .map_err(|e: ruma::IdParseError| e.to_string())?;
    if let Ok(room) = client.join_room_by_id_or_alias(&alias, &[]).await {
        return Ok(room);
    }
    let mut request = create_room::v3::Request::default();
    request.name = Some("timing".to_string());
    request.room_alias_name = Some("timing".to_string());
    request.preset = Some(RoomPreset::PublicChat);
    request.is_direct = false;
    client.create_room(request).await.map_err(|e| e.to_string())
}

// ----- per-event spaces (publish) -----

/// The space + timing room of a published event.
#[derive(Debug, Clone)]
pub struct EventRooms {
    pub space: Room,
    pub timing: Room,
    pub space_alias: OwnedRoomAliasId,
    pub timing_alias: OwnedRoomAliasId,
}

/// Server name of the homeserver, used for `via` and room aliases.
fn server_name(client: &Client) -> OwnedServerName {
    let host = client
        .homeserver()
        .host_str()
        .map(|h| h.split(':').next().unwrap_or(h).to_string())
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| "localhost".to_string());
    host.parse::<OwnedServerName>()
        .unwrap_or_else(|_| "localhost".parse().expect("static server name"))
}

/// Build the room alias `#<localpart>:<server>` for this homeserver.
fn alias(client: &Client, localpart: &str) -> Result<OwnedRoomAliasId, String> {
    format!("#{localpart}:{}", server_name(client))
        .parse()
        .map_err(|e: ruma::IdParseError| e.to_string())
}

async fn create_room_with_alias(
    client: &Client,
    room_alias: &OwnedRoomAliasId,
    name: &str,
    is_space: bool,
) -> Result<Room, String> {
    let mut request = create_room::v3::Request::default();
    request.name = Some(name.to_string());
    request.room_alias_name = Some(room_alias.alias().to_string());
    request.preset = Some(RoomPreset::PublicChat);
    request.visibility = Visibility::Public;
    if is_space {
        let mut creation = CreationContent::new();
        creation.room_type = Some(RoomType::Space);
        request.creation_content =
            Some(Raw::new(&creation).map_err(|e: serde_json::Error| e.to_string())?);
    }
    client.create_room(request).await.map_err(|e| e.to_string())
}

/// The `io.kt.event` state content of a room, if any.
async fn read_event_meta_content(client: &Client, room: &Room) -> Option<serde_json::Value> {
    use ruma::api::client::state::get_state_events;
    let request = get_state_events::v3::Request::new(room.room_id().to_owned());
    let response = client.send(request).await.ok()?;
    for raw in response.room_state {
        let json: serde_json::Value = serde_json::from_str(raw.json().get()).ok()?;
        if json["type"].as_str() == Some("io.kt.event") {
            return json.get("content").cloned();
        }
    }
    None
}

/// The event id recorded in the `io.kt.event` state event of a room, if any.
async fn read_event_meta(client: &Client, room: &Room) -> Option<String> {
    read_event_meta_content(client, room)
        .await
        .and_then(|c| c.get("id")?.as_str().map(|s| s.to_string()))
}

/// Create the event space + timing room, or join our existing ones.
///
/// Idempotent: if the space alias is already taken by *this* event, it is
/// joined and reused (multi-device convergence).  If it is taken by a
/// different event, an error is returned with a suggestion to disambiguate.
pub async fn publish_event(
    client: &Client,
    event: &crate::event::EventInfo,
) -> Result<EventRooms, String> {
    if !crate::event::valid_event_id(&event.id) {
        return Err(format!(
            "Event id '{}' is not usable — it needs a year, e.g. kt-2026-...",
            event.id
        ));
    }
    let space_alias = alias(client, &event.id)?;
    let timing_alias = alias(client, &format!("{}-timing", event.id))?;

    let space = match client.is_room_alias_available(&space_alias).await {
        Ok(true) => create_room_with_alias(client, &space_alias, &event.name, true).await?,
        Ok(false) => {
            let res = client
                .resolve_room_alias(&space_alias)
                .await
                .map_err(|e| e.to_string())?;
            let room = client
                .join_room_by_id(&res.room_id)
                .await
                .map_err(|e| e.to_string())?;
            match read_event_meta(client, &room).await {
                Some(id) if id == event.id => room,
                _ => {
                    return Err(format!(
                        "Space alias '{space_alias}' is in use by a different event — add the club/district or override the event slug"
                    ));
                }
            }
        }
        Err(e) => return Err(e.to_string()),
    };

    let timing = match client.is_room_alias_available(&timing_alias).await {
        Ok(true) => create_room_with_alias(client, &timing_alias, "timing", false).await?,
        Ok(false) => {
            let res = client
                .resolve_room_alias(&timing_alias)
                .await
                .map_err(|e| e.to_string())?;
            client
                .join_room_by_id(&res.room_id)
                .await
                .map_err(|e| e.to_string())?
        }
        Err(e) => return Err(e.to_string()),
    };

    // Link space <-> timing room.
    let via = vec![server_name(client)];
    space
        .send_state_event_for_key(timing.room_id(), SpaceChildEventContent::new(via.clone()))
        .await
        .map_err(|e| e.to_string())?;
    timing
        .send_state_event_for_key(space.room_id(), SpaceParentEventContent::new(via))
        .await
        .map_err(|e| e.to_string())?;

    // Meta + topic on the space: searchable directory entry + room identity
    // (so a later publish can detect "this is ours").
    let meta = serde_json::json!({
        "id": event.id,
        "name": event.name,
        "club": event.sponsoring_club,
        "year": event.year,
    });
    space
        .send_state_event_raw("io.kt.event", "", meta)
        .await
        .map_err(|e| e.to_string())?;
    space
        .send_state_event(RoomTopicEventContent::new(format!(
            "{} · {} ({})",
            event.name, event.sponsoring_club, event.year
        )))
        .await
        .map_err(|e| e.to_string())?;

    // Give the timing room its setup manifest so fresh devices can adopt it.
    let _ = send_setup(&timing, event).await;

    Ok(EventRooms {
        space,
        timing,
        space_alias,
        timing_alias,
    })
}

/// Publish the current event using the logged-in identity (the single account
/// connected on the Home page).  Errors when no session is active.
pub async fn publish_current_event(event: &crate::event::EventInfo) -> Result<EventRooms, String> {
    let Some(client) = client() else {
        return Err("Log in on the Home page first".to_string());
    };
    publish_event(&client, event).await
}

/// A published event found via the room-directory search.
#[derive(Debug, Clone)]
pub struct EventSearchResult {
    pub name: String,
    pub alias: String,
    pub room_id: String,
}

/// Search the homeserver's public room directory for published khanatime event
/// spaces (rooms with an `io.kt.event`-style alias or a `kt-` alias).
pub async fn search_events(client: &Client, term: &str) -> Result<Vec<EventSearchResult>, String> {
    let mut request = get_public_rooms_filtered::v3::Request::new();
    request.limit = Some(uint!(20));
    let mut filter = ruma::directory::Filter::default();
    filter.generic_search_term = Some(term.to_string());
    request.filter = filter;
    let response = client.send(request).await.map_err(|e| e.to_string())?;
    let out = response
        .chunk
        .into_iter()
        .filter(|c| c.room_type == Some(RoomType::Space))
        .filter(|c| {
            let alias = c
                .canonical_alias
                .as_ref()
                .map(|a| a.to_string())
                .unwrap_or_default();
            alias.starts_with("#kt-")
                || c.name
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains("khanatime")
        })
        .map(|c| {
            let alias = c
                .canonical_alias
                .as_ref()
                .map(|a| a.to_string())
                .unwrap_or_default();
            EventSearchResult {
                name: c.name.unwrap_or_default(),
                alias,
                room_id: c.room_id.to_string(),
            }
        })
        .collect();
    Ok(out)
}

/// Join a published event space and build the local [crate::event::EventInfo]
/// from its `io.kt.event` state.  Entries/stages arrive later via the timing
/// room's setup-manifest backfill.
pub async fn open_published_event(
    client: &Client,
    alias: &str,
) -> Result<crate::event::EventInfo, String> {
    let parsed: ruma::OwnedRoomOrAliasId = alias
        .parse()
        .map_err(|e: ruma::IdParseError| e.to_string())?;
    let room = client
        .join_room_by_id_or_alias(&parsed, &[])
        .await
        .map_err(|e| e.to_string())?;
    let meta = read_event_meta_content(client, &room)
        .await
        .ok_or_else(|| "That room isn't a khanatime event".to_string())?;
    let id = meta
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "That room isn't a khanatime event".to_string())?;
    let mut ev = crate::event::EventInfo {
        id: id.to_string(),
        name: meta
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        sponsoring_club: meta
            .get("club")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        year: meta
            .get("year")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        status: crate::event::EventStatus::Published,
        ..Default::default()
    };
    ev.space_alias = Some(alias.to_string());
    Ok(ev)
}

/// Join an event room by alias (used by invite arrivals).
pub async fn join_room_by_alias(client: &Client, alias: &str) -> Result<Room, String> {
    let parsed: ruma::OwnedRoomOrAliasId = alias
        .parse()
        .map_err(|e: ruma::IdParseError| e.to_string())?;
    client
        .join_room_by_id_or_alias(&parsed, &[])
        .await
        .map_err(|e| e.to_string())
}

/// Join the current event's timing room. Prefers the published alias, then the
/// published room id, then falls back to the shared `#timing` room.
pub async fn join_room_for_event(
    client: &Client,
    event: &crate::event::EventInfo,
) -> Result<Room, String> {
    if let Some(alias) = &event.timing_alias {
        if let Ok(room) = join_room_by_alias(client, alias).await {
            return Ok(room);
        }
    }
    if let Some(id) = &event.timing_id {
        if let Ok(room_id) = id.parse::<ruma::OwnedRoomId>() {
            if let Ok(room) = client.join_room_by_id(&room_id).await {
                return Ok(room);
            }
        }
    }
    join_or_create_room(client).await
}

// ----- send -----

pub async fn send_chat(room: &Room, text: &str) -> Result<String, String> {
    let content = serde_json::json!({ "msgtype": "m.text", "body": text });
    room.send_raw(TimingEvent::MESSAGE_TYPE, content)
        .await
        .map(|res| res.response.event_id.to_string())
        .map_err(|e| e.to_string())
}

pub async fn send_timing(room: &Room, event: &TimingEvent) -> Result<String, String> {
    room.send_raw(TimingEvent::MESSAGE_TYPE, event.to_matrix_content())
        .await
        .map(|res| res.response.event_id.to_string())
        .map_err(|e| e.to_string())
}

/// Broadcast the full event setup so fresh devices joining the timing room can
/// adopt it (`khanatime_setup:` body prefix).  Merge is last-writer-wins.
pub async fn send_setup(room: &Room, event: &crate::event::EventInfo) -> Result<String, String> {
    let json = serde_json::to_string(event).map_err(|e| e.to_string())?;
    let body = format!("{}{}", TimingEvent::SETUP_PREFIX, json);
    send_chat(room, &body).await
}

/// Send a stored outbox message to the room, returning the Matrix event id.
/// Setup manifests are plain `m.text` bodies; timing messages carry the
/// `khanatime` content key (reconstructed from their `KT {json}` body).
pub async fn send_log_message(room: &Room, msg: &crate::log::LogMsg) -> Result<String, String> {
    if msg.body.starts_with(TimingEvent::SETUP_PREFIX)
        || msg.body.starts_with(TimingEvent::ENTRY_PREFIX)
    {
        send_chat(room, &msg.body).await
    } else if let Some(te) = TimingEvent::from_body(&msg.body) {
        send_timing(room, &te).await
    } else {
        Err("not a sendable log message".to_string())
    }
}

/// Broadcast a results snapshot for the audit trail (`khanatime_result:` body
/// prefix).  Informational only — every device computes results from the same
/// merged data.
pub async fn send_result(
    room: &Room,
    event: &crate::event::EventInfo,
    scores: &[crate::event::ScoreData],
) -> Result<(), String> {
    let body = serde_json::json!({
        "event_id": event.id,
        "ts": js_sys::Date::now() as i64,
        "scores": scores,
    });
    let body = format!("{}{}", TimingEvent::RESULT_PREFIX, body);
    send_chat(room, &body).await.map(|_| ())
}

/// Replay the full room history oldest→newest into `on_event`, so a joining
/// device merges every timing message stored on the server.  Idempotent: the
/// merge sink dedupes runs and last-writer-wins scores/setup.
pub async fn backfill_room_history(
    client: &Client,
    room: &Room,
    on_event: &dyn Fn(IncomingMessage),
) -> Result<usize, String> {
    let mut from: Option<String> = None;
    let mut messages: Vec<IncomingMessage> = Vec::new();
    loop {
        let mut request =
            get_message_events::v3::Request::new(room.room_id().to_owned(), Direction::Backward);
        request.from = from;
        request.limit = uint!(100);
        let response = client.send(request).await.map_err(|e| e.to_string())?;
        for raw in &response.chunk {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(raw.json().get()) else {
                continue;
            };
            if let Some(msg) = parse_message_json(room.room_id(), &v) {
                messages.push(msg);
            }
        }
        match response.end {
            Some(end) if !end.is_empty() => from = Some(end),
            _ => break,
        }
    }
    messages.reverse(); // server returned newest-first pages
    let count = messages.len();
    for msg in messages {
        on_event(msg);
    }
    Ok(count)
}

// ----- receive -----

/// Spawn the long-polling sync loop, pushing incoming messages into [sink].
pub fn start_sync(client: Client, sink: Rc<dyn Fn(IncomingMessage)>) {
    wasm_bindgen_futures::spawn_local(async move {
        let settings = SyncSettings::new().timeout(Duration::from_secs(30));
        let mut stream = Box::pin(client.sync_stream(settings).await);
        while let Some(res) = stream.next().await {
            match res {
                Ok(response) => {
                    for (room_id, update) in response.rooms.joined {
                        for tev in &update.timeline.events {
                            if let Some(msg) = parse_timeline_event(&room_id, tev) {
                                sink(msg);
                            }
                        }
                    }
                }
                Err(e) => khanatime::log!("matrix sync error: {e}"),
            }
        }
    });
}

fn parse_timeline_event(room_id: &OwnedRoomId, tev: &TimelineEvent) -> Option<IncomingMessage> {
    let raw = match &tev.kind {
        TimelineEventKind::PlainText { event } => event.json(),
        _ => return None,
    };
    let v: serde_json::Value = serde_json::from_str(raw.get()).ok()?;
    parse_message_json(room_id, &v)
}

/// Parse any `m.room.message` JSON into an [IncomingMessage] (shared by the
/// sync loop and history backfill).
fn parse_message_json(room_id: &ruma::RoomId, v: &serde_json::Value) -> Option<IncomingMessage> {
    if v["type"].as_str() != Some(TimingEvent::MESSAGE_TYPE) {
        return None;
    }
    let content = v.get("content")?;
    if content.get("msgtype").and_then(|m| m.as_str()) != Some("m.text") {
        return None;
    }
    let sender = v
        .get("sender")
        .and_then(|s| s.as_str())
        .unwrap_or("?")
        .to_string();
    let body = content
        .get("body")
        .and_then(|b| b.as_str())
        .unwrap_or("")
        .to_string();
    let ts = v
        .get("origin_server_ts")
        .and_then(|t| t.as_i64())
        .unwrap_or(0);
    let mid = v
        .get("event_id")
        .and_then(|m| m.as_str())
        .unwrap_or("")
        .to_string();
    Some(IncomingMessage {
        room: room_id.to_string(),
        mid,
        sender,
        body,
        ts,
        timing: TimingEvent::from_matrix_content(content),
        raw: v.to_string(),
    })
}
