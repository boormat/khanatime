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

use crate::Model;

#[cfg(target_arch = "wasm32")]
use crate::app::ConnState;
#[cfg(target_arch = "wasm32")]
use std::rc::Rc;

/// Connection actions driven from the Home page.
#[derive(Clone)]
pub enum Msg {
    Connect,
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
                    .await?;
            crate::services::matrix::set_room(Some(room.clone()));
            crate::services::matrix::start_sync(client, sink_for(model));
            spawn_backfill(model);
            Ok::<_, String>(room.room_id().to_string())
        }
        .await;
        match res {
            Ok(room_id) => {
                model.app.identity.set(stored.user_id.clone());
                model.app.conn.set(ConnState::LoggedIn(stored.user_id));
                model.app.room.set(Some(room_id));
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
        match crate::services::matrix::join_room_for_event(&client, &model.app.event.get_clone())
            .await
        {
            Ok(room) => {
                crate::services::matrix::set_room(Some(room.clone()));
                model.app.room.set(Some(room.room_id().to_string()));
                spawn_backfill(model);
                flush_pending(model);
            }
            Err(e) => model.app.conn.set(ConnState::Error(e)),
        }
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
    let pending = crate::log::load_pending(&id);
    if id.is_empty() || pending.is_empty() {
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
        crate::app::refresh_feed(model);
    });
}

#[cfg(target_arch = "wasm32")]
fn update_wasm(model: Model, msg: Msg) {
    match msg {
        Msg::Connect => connect(model),
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
                    .await?;
            crate::services::matrix::set_room(Some(room.clone()));
            crate::services::matrix::start_sync(client, sink_for(model));
            spawn_backfill(model);
            Ok::<_, String>(room.room_id().to_string())
        }
        .await;
        match res {
            Ok(room_id) => {
                let user_id = crate::services::matrix::client()
                    .and_then(|c| c.user_id().map(|u| u.to_string()))
                    .unwrap_or_else(|| user.clone());
                model.app.identity.set(user_id.clone());
                model.app.conn.set(ConnState::LoggedIn(user_id));
                model.app.room.set(Some(room_id));
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

    // Durable record: append to the log, then drop any outbox echo.
    let log_msg = crate::log::LogMsg::from_room(
        msg.mid.clone(),
        msg.ts,
        msg.sender.clone(),
        msg.body.clone(),
        msg.raw.clone(),
    );
    if crate::log::append_log(&id, log_msg) {
        crate::log::reconcile(&id);
    }

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
