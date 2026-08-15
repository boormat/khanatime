//! Matrix connection + sync plumbing (wasm only).
//!
//! Non-UI home for what the old Sync page used to do: connect/register/logout,
//! resume the persisted session, join the current event's timing room, run the
//! merge sink that folds room history (live + backfill) into local state, and
//! flush the local pending outbox to the room.  The Home page owns the
//! connection form; the Chat page shows the transaction log (room history +
//! pending) read-only.
//!
//! The room is the durable store; local state is rebuilt from the message log
//! (`log.rs`).  Outgoing messages land in the pending outbox first and are
//! flushed here; once the room acks them they're promoted into the log.
//!
//! Offline handoff: a device can export the event's log as a QR parcel
//! (`services::qr` — see [export_parcel]) and another can [import_parcel] it
//! without any network.  Exported messages become QR-origin log entries; on
//! reconnect [relay_to_room] re-broadcasts anything not yet confirmed in the
//! connected room, so a parcel and a room converge on the same log.  Content-id
//! dedup keeps every path idempotent.

use crate::Model;

#[cfg(target_arch = "wasm32")]
use crate::app::ConnState;
#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;
#[cfg(target_arch = "wasm32")]
use std::rc::Rc;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

/// Connection actions driven from the Home page.
#[derive(Clone)]
pub enum Msg {
    Connect,
    /// Browser-based OAuth/SSO sign-in (passwordless matrix.org accounts).
    SsoLogin,
    Logout,
}

pub fn update(model: Model, msg: Msg) {
    #[cfg(target_arch = "wasm32")]
    update_wasm(model, msg);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (model, msg);
}

// ----- lifecycle (wasm) -----

/// Resume a persisted Matrix session on app load (wasm only).
#[cfg(target_arch = "wasm32")]
pub fn resume_on_load(model: Model) {
    // Demo events are local-only: never connect or join a room for them.
    if model.app.event.with(|e| e.is_demo()) {
        return;
    }
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
                    .await;
            crate::services::matrix::set_room(room.clone());
            crate::services::matrix::start_sync(client, sink_for(model));
            if room.is_some() {
                spawn_backfill(model);
            }
            Ok::<_, String>(room.map(|r| r.room_id().to_string()))
        }
        .await;
        match res {
            Ok(room_id) => {
                model.app.identity.set(stored.user_id.clone());
                model.app.conn.set(ConnState::LoggedIn(stored.user_id));
                model.app.room.set(room_id);
                flush_pending(model);
            }
            Err(e) => model.app.conn.set(ConnState::Error(e)),
        }
    });
}

/// Re-join the room for the currently selected event (after switching event).
#[cfg(target_arch = "wasm32")]
pub fn join_current_event(model: Model) {
    // Demo events are local-only: never join a timing room for them.
    if model.app.event.with(|e| e.is_demo()) {
        return;
    }
    let Some(client) = crate::services::matrix::client() else {
        return;
    };
    wasm_bindgen_futures::spawn_local(async move {
        let room =
            crate::services::matrix::join_room_for_event(&client, &model.app.event.get_clone())
                .await;
        crate::services::matrix::set_room(room.clone());
        model
            .app
            .room
            .set(room.as_ref().map(|r| r.room_id().to_string()));
        if room.is_some() {
            spawn_backfill(model);
        }
        flush_pending(model);
    });
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub fn resume_on_load(_model: Model) {}

#[cfg(not(target_arch = "wasm32"))]
pub fn join_current_event(_model: Model) {}

// ----- outgoing (wasm) -----

/// Send every unsent outbox message for the current event to its room, oldest
/// first, promoting each into the log on ack.  Stops at the first failure (the
/// rest stay pending and are retried on the next connect).  No-op when
/// disconnected, and on native builds.
pub fn flush_pending(model: Model) {
    #[cfg(target_arch = "wasm32")]
    flush_pending_wasm(model);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = model;
}

#[cfg(target_arch = "wasm32")]
fn flush_pending_wasm(model: Model) {
    let Some(room) = crate::services::matrix::room() else {
        return;
    };
    let id = model.app.event.with(|e| e.id.clone());
    if id.is_empty() {
        return;
    }
    let pending = crate::log::load_pending(&id);
    if pending.is_empty() {
        // Nothing to flush, but QR-parcel entries may still need relaying.
        relay_to_room(model);
        return;
    }
    wasm_bindgen_futures::spawn_local(async move {
        for msg in pending {
            match crate::services::matrix::send_log_message(&room, &msg).await {
                Ok(mid) => {
                    crate::log::promote(&id, &msg.local_id, &mid);
                }
                Err(e) => {
                    khanatime::log!("flush stopped: {e}");
                    break;
                }
            }
        }
        crate::log::reconcile(&id);
        relay_to_room(model);
        crate::app::refresh_feed(model);
    });
}

/// Re-broadcast log entries that aren't confirmed in the connected room yet:
/// QR-parcel imports (and, later, a second room's messages) get relayed to the
/// current timing room, oldest first.  Each successful send confirms the entry
/// in the room (real mid), so a reconnect won't re-send it.  Runs after the
/// outbox flush — those messages are still pending with `origin == ""` and are
/// skipped here to avoid double-sending.
#[cfg(target_arch = "wasm32")]
pub fn relay_to_room(model: Model) {
    let Some(room) = crate::services::matrix::room() else {
        return;
    };
    let id = model.app.event.with(|e| e.id.clone());
    if id.is_empty() {
        return;
    }
    let room_id = room.room_id().to_string();
    let log = crate::log::load_log(&id);
    let pending: Vec<usize> = log
        .iter()
        .enumerate()
        .filter(|(_, m)| !m.origin.is_empty() && m.origin != room_id)
        .map(|(i, _)| i)
        .collect();
    if pending.is_empty() {
        return;
    }
    wasm_bindgen_futures::spawn_local(async move {
        for idx in pending {
            let Some(msg) = log.get(idx) else {
                continue;
            };
            match crate::services::matrix::send_log_message(&room, msg).await {
                Ok(mid) => {
                    crate::log::confirm_in_room(
                        &id,
                        &crate::ids::content_id(&msg.body),
                        &mid,
                        &room_id,
                    );
                }
                Err(e) => {
                    khanatime::log!("relay stopped: {e}");
                    break;
                }
            }
        }
        crate::app::refresh_feed(model);
    });
}

// ----- QR parcel handoff (works offline) -----

/// Pack the current event's whole durable log into a QR parcel and stage it on
/// `model.app.parcel_export`.  Exporting first promotes the local outbox
/// (`publish_outbox`) — handing a message off is publishing it, so unsent
/// entries leave the outbox and are relayed to the room later instead of being
/// stuck locally.
pub fn export_parcel(model: Model) {
    let (id, uid) = model.app.event.with(|e| (e.id.clone(), e.uid.clone()));
    if id.is_empty() {
        model
            .app
            .parcel_status
            .set("No event loaded to export.".to_string());
        return;
    }
    let moved = crate::log::publish_outbox(&id);
    let log = crate::log::load_log(&id);
    // Full = whole log (bootstrap); Timing only = just the KT timing records,
    // for a receiver that already has the event.
    let msgs = if model.app.parcel_mode.get() == crate::app::ParcelMode::TimingOnly {
        crate::services::qr::filter_timing(&log)
    } else {
        log
    };
    let text = crate::services::qr::pack_parcel(&uid, &msgs);
    model.app.parcel_export.set(text.clone());
    // QR frames carry the compressed payload: base64(deflate(json)).
    let payload = crate::services::qr::parcel_payload(&text);
    let frames = crate::services::qr::pack_frames(&payload);
    let svgs = crate::services::qr::qr_svgs(&frames, crate::services::qr::MIN_MODULE_PX);
    model.app.parcel_qr_svgs.set(svgs.clone());
    model.app.parcel_qr_total.set(svgs.len());
    model.app.parcel_qr_index.set(0);
    model.app.parcel_qr_paused.set(false);
    #[cfg(target_arch = "wasm32")]
    start_qr_animation(model, svgs.len());
    let n = msgs.len();
    let kind = match model.app.parcel_mode.get() {
        crate::app::ParcelMode::Full => "full event",
        crate::app::ParcelMode::TimingOnly => "timing",
    };
    let moved_note = if moved > 0 {
        format!(" ({moved} unsent moved into the handoff)")
    } else {
        String::new()
    };
    model.app.parcel_status.set(format!(
        "{n} {kind} messages{moved_note}, {frame_count} QR — scan or copy.",
        frame_count = svgs.len()
    ));
}

/// Cycle the exported QR display through its frames on a timer.
#[cfg(target_arch = "wasm32")]
fn start_qr_animation(model: Model, n: usize) {
    if n <= 1 {
        return;
    }
    clear_qr_timer();
    let Some(window) = web_sys::window() else {
        return;
    };
    let closure = wasm_bindgen::closure::Closure::<dyn FnMut()>::wrap(Box::new(move || {
        if !model.app.parcel_qr_paused.get() {
            model.app.parcel_qr_index.update(|i| *i = (*i + 1) % n);
        }
    }));
    let id = window
        .set_interval_with_callback_and_timeout_and_arguments_0(
            closure.as_ref().unchecked_ref(),
            1300,
        )
        .unwrap_or_default();
    closure.forget();
    QR_TIMER.with(|t| *t.borrow_mut() = Some(id));
}

#[cfg(target_arch = "wasm32")]
fn clear_qr_timer() {
    if let Some(id) = QR_TIMER.with(|t| t.borrow_mut().take()) {
        if let Some(window) = web_sys::window() {
            window.clear_interval_with_handle(id);
        }
    }
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static QR_TIMER: std::cell::RefCell<Option<i32>> = const { std::cell::RefCell::new(None) };
}

/// Pause or resume the animated QR export display at its current frame.
pub fn toggle_qr_pause(model: Model) {
    model.app.parcel_qr_paused.update(|p| *p = !*p);
}

/// Clear the QR export display (stops the animation and hides the codes).
pub fn clear_qr(model: Model) {
    #[cfg(target_arch = "wasm32")]
    clear_qr_timer();
    model.app.parcel_qr_svgs.set(Vec::new());
    model.app.parcel_qr_total.set(0);
    model.app.parcel_qr_index.set(0);
    model.app.parcel_qr_paused.set(false);
}

/// Import a QR parcel: parse it, gate on the current event's uid, then append
/// each message to the durable log exactly like a room message (content-id
/// dedup makes re-import idempotent).  When the parcel names a different event
/// the user is warned and offered an open-and-import button (a parcel for the
/// current event imports straight in).
pub fn import_parcel(model: Model) {
    let id = model.app.event.with(|e| e.id.clone());
    let uid = model.app.event.with(|e| e.uid.clone());
    let Some(parcel) = parse_parcel_text(model) else {
        return;
    };
    // No current event, or the parcel names a different one (including an
    // unidentified draft, uid empty): offer to open the parcel's own event and
    // import there.  A parcel whose event is the current one imports directly.
    if id.is_empty() || parcel.event_uid != uid {
        match parcel_event_name(&parcel) {
            Some((eid, name)) => {
                model.app.parcel_open_event.set(Some((eid, name.clone())));
                model.app.parcel_status.set(format!(
                    "This parcel is for \"{name}\" — open it to import."
                ));
            }
            None => {
                model.app.parcel_open_event.set(None);
                if id.is_empty() {
                    model
                        .app
                        .parcel_status
                        .set("Open the event to import into first.".to_string());
                } else {
                    model.app.parcel_status.set(format!(
                        "This parcel is for a different event (uid {}) — open that event first.",
                        parcel.event_uid
                    ));
                }
            }
        }
        return;
    }
    apply_parcel(model, &id, &parcel);
}

/// Import a parcel string decoded directly (e.g. by the camera scanner),
/// reusing the paste path so gating + replay behave identically.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // wasm scan sink
pub fn import_parcel_text(model: Model, text: &str) {
    model.app.parcel_import.set(text.to_string());
    import_parcel(model);
}

/// Open the event a mismatched parcel belongs to and import it there.  Used by
/// the handoff's "Open <event> and import" button: after opening, the parcel
/// imports without the uid gate (the fresh event has no uid until its setup
/// manifest is imported).
pub fn open_parcel_event(model: Model) {
    let Some(parcel) = parse_parcel_text(model) else {
        return;
    };
    let Some((eid, name)) = parcel_event_name(&parcel) else {
        model
            .app
            .parcel_status
            .set("This parcel has no setup manifest — can't tell which event to open.".into());
        model.app.parcel_open_event.set(None);
        return;
    };
    crate::update(model, crate::Msg::SetEvent(eid.clone()));
    apply_parcel(model, &eid, &parcel);
    model.app.parcel_open_event.set(None);
    model
        .app
        .parcel_status
        .set(format!("Opened {name} and imported the parcel."));
}

/// Read and parse the staged parcel text, surfacing errors on the status line.
fn parse_parcel_text(model: Model) -> Option<crate::services::qr::Parcel> {
    let text = model.app.parcel_import.get_clone();
    if text.trim().is_empty() {
        model
            .app
            .parcel_status
            .set("Paste or scan a parcel first.".to_string());
        return None;
    }
    match crate::services::qr::unpack_parcel(&text) {
        Ok(p) => Some(p),
        Err(e) => {
            model.app.parcel_status.set(format!("Import failed: {e}"));
            None
        }
    }
}

/// The `(event id, name)` a parcel belongs to, from its setup manifest.
fn parcel_event_name(parcel: &crate::services::qr::Parcel) -> Option<(String, String)> {
    parcel.msgs.iter().find_map(|pm| {
        if pm
            .body
            .starts_with(crate::timing_event::TimingEvent::SETUP_PREFIX)
        {
            crate::event::from_setup_body(&pm.body).map(|e| (e.id, e.name))
        } else {
            None
        }
    })
}

/// Append a parcel's messages to an event's log and rebuild local state.
fn apply_parcel(model: Model, id: &str, parcel: &crate::services::qr::Parcel) {
    let mut added = 0;
    for pm in &parcel.msgs {
        let msg = crate::log::LogMsg::from_parcel(pm.body.clone(), pm.ts, pm.sender.clone());
        if crate::log::append_log(id, msg) {
            added += 1;
        }
    }
    crate::log::reconcile(id);
    if added > 0 {
        // Rebuild event/scores/runs from the now-merged log, like SetEvent.
        let (event, scores, runs) =
            crate::replay::replay(&crate::log::load_log(id), &crate::log::load_pending(id));
        model.app.event.set(event);
        model.app.scores.set(scores);
        model.app.runs.set(runs);
        crate::app::refresh_feed(model);
    }
    model.app.parcel_import.set(String::new());
    model.app.parcel_status.set(if added == 0 {
        "Nothing new — this parcel was already imported.".to_string()
    } else {
        format!("Imported {added} messages.")
    });
    crate::update(model, crate::Msg::Reload);
}
#[cfg(target_arch = "wasm32")]
fn update_wasm(model: Model, msg: Msg) {
    match msg {
        Msg::Connect => connect(model),
        Msg::SsoLogin => sso_login(model),
        Msg::Logout => logout(model),
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
    let sm = model.screens.home;
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
                    .await;
            crate::services::matrix::set_room(room.clone());
            crate::services::matrix::start_sync(client, sink_for(model));
            if room.is_some() {
                spawn_backfill(model);
            }
            Ok::<_, String>(room.map(|r| r.room_id().to_string()))
        }
        .await;
        match res {
            Ok(room_id) => {
                let user_id = crate::services::matrix::client()
                    .and_then(|c| c.user_id().map(|u| u.to_string()))
                    .unwrap_or_else(|| user.clone());
                model.app.identity.set(user_id.clone());
                model.app.conn.set(ConnState::LoggedIn(user_id));
                model.app.room.set(room_id);
                // Land on the Home dashboard (event status hub) once connected.
                crate::update(model, crate::Msg::Show(crate::Screen::Home));
                flush_pending(model);
            }
            Err(e) => model.app.conn.set(ConnState::Error(e)),
        }
        sm.busy.set(false);
    });
}

#[cfg(target_arch = "wasm32")]
fn logout(model: Model) {
    let sm = model.screens.home;
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
        model.screens.chat.feed.set(Vec::new());
        model
            .screens
            .chat
            .expanded
            .set(std::collections::HashSet::new());
        sm.busy.set(false);
    });
}

// ----- OAuth / SSO -----

/// Probe the current homeserver for OIDC support and expose it on the Home
/// form (toggles the SSO button).  Called when the homeserver field changes.
#[cfg(target_arch = "wasm32")]
pub fn probe_oidc(model: Model) {
    let sm = model.screens.home;
    let hs = sm.homeserver.get_clone();
    wasm_bindgen_futures::spawn_local(async move {
        let ok = match crate::services::matrix::new_client(&hs).await {
            Ok(client) => crate::services::matrix::oidc_supported(&client).await,
            Err(_) => false,
        };
        sm.sso.set(ok);
    });
}

/// Start the OAuth/SSO sign-in: build the authorization URL and open it in a
/// new browser tab.  The tab finishes the login at the homeserver and is
/// redirected back to this app with `?code&state`; [sso_wait_for_callback]
/// picks that up over a BroadcastChannel named by the state token.
#[cfg(target_arch = "wasm32")]
fn sso_login(model: Model) {
    let sm = model.screens.home;
    let hs = sm.homeserver.get_clone();
    sm.busy.set(true);
    model.app.conn.set(ConnState::Connecting);
    // Open the tab synchronously from the click handler — popup blockers
    // reject `window.open` after an await.  Its URL is set once the
    // authorization URL is built (a few network round trips later).
    let tab = match web_sys::window().map(|w| w.open()) {
        Some(Ok(Some(tab))) => tab,
        _ => {
            model.app.conn.set(ConnState::Error(
                "couldn't open the sign-in tab — allow popups for this site".to_string(),
            ));
            sm.busy.set(false);
            return;
        }
    };
    wasm_bindgen_futures::spawn_local(async move {
        let res = async {
            let client = crate::services::matrix::new_client(&hs).await?;
            if !crate::services::matrix::oidc_supported(&client).await {
                return Err(
                    "This homeserver doesn't offer SSO sign-in — use a username and password"
                        .to_string(),
                );
            }
            let redirect_uri = crate::services::matrix::oauth_redirect_uri()?;
            // MAS (matrix.org's auth server) refuses to register clients whose
            // redirect URIs are http or on localhost, so SSO only works when the
            // app is served from a real https origin (the deployed app).  Fail
            // with a clear message instead of a raw 400.
            if redirect_uri.scheme() != "https"
                || matches!(
                    redirect_uri.host_str(),
                    Some("localhost" | "127.0.0.1" | "::1" | "[::1]")
                )
            {
                return Err(
                    "SSO sign-in only works from a real https address — matrix.org rejects http/localhost redirects. Use the dev server's username/password, or test SSO on the deployed app."
                        .to_string(),
                );
            }
            let data = crate::services::matrix::oauth_client_data(&client, &redirect_uri).await?;
            let auth = client
                .oauth()
                .login(redirect_uri, None, data, None)
                .build()
                .await
                .map_err(|e| e.to_string())?;
            Ok::<_, String>((client, auth))
        }
        .await;
        match res {
            Ok((client, auth)) => {
                crate::services::matrix::set_client(Some(client));
                let state = auth.state.secret().to_string();
                let _ = tab.location().set_href(&auth.url.to_string());
                model.app.conn.set(ConnState::SsoPending);
                // Not blocking: the user is in the sign-in tab now; if it never
                // completes they can retry with the password path.
                sm.busy.set(false);
                sso_wait_for_callback(model, &state);
            }
            Err(e) => {
                model.app.conn.set(ConnState::Error(e));
                sm.busy.set(false);
            }
        }
    });
}

// Keeps the BroadcastChannel (and its message listener) alive for the SSO
// wait; dropping it would close the channel and drop the callback.
#[cfg(target_arch = "wasm32")]
thread_local! {
    static SSO_CHANNEL: RefCell<Option<web_sys::BroadcastChannel>> =
        const { RefCell::new(None) };
}

/// Listen on a BroadcastChannel named by the OAuth `state` token.  The tab we
/// opened posts the callback URL back here when the homeserver redirects to it.
#[cfg(target_arch = "wasm32")]
fn sso_wait_for_callback(model: Model, state: &str) {
    use wasm_bindgen::JsCast;
    let channel = web_sys::BroadcastChannel::new(state).expect("broadcast channel");
    let on_msg = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::MessageEvent)>::wrap(
        Box::new(move |ev: web_sys::MessageEvent| {
            let data = ev.data().as_string().unwrap_or_default();
            sso_complete(model, data);
        }),
    );
    channel.set_onmessage(Some(on_msg.as_ref().unchecked_ref()));
    on_msg.forget();
    SSO_CHANNEL.with(|c| *c.borrow_mut() = Some(channel));
}

/// Complete the OAuth login with the callback URL the sign-in tab posted, then
/// proceed exactly like a password connect (identity, room, sync, backfill).
#[cfg(target_arch = "wasm32")]
fn sso_complete(model: Model, callback_url: String) {
    wasm_bindgen_futures::spawn_local(async move {
        let res = async {
            let Some(client) = crate::services::matrix::client() else {
                return Err("sign-in session lost — try again".to_string());
            };
            crate::services::matrix::finish_oauth_login(&client, &callback_url).await?;
            crate::services::matrix::save_session(&client, &client.homeserver().to_string());
            let room =
                crate::services::matrix::join_room_for_event(&client, &model.app.event.get_clone())
                    .await;
            crate::services::matrix::set_room(room.clone());
            crate::services::matrix::start_sync(client, sink_for(model));
            if room.is_some() {
                spawn_backfill(model);
            }
            Ok::<_, String>(room.map(|r| r.room_id().to_string()))
        }
        .await;
        match res {
            Ok(room_id) => {
                let user_id = crate::services::matrix::client()
                    .and_then(|c| c.user_id().map(|u| u.to_string()))
                    .unwrap_or_default();
                model.app.identity.set(user_id.clone());
                model.app.conn.set(ConnState::LoggedIn(user_id));
                model.app.room.set(room_id);
                crate::update(model, crate::Msg::Show(crate::Screen::Home));
                flush_pending(model);
            }
            Err(e) => model.app.conn.set(ConnState::Error(e)),
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub fn probe_oidc(_model: Model) {}

// ----- merge sink -----

/// Sink: every room message is stored in the event log and merged into derived
/// state (setup / runs / scores).  The log is the durable record; the feed and
/// results are rebuilt from it via `refresh_feed` / `Reload`.
#[cfg(target_arch = "wasm32")]
fn sink_for(model: Model) -> Rc<dyn Fn(crate::services::matrix::IncomingMessage)> {
    Rc::new(move |msg| handle_incoming(model, msg))
}

#[cfg(target_arch = "wasm32")]
fn spawn_backfill(model: Model) {
    wasm_bindgen_futures::spawn_local(async move {
        let (Some(client), Some(room)) = (
            crate::services::matrix::client(),
            crate::services::matrix::room(),
        ) else {
            return;
        };
        let sink = sink_for(model);
        if let Err(e) = crate::services::matrix::backfill_room_history(&client, &room, &*sink).await
        {
            khanatime::log!("matrix backfill error: {e}");
        }
        crate::log::reconcile(&model.app.event.with(|e| e.id.clone()));
        crate::app::refresh_feed(model);
    });
}

/// Merge an incoming room message into local state.  The message is appended to
/// the event's durable log (deduped by Matrix event id); an echo of our own
/// pending outbox is dropped from the outbox via `reconcile`.
#[cfg(target_arch = "wasm32")]
fn handle_incoming(model: Model, msg: crate::services::matrix::IncomingMessage) {
    // Scoped by room: the app only ever joins the selected event's timing room,
    // and the room id check drops stragglers from a previous event.
    let Some(room) = crate::services::matrix::room() else {
        return;
    };
    if msg.room.as_str() != room.room_id().as_str() {
        return;
    }
    let id = model.app.event.with(|e| e.id.clone());
    if id.is_empty() {
        return;
    }

    // Durable record: append to the log, then drop any outbox echo.  A message
    // that matches a QR-parcel entry by content id is the relay echo (or a
    // copy handed in from another parcel) — confirm the existing entry in the
    // room instead of duplicating it.
    let room_id = room.room_id().as_str();
    let log_msg = crate::log::LogMsg::from_room(
        msg.mid.clone(),
        msg.ts,
        msg.sender.clone(),
        msg.body.clone(),
        msg.raw.clone(),
        room_id,
    );
    if crate::log::confirm_in_room(&id, &crate::ids::content_id(&msg.body), &msg.mid, room_id) {
        // Relay ack was lost; this echo confirms the parcel entry in the room.
    } else if crate::log::append_log(&id, log_msg) {
        crate::log::reconcile(&id);
    }
    crate::app::refresh_feed(model);

    // Event setup manifest: adopt last-writer-wins when ids match (fresh
    // devices start with an empty local event).
    if msg
        .body
        .starts_with(crate::timing_event::TimingEvent::SETUP_PREFIX)
    {
        if let Some(incoming) = crate::event::from_setup_body(&msg.body) {
            model.app.event.update(|e| {
                crate::event::merge_setup(e, &incoming);
            });
        }
        crate::update(model, crate::Msg::Reload);
        return;
    }

    // Per-entry state message: upsert (or tombstone) in the local event, the
    // same way replay would.  Guarded by event uid like setup.
    if msg
        .body
        .starts_with(crate::timing_event::TimingEvent::ENTRY_PREFIX)
    {
        if let Some(entry_msg) = crate::event::from_entry_body(&msg.body) {
            model.app.event.update(|e| {
                if e.uid.is_empty() {
                    e.uid = entry_msg.event_id.clone();
                }
                if entry_msg.event_id == e.uid {
                    if entry_msg.delete {
                        e.remove_entry(entry_msg.entry.entry_no);
                    } else {
                        e.upsert_entry(entry_msg.entry);
                    }
                }
            });
            crate::update(model, crate::Msg::Reload);
        }
        return;
    }

    let Some(te) = msg.timing else {
        return; // plain chat / results-snapshot messages: log-only
    };

    if te.r#type == crate::event::RUN_START || te.r#type == crate::event::RUN_FINISH {
        // Mirror the remote run into local state (run numbering,
        // pending-starts) so Start/Finish screens stay live.
        let run = crate::event::record_from_timing(&te);
        model.app.runs.update(|runs| {
            crate::event::add_run(runs, run);
        });
    }
    if te.r#type == crate::event::RUN_START && te.status.as_deref() == Some("dns") {
        // A no-show start scores NOSHO so the results cell reads "DNS".
        model.app.scores.update(|s| {
            crate::event::upsert_ktime(s, te.test, &te.car, crate::event::KTime::NOSHO);
        });
    }
    if te.r#type == crate::event::RUN_FINISH {
        // Full KTime: keeps DNF/FTS/WD/NOSHO and penalty flags intact.
        let run = crate::event::record_from_timing(&te);
        let kt = crate::event::finish_to_ktime(&run);
        model.app.scores.update(|s| {
            crate::event::upsert_ktime(s, te.test, &te.car, kt);
        });
    }
    crate::update(model, crate::Msg::Reload);
}
