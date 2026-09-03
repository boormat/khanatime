use sycamore::prelude::*;

use crate::app::ConnState;

// Events hub: pick how to get an event open.
//
// - Load the local demo event for training officials (never published).
// - Pick a published event from Matrix (search by default; QR / room-id are
//   placeholders).
// - Plan a new event (opens the Event admin draft form).
// - Re-open any event saved on this device.

#[derive(Clone, Copy)]
pub struct Model {
    pub search_term: Signal<String>,
    pub search_busy: Signal<bool>,
    pub search_results: Signal<Vec<SearchResult>>,
    pub search_msg: Signal<String>,
    pub feedback: Signal<String>,
}

/// A published event found in the room directory (name, alias, room id).
#[derive(Clone, PartialEq)]
pub struct SearchResult {
    pub name: String,
    pub alias: String,
    pub room_id: String,
}

pub fn init() -> Model {
    Model {
        search_term: create_signal(String::new()),
        search_busy: create_signal(false),
        search_results: create_signal(Vec::new()),
        search_msg: create_signal(String::new()),
        feedback: create_signal(String::new()),
    }
}

#[derive(Clone)]
pub enum Msg {
    Search,
    OpenResult(String),
    ScanQr,
    EnterRoomId,
    PlanNew,
}

pub fn update(model: crate::Model, msg: Msg) {
    match msg {
        Msg::Search => {
            #[cfg(target_arch = "wasm32")]
            search(model);
            #[cfg(not(target_arch = "wasm32"))]
            let _ = model;
        }
        Msg::OpenResult(room_id) => {
            #[cfg(target_arch = "wasm32")]
            open_result(model, room_id);
            #[cfg(not(target_arch = "wasm32"))]
            {
                let _ = (model, room_id);
            }
        }
        Msg::ScanQr => {
            model.screens.events.feedback.set(
                "QR scanning is coming soon — search or join from the room list for now.".into(),
            );
        }
        Msg::EnterRoomId => {
            model.screens.events.feedback.set(
                "Joining by room id is coming soon — use Search to find a published event.".into(),
            );
        }
        Msg::PlanNew => {
            crate::update(
                model,
                crate::Msg::EventMsg(crate::khana::page::event::Msg::CreateDraft),
            );
            crate::update(model, crate::Msg::Show(crate::Screen::Event));
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn search(model: crate::Model) {
    let em = model.screens.events;
    let Some(client) = crate::services::matrix::client() else {
        em.feedback
            .set("Log in on the Home page first, then search.".to_string());
        return;
    };
    let term = em.search_term.get_clone();
    if term.trim().is_empty() {
        em.feedback
            .set("Type something to search for (e.g. a club or year).".to_string());
        return;
    }
    em.search_busy.set(true);
    em.search_msg.set(String::new());
    em.feedback.set(String::new());
    wasm_bindgen_futures::spawn_local(async move {
        let res = crate::services::matrix::search_events(&client, &term).await;
        em.search_busy.set(false);
        match res {
            Ok(results) => {
                let mapped: Vec<SearchResult> = results
                    .into_iter()
                    .map(|r| SearchResult {
                        name: r.name,
                        alias: r.alias,
                        room_id: r.room_id,
                    })
                    .collect();
                em.search_results.set(mapped.clone());
                em.search_msg.set(if mapped.is_empty() {
                    "No published events found.".to_string()
                } else {
                    format!("{} event(s) found.", mapped.len())
                });
            }
            Err(e) => em.feedback.set(format!("Search failed: {e}")),
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn open_result(model: crate::Model, room_id: String) {
    let em = model.screens.events;
    let Some(client) = crate::services::matrix::client() else {
        em.feedback
            .set("Log in on the Home page first.".to_string());
        return;
    };
    em.search_busy.set(true);
    em.feedback.set(String::new());
    wasm_bindgen_futures::spawn_local(async move {
        let res = crate::services::matrix::open_published_event(&client, &room_id).await;
        em.search_busy.set(false);
        match res {
            Ok(ev) => {
                // Seed into the durable log (not the outbox) so the adopted
                // setup replays into a current event without being re-broadcast.
                crate::log::seed_setup_to_log(&ev.id, &crate::event::setup_body(&ev), "");
                crate::update(model, crate::Msg::SetEvent(ev.id));
                crate::update(model, crate::Msg::Show(crate::Screen::Home));
            }
            Err(e) => em.feedback.set(format!("Couldn't open event: {e}")),
        }
    });
}

pub fn view(model: crate::Model) -> View {
    view! {
        div {
            h1(class="title") { "Events" }
            p(class="help") { "Choose how to get an event open." }
            (view_current(model))
            (view_open_events(model))
            (view_published(model))
            (view_plan(model))
            (crate::khana::helpers::view_handoff(model))
            (view_feedback(model))
        }
    }
}

/// The event that's currently open, if any.
fn view_current(model: crate::Model) -> View {
    let (id, name, status, demo) = model.khana.event.with(|e| {
        (
            e.id.clone(),
            e.name.clone(),
            e.status.to_string(),
            e.is_demo(),
        )
    });
    if id.is_empty() {
        return view! {
            div(class="box") {
                h2(class="title is-5") { "Current event" }
                p(class="help") { "No event open." }
            }
        };
    }
    view! {
        div(class="box") {
            h2(class="title is-5") {
                "Current event"
                (if demo {
                    view! { span(class="tag is-warning is-pulled-right") { "DEMO" } }
                } else {
                    view! {}
                })
            }
            div(class="field is-grouped") {
                div(class="control is-expanded") {
                    p(class="has-text-weight-medium") { (name) }
                    span(class="tag is-light") { (status) }
                }
                div(class="control") {
                    button(
                        class="button is-small is-link",
                        on:click=move |_| crate::update(model, crate::Msg::Show(crate::Screen::Event)),
                    ) {
                        "Event admin"
                    }
                }
                div(class="control") {
                    button(
                        class="button is-small is-danger is-outlined",
                        on:click=move |_| crate::update(model, crate::Msg::ClearEvent),
                    ) {
                        span(class="icon is-small") { i(class="fa fa-xmark") }
                        span { "Close" }
                    }
                }
            }
        }
    }
}

/// Search the Matrix room directory for a published event.
fn view_published(model: crate::Model) -> View {
    let em = model.screens.events;
    let logged_in = matches!(model.sync.conn.get_clone(), ConnState::LoggedIn(_));
    view! {
        div(class="box") {
            h2(class="title is-5") {
                "Pick a published event"
                span(class="tag is-light is-pulled-right") { "Matrix" }
            }
            (if !logged_in {
                view! {
                    p(class="help") {
                        "Log in on the Home page first to search the room directory."
                    }
                }
            } else {
                view! {
                    div(class="field has-addons") {
                        div(class="control is-expanded") {
                            input(
                                class="input",
                                placeholder="Search… e.g. club or year",
                                bind:value=em.search_term,
                                on:keydown=move |ev: web_sys::KeyboardEvent| {
                                    if ev.key_code() == 13 {
                                        crate::update(model, crate::Msg::EventsMsg(Msg::Search));
                                    }
                                },
                            )
                        }
                        div(class="control") {
                            button(
                                class="button is-link",
                                disabled=em.search_busy.get(),
                                on:click=move |_| crate::update(model, crate::Msg::EventsMsg(Msg::Search)),
                            ) {
                                (if em.search_busy.get() { "Searching…" } else { "Search" })
                            }
                        }
                    }
                    (move || view_search_results(model))
                    div(class="field is-grouped") {
                        div(class="control") {
                            button(
                                class="button is-small is-light",
                                on:click=move |_| crate::update(model, crate::Msg::EventsMsg(Msg::ScanQr)),
                            ) {
                                span(class="icon is-small") { i(class="fa fa-qrcode") }
                                span { "Scan QR code" }
                            }
                        }
                        div(class="control") {
                            button(
                                class="button is-small is-light",
                                on:click=move |_| crate::update(model, crate::Msg::EventsMsg(Msg::EnterRoomId)),
                            ) {
                                span(class="icon is-small") { i(class="fa fa-hashtag") }
                                span { "Enter room id" }
                            }
                        }
                    }
                    (move || {
                        let msg = em.search_msg.get_clone();
                        if msg.is_empty() {
                            view! { p(class="help") { "Search is the default. QR scanning and room-id entry are coming soon." } }
                        } else {
                            view! { p(class="help") { (msg) } }
                        }
                    })
                }
            })
        }
    }
}

fn view_search_results(model: crate::Model) -> View {
    let em = model.screens.events;
    let results = em.search_results.get_clone();
    if results.is_empty() {
        return view! {};
    }
    let rows: Vec<View> = results
        .iter()
        .map(|r| {
            let alias = r.alias.clone();
            let name = r.name.clone();
            let open_id = r.room_id.clone();
            view! {
                div(class="field is-grouped") {
                    div(class="control is-expanded") {
                        p(class="has-text-weight-medium") { (name) }
                        span(class="tag is-light") { (alias) }
                    }
                    div(class="control") {
                        button(
                            class="button is-small is-link",
                            disabled=em.search_busy.get(),
                            on:click=move |_| crate::update(model, crate::Msg::EventsMsg(Msg::OpenResult(open_id.clone()))),
                        ) {
                            "Open"
                        }
                    }
                }
            }
        })
        .collect();
    view! { (rows) }
}

/// Plan a new event — jumps to the Event admin draft form.  Requires an
/// identity: every event must have a user/owner, so sign in first.
fn view_plan(model: crate::Model) -> View {
    let has_identity = model.sync.app_identity.with(|a| !a.is_empty());
    view! {
        div(class="box") {
            h2(class="title is-5") { "Plan a new event" }
            p(class="help") {
                "Starts a local draft with defaults you can edit and save, then publish to Matrix when timing starts."
            }
            div(class="field") {
                div(class="control") {
                    button(
                        class="button is-link",
                        disabled=!has_identity,
                        on:click=move |_| crate::update(model, crate::Msg::EventsMsg(Msg::PlanNew)),
                    ) {
                        span(class="icon is-small") { i(class="fa fa-plus") }
                        span { "Plan new event" }
                    }
                }
                (if has_identity {
                    view! {}
                } else {
                    view! { p(class="help is-warning") {
                        "Sign in first — every event needs a user as its owner."
                    } }
                })
            }
        }
    }
}

fn view_feedback(model: crate::Model) -> View {
    let msg = model.screens.events.feedback.get_clone();
    if msg.is_empty() {
        view! {}
    } else {
        view! { p(class="help is-danger") { (msg) } }
    }
}

// ── Event list (saved on this device) ──

fn view_open_events(model: crate::Model) -> View {
    let sm = model.screens.home;
    let _ = sm.refresh.get();
    let mut ids: Vec<String> = crate::event::list_events().into_iter().collect();
    ids.retain(|id| id != crate::event::DEMO_EVENT_ID);
    ids.sort();
    let recent = crate::event::session_recent_event();
    let mut rows: Vec<View> = vec![view_event_row(
        model,
        crate::event::DEMO_EVENT_ID.to_string(),
        "Khanatime Demo".to_string(),
        None,
        None,
        true,
        crate::event::DEMO_EVENT_ID == recent,
    )];
    for id in ids {
        let e = crate::event::load_event(&id);
        let name = if e.name.is_empty() {
            id.clone()
        } else {
            e.name.clone()
        };
        let hs_tag = if e.is_published() {
            Some(crate::page::home::hs_host_port(
                e.primary_homeserver().unwrap_or_default(),
            ))
        } else {
            None
        };
        let is_recent = id == recent;
        rows.push(view_event_row(
            model,
            id,
            name,
            hs_tag,
            Some(e.status.to_string()),
            false,
            is_recent,
        ));
    }
    let body = if rows.is_empty() {
        view! { p(class="help") { "No events on this device yet." } }
    } else {
        view! { div(class="mt-2") { (rows) } }
    };
    view! {
        div(class="box") {
            h2(class="title is-5") {
                "Saved events"
                span(class="tag is-light is-pulled-right") { "Device" }
            }
            p(class="help") {
                "Open the demo for training, or an event saved on this device."
            }
            (body)
            (view_delete_modal(model))
        }
    }
}

fn view_event_row(
    model: crate::Model,
    id: String,
    name: String,
    hs_tag: Option<String>,
    status: Option<String>,
    is_demo: bool,
    is_recent: bool,
) -> View {
    let sm = model.screens.home;
    let open_id = id.clone();
    let del_id = id.clone();
    let mut tags: Vec<View> = vec![];
    if is_recent {
        tags.push(view! { span(class="tag is-success is-light") { "Recent" } });
    }
    if is_demo {
        tags.push(view! { span(class="tag is-warning is-light") { "Demo" } });
    }
    if let Some(hs) = hs_tag {
        tags.push(view! { span(class="tag is-link is-light") { (hs) } });
    }
    if let Some(s) = status {
        tags.push(view! { span(class="tag is-light") { (s) } });
    }
    view! {
        div(class="field is-grouped is-grouped-multiline mb-2") {
            div(class="control is-expanded") {
                span(class="has-text-weight-semibold mr-2") { (name) }
                (tags)
            }
            div(class="control") {
                button(
                    class="button is-small is-link",
                    on:click=move |_| {
                        if is_demo {
                            crate::update(model, crate::Msg::LoadDemo);
                        } else {
                            let id = open_id.clone();
                            crate::update(model, crate::Msg::OpenSaved(id));
                        }
                    },
                ) { "Open" }
            }
            (if is_demo {
                view! {
                    div(class="control") {
                        button(
                            class="button is-small is-warning",
                            on:click=move |_| crate::update(model, crate::Msg::ResetDemo),
                        ) { "Reset" }
                    }
                }
            } else {
                view! {
                    div(class="control") {
                        button(
                            class="button is-small is-danger is-light",
                            on:click=move |_| {
                                let d = del_id.clone();
                                sm.delete_target.set(Some(d));
                            },
                        ) {
                            span(class="icon is-small") { i(class="fa fa-trash") }
                        }
                    }
                }
            })
        }
    }
}

fn view_delete_modal(model: crate::Model) -> View {
    let sm = model.screens.home;
    let Some(id) = sm.delete_target.get_clone() else {
        return view! {};
    };
    let e = crate::event::load_event(&id);
    let name = if e.name.is_empty() {
        id.clone()
    } else {
        e.name.clone()
    };
    let del_id = id.clone();
    view! {
        div(class="modal is-active") {
            div(class="modal-background")
            div(class="modal-card") {
                header(class="modal-card-head") {
                    p(class="modal-card-title") { "Delete this event?" }
                    button(class="delete", on:click=move |_| sm.delete_target.set(None))
                }
                section(class="modal-card-body") {
                    p { "This removes the saved event:" }
                    p(class="has-text-weight-medium") { (name) }
                    p(class="help") {
                        "Its data is removed from this device only."
                    }
                }
                footer(class="modal-card-foot") {
                    button(
                        class="button is-danger",
                        on:click=move |_| {
                            sm.delete_target.set(None);
                            crate::update(
                                model,
                                crate::Msg::DeleteEvent(del_id.clone()),
                            );
                        },
                    ) { "Delete" }
                    button(class="button", on:click=move |_| sm.delete_target.set(None)) {
                        "Cancel"
                    }
                }
            }
        }
    }
}
