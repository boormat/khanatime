use std::collections::{HashMap, HashSet};

use sycamore::prelude::*;

use crate::app::ConnState;
use crate::event::{EventInfo, KTime, RunRecord, ScoreData, RUN_FINISH};

// Home / dashboard: sign-in, event picker, quick actions and a live summary of
// event status.  With no event selected it shows just the picker / sign-in bits.

#[derive(Clone, Copy)]
pub struct Model {
    /// Homeserver for the SSO target or the "add custom homeserver" popup.
    pub homeserver: Signal<String>,
    /// Username for the "add custom homeserver" registration.
    pub username: Signal<String>,
    pub busy: Signal<bool>,
    /// The "add a custom homeserver" URL popup is open.
    pub show_add_hs: Signal<bool>,
    /// Pasted join-link URL on the pick-event box.
    pub join_url: Signal<String>,
    /// Feedback line for a pasted join link.
    pub join_msg: Signal<String>,
    /// Homeserver awaiting a Forget confirmation ("are you sure" modal).
    pub forget_target: Signal<Option<String>>,
    /// The Accounts box is collapsed to a one-line summary.
    pub collapsed: Signal<bool>,
    /// A saved event awaiting a Delete confirmation ("are you sure" modal).
    pub delete_target: Signal<Option<String>>,
    /// Bumped after the local event list changes so the picker re-renders.
    pub refresh: Signal<u8>,
    /// Burger menu open state.
    pub burger_open: Signal<bool>,
}

pub fn init() -> Model {
    Model {
        homeserver: create_signal("http://localhost:8008".to_string()),
        username: create_signal(String::new()),
        busy: create_signal(false),
        show_add_hs: create_signal(false),
        join_url: create_signal(String::new()),
        join_msg: create_signal(String::new()),
        forget_target: create_signal(None),
        collapsed: create_signal(false),
        delete_target: create_signal(None),
        refresh: create_signal(0),
        burger_open: create_signal(false),
    }
}

pub fn view(model: crate::Model) -> View {
    view! {
        (move || {
            // No event open: show the picker-style home regardless of whether
            // the device is online/offline or an account is connected.
            if model.khana.event.with(|e| e.is_null()) {
                view_no_event(model)
            } else {
                view_dashboard(model)
            }
        })
    }
}

/// The home screen when no event is open.  Same whether online or offline:
/// accounts, demo + saved events, join-by-link, and phone (parcel) sync.
fn view_no_event(model: crate::Model) -> View {
    view! {
        section(class="hero is-small") {
            div(class="hero-body") {
                h1(class="title") { "Khana Time" }
                p(class="subtitle is-6 has-text-grey") {
                    "Timing for grassroots khanacross."
                }
            }
        }
        (move || view_sso_prompt(model))
        (move || view_welcome_options(model))
        (move || view_sessions(model))
        (move || view_join_by_url(model))
        (move || view_phone_sync(model))
    }
}

/// Welcome hub: the first-run option cards for what you're here to do.
fn view_welcome_options(model: crate::Model) -> View {
    let has_identity = model.sync.app_identity.with(|a| !a.is_empty());
    let identity = model.sync.app_identity.get_clone();
    let identity_line = if identity.is_empty() {
        view! { p(class="help") { "Not signed in — your recordings won't be attributed until you log in." } }
    } else {
        view! { p(class="help has-text-success") { (format!("Signed in as {identity}")) } }
    };
    let status_line = {
        use crate::app::ConnState;
        let conn = model.sync.conn.get_clone();
        let room = model.sync.room.get_clone();
        let (cls, text) = match conn {
            ConnState::LoggedIn(_) if room.is_some() => (
                "has-text-success",
                format!("Online · connected to room {}", room.unwrap()),
            ),
            ConnState::LoggedIn(_) => ("has-text-warning", "Online · no timing room".to_string()),
            ConnState::Connecting => ("has-text-warning", "Connecting…".to_string()),
            ConnState::SsoPending => (
                "has-text-warning",
                "Waiting for the sign-in tab…".to_string(),
            ),
            ConnState::Error(e) => ("has-text-danger", e),
            _ => ("has-text-grey", "Offline — local / parcel mode".to_string()),
        };
        view! { p(class=format!("help {cls}")) { (text) } }
    };
    view! {
        div(class="box") {
            h2(class="title is-5") { "Welcome" }
            div(class="buttons") {
                button(
                    class="button is-link",
                    on:click=move |_| crate::update(model, crate::Msg::LoadDemo),
                ) {
                    span(class="icon is-small") { i(class="fa fa-flask") }
                    span { "Test with the demo event" }
                }
                button(
                    class="button is-primary",
                    disabled=!has_identity,
                    on:click=move |_| crate::update(model, crate::Msg::Show(crate::Screen::Events)),
                ) {
                    span(class="icon is-small") { i(class="fa fa-plus") }
                    span { "Create an event" }
                }
                button(
                    class="button is-light",
                    on:click=move |_| crate::update(model, crate::Msg::Show(crate::Screen::Events)),
                ) {
                    span(class="icon is-small") { i(class="fa fa-binoculars") }
                    span { "Spectate — find a published event" }
                }
                button(
                    class="button is-light",
                    on:click=move |_| crate::update(model, crate::Msg::Show(crate::Screen::Events)),
                ) {
                    span(class="icon is-small") { i(class="fa fa-arrows-rotate") }
                    span { "Switch between events on this device" }
                }
            }
            (identity_line)
            (status_line)
            (if has_identity {
                view! {}
            } else {
                view! { p(class="help is-warning") {
                    "Sign in to create an event — every event needs a user as its owner."
                } }
            })
        }
    }
}

/// First-run identity prompt: no app identity yet, not arriving via a join
/// link, and not dismissed this session — nudge the user to do the matrix SSO
/// so their recordings are attributed.  Dismissible ("Not now").
#[cfg(target_arch = "wasm32")]
fn view_sso_prompt(model: crate::Model) -> View {
    view! {
        (move || {
            let has_identity = model.sync.app_identity.with(|a| !a.is_empty());
            let has_pending = model.sync.pending_join.with(|p| p.is_some());
            if has_identity || has_pending || sso_prompt_dismissed() {
                return view! {};
            }
            view! {
                div(class="notification is-info is-light") {
                    div(class="level is-mobile") {
                        div(class="level-left") {
                            div {
                                p(class="has-text-weight-medium") {
                                    "Identify your recordings"
                                }
                                p(class="help") {
                                    "Sign in to Matrix.org once so the times you record are attributed to you — works offline too."
                                }
                            }
                        }
                        div(class="level-right") {
                            div(class="buttons are-small") {
                                button(
                                    class="button is-link",
                                    on:click=move |_| {
                                        crate::update(model, crate::Msg::Conn(crate::sync::Msg::SsoLoginFor("https://matrix.org".to_string())));
                                    },
                                ) {
                                    span(class="icon is-small") { i(class="fa fa-id-badge") }
                                    span { "Sign in to Matrix.org" }
                                }
                                button(
                                    class="button is-light",
                                    on:click=move |_| {
                                        set_sso_prompt_dismissed();
                                    },
                                ) { "Not now" }
                            }
                        }
                    }
                }
            }
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn view_sso_prompt(_model: crate::Model) -> View {
    view! {}
}

#[cfg(target_arch = "wasm32")]
fn sso_prompt_dismissed() -> bool {
    web_sys::window()
        .and_then(|w| w.session_storage().ok().flatten())
        .and_then(|st| st.get_item("kt_sso_prompt_dismissed").ok().flatten())
        .is_some()
}

#[cfg(target_arch = "wasm32")]
fn set_sso_prompt_dismissed() {
    if let Some(st) = web_sys::window().and_then(|w| w.session_storage().ok().flatten()) {
        let _ = st.set_item("kt_sso_prompt_dismissed", "1");
    }
}

fn view_dashboard(model: crate::Model) -> View {
    view! {
        (move || view_event_header(model))
        (move || view_sessions(model))
        (move || view_status_summary(model))
    }
}

/// Event header: name + switch link + connection tag, all in one line.
fn view_event_header(model: crate::Model) -> View {
    let event = model.khana.event.get_clone();
    if event.is_null() {
        return view! {};
    }
    let name = if event.name.is_empty() {
        "Untitled event".to_string()
    } else {
        event.name.clone()
    };
    view! {
        div(class="level is-mobile mb-2") {
            div(class="level-left") {
                span(class="has-text-weight-semibold") { (name) }
                a(class="has-text-link is-size-7 ml-2", on:click=move |_| {
                    crate::update(model, crate::Msg::Show(crate::Screen::Events));
                }) {
                    span(class="icon is-small") { i(class="fa fa-pen") }
                }
            }
            div(class="level-right") {
                (move || view_status_tag(model))
            }
        }
    }
}

/// Connection status tag — clickable, goes to Accounts page.
#[cfg(target_arch = "wasm32")]
fn view_status_tag(model: crate::Model) -> View {
    let logged_in = matches!(model.sync.conn.get_clone(), ConnState::LoggedIn(_));
    let sess = logged_in
        .then(crate::services::matrix::active_hs)
        .flatten()
        .and_then(|hs| crate::services::matrix::load_session_for(&hs));
    let tag = if let Some(sess) = sess {
        if crate::services::matrix::is_matrix_org(&sess.homeserver) {
            view! { span(class="tag is-success is-light is-small") {
                span(class="icon is-small") { i(class="fa fa-check") }
                span { (sess.user_id) }
            } }
        } else {
            view! { span(class="tag is-link is-light is-small") { (hs_host_port(&sess.homeserver)) } }
        }
    } else {
        view! { span(class="tag is-grey is-light is-small") { "Offline" } }
    };
    view! {
        a(on:click=move |_| crate::update(model, crate::Msg::Show(crate::Screen::Accounts))) {
            (tag)
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn view_status_tag(_model: crate::Model) -> View {
    view! {}
}

/// Simple account status bar — just the clickable tag + forget modal.
#[cfg(target_arch = "wasm32")]
fn view_sessions(model: crate::Model) -> View {
    view! {
        (move || {
            let sm = model.screens.home;
            let _ = sm.refresh.get();
            view! {
                div(class="box") {
                    (view_account_footer(model))
                    (view_forget_modal(model))
                }
            }
        })
    }
}

/// Homeserver as `host[:port]` for a badge (strips scheme and trailing slash).
pub(crate) fn hs_host_port(hs: &str) -> String {
    let s = hs
        .strip_prefix("https://")
        .or_else(|| hs.strip_prefix("http://"))
        .unwrap_or(hs);
    let end = s.find(['/', '?']).unwrap_or(s.len());
    s[..end].to_string()
}

#[cfg(not(target_arch = "wasm32"))]
fn view_sessions(_model: crate::Model) -> View {
    view! {}
}

pub fn view_comms(model: crate::Model) -> View {
    let conn = model.sync.conn.get_clone();
    let room = model.sync.room.get_clone();
    let (color, text) = match conn {
        ConnState::LoggedIn(_) if room.is_some() => {
            ("is-success", format!("Connected · room {}", room.unwrap()))
        }
        ConnState::LoggedIn(_) => ("is-warning", "Logged in · no timing room".to_string()),
        ConnState::Connecting => ("is-warning", "Connecting...".to_string()),
        ConnState::SsoPending => ("is-warning", "Waiting for the sign-in tab…".to_string()),
        ConnState::Error(e) => ("is-danger", e),
        _ => ("is-danger", "Not connected".to_string()),
    };
    view! {
        div(class="box") {
            h2(class="title is-6") { "Comms" }
            span(class=format!("tag {}", color)) { (text) }
        }
    }
}

struct StageProgress {
    num: u8,
    name: String,
    completed: usize,
    scored_runs: usize,
    total_runs: usize,
}

/// (total, active competitors, withdrawn, draft, reserve) entry counts.
fn entry_counts(event: &EventInfo) -> (usize, usize) {
    let total = event.entries.len();
    (total, total)
}

/// Per-stage progress for active entries:
/// - completed: has a real (non-withdrawn) recorded time
/// - scored_runs: finished at least `runs_scored` runs
/// - total_runs: finished all `runs_total` runs
fn stage_progress(
    event: &EventInfo,
    scores: &[ScoreData],
    runs: &[RunRecord],
) -> Vec<StageProgress> {
    let active_cars: HashSet<&str> = event.entries.iter().map(|e| e.car.as_str()).collect();

    let mut completed: HashMap<u8, usize> = HashMap::new();
    for s in scores {
        if matches!(s.time, KTime::Time(_)) && active_cars.contains(s.car.as_str()) {
            *completed.entry(s.stage).or_insert(0) += 1;
        }
    }

    // Distinct finished runs per (stage, car).
    let mut runs_done: HashMap<(u8, String), HashSet<String>> = HashMap::new();
    for r in runs {
        if r.r#type != RUN_FINISH || r.voided {
            continue;
        }
        runs_done
            .entry((r.test, r.car.clone()))
            .or_default()
            .insert(r.uid.clone());
    }

    event
        .stages
        .iter()
        .map(|st| {
            let done = |min: u8| {
                active_cars
                    .iter()
                    .filter(|car| {
                        runs_done
                            .get(&(st.num, (*car).to_string()))
                            .map_or(0, HashSet::len)
                            >= min as usize
                    })
                    .count()
            };
            StageProgress {
                num: st.num,
                name: st.name.clone(),
                completed: completed.get(&st.num).copied().unwrap_or(0),
                scored_runs: done(st.runs_scored),
                total_runs: done(st.runs_total),
            }
        })
        .collect()
}

fn view_status_summary(model: crate::Model) -> View {
    let event = model.khana.event.get_clone();
    if event.is_null() {
        return view! {};
    }
    let scores = model.khana.scores.get_clone();
    let runs = model.khana.runs.get_clone();

    let (total, active) = entry_counts(&event);
    let stages = stage_progress(&event, &scores, &runs);
    let unassigned = event.entries.iter().filter(|e| e.car.is_empty()).count();
    // Pre-compute owned shared-car info (groups borrows event.entries which
    // goes out of scope before the view! macro, which needs 'static data).
    let shared_box: View = {
        let groups = crate::event::shared_groups(&event.entries);
        if groups.is_empty() {
            view! {}
        } else {
            let lines: Vec<View> = groups
                .iter()
                .map(|(name, members)| {
                    // Clone before view! to avoid borrowing groups into the
                    // view! closure (which needs 'static data).
                    let name = name.clone();
                    let who = members
                        .iter()
                        .map(|e| {
                            let car = if e.car.is_empty() {
                                "?"
                            } else {
                                e.car.as_str()
                            };
                            format!("{car} {name2}", name2 = e.name)
                        })
                        .collect::<Vec<_>>()
                        .join(" · ");
                    view! {
                        div(class="level") {
                            div(class="level-left") {
                                span(class="tag is-warning") {
                                    i(class="fa fa-users")
                                    (name)
                                }
                                span(class="ml-2") { (who) }
                            }
                        }
                    }
                })
                .collect();
            view! {
                div(class="mt-2") {
                    h3(class="title is-6") { "Shared cars" }
                    (lines)
                }
            }
        }
    };

    let mut rows: Vec<View> = vec![];
    for s in &stages {
        let stage_name = if s.name.is_empty() {
            format!("Test {}", s.num)
        } else {
            s.name.clone()
        };
        let pct = (s.completed * 100 + active / 2) / active.max(1);
        let completed_cell = format!("{} / {} ({pct}%)", s.completed, active);
        let min_cell = s.scored_runs.to_string();
        let all_cell = s.total_runs.to_string();
        rows.push(view! {
            tr {
                td { (stage_name) }
                td { (completed_cell) }
                td { (min_cell) }
                td { (all_cell) }
            }
        });
    }

    view! {
        div(class="box") {
            h2(class="title is-6") { "Event status" }
            div(class="field is-grouped is-grouped-multiline") {
                div(class="control") { span(class="tags has-addons") {
                    span(class="tag") { "Entries" }
                    span(class="tag is-link") { (total.to_string()) }
                } }
                div(class="control") { span(class="tags has-addons") {
                    span(class="tag") { "Competitors" }
                    span(class="tag is-success") { (active.to_string()) }
                } }
                            (if unassigned > 0 {
                    view! { div(class="control") { span(class="tags has-addons") {
                        span(class="tag") { "Awaiting #" }
                        span(class="tag is-danger") { (unassigned.to_string()) }
                    } } }
                } else {
                    view! {}
                })
            }
            (shared_box)
            table(class="table is-fullwidth is-striped") {
                thead {
                    tr {
                        th { "Test" }
                        th { "Completed (needs it)" }
                        th { "Scored runs" }
                        th { "Total runs" }
                    }
                }
                tbody { (rows) }
            }
            p(class="help") {
                "Completed = recorded a time for the test. Scored = finished at least the scored-run count. Total = finished all runs."
            }
        }
    }
}

/// Footer of the Accounts box, shown identically whether logged in or not:
/// the relevant login entry (a pending join takes priority, else Matrix.org
/// SSO while that homeserver isn't stored) plus an always-visible
/// "Add custom homeserver" button that opens a URL-only popup.
#[cfg(target_arch = "wasm32")]
fn view_account_footer(model: crate::Model) -> View {
    let sm = model.screens.home;
    let pending = model.sync.pending_join.get_clone();
    let pending_view = if let Some(inv) = &pending {
        let name = inv.event.clone();
        view! {
            div(class="notification is-info is-light") {
                p { "Log in to join " (name) }
                p(class="help") {
                    "You'll be taken straight to the event once signed in."
                }
            }
        }
    } else {
        view! {}
    };
    let has_matrix_org = crate::services::matrix::has_matrix_org_session();
    let primary = if let Some(inv) = &pending {
        let join_hs = inv.homeserver.clone();
        view! {
            div(class="control") {
                button(
                    class="button is-link",
                    disabled=sm.busy.get(),
                    on:click=move |_| {
                        sm.homeserver.set(join_hs.clone());
                        crate::update(model, crate::Msg::Conn(crate::sync::Msg::SsoLogin));
                    },
                ) {
                    span(class="icon is-small") { i(class="fa fa-id-badge") }
                    span { "Sign in to join" }
                }
            }
        }
    } else if !has_matrix_org {
        view! {
            div(class="control") {
                button(
                    class="button is-link",
                    disabled=sm.busy.get(),
                    on:click=move |_| {
                        sm.homeserver.set("https://matrix.org".to_string());
                        crate::update(model, crate::Msg::Conn(crate::sync::Msg::SsoLogin));
                    },
                ) {
                    span(class="icon is-small") { i(class="fa fa-user-plus") }
                    span { "Login to Matrix.org" }
                }
            }
        }
    } else {
        view! {}
    };
    view! {
        (pending_view)
        div(class="field is-grouped") {
            (primary)
        }
    }
}

/// "Are you sure?" modal before forgetting a stored session.
#[cfg(target_arch = "wasm32")]
fn view_forget_modal(model: crate::Model) -> View {
    let sm = model.screens.home;
    let Some(hs) = sm.forget_target.get_clone() else {
        return view! {};
    };
    let hs_display = hs.clone();
    let hs_confirm = hs.clone();
    view! {
        div(class="modal is-active") {
            div(class="modal-background")
            div(class="modal-card") {
                header(class="modal-card-head") {
                    p(class="modal-card-title") { "Forget this account?" }
                    button(class="delete", on:click=move |_| sm.forget_target.set(None))
                }
                section(class="modal-card-body") {
                    p { "Remove the stored session for:" }
                    p(class="has-text-weight-medium") { (hs_display) }
                    p(class="help") {
                        "Forgetting the active account also signs it out server-side."
                    }
                }
                footer(class="modal-card-foot") {
                    button(
                        class="button is-danger",
                        on:click=move |_| {
                            sm.forget_target.set(None);
                            crate::update(
                                model,
                                crate::Msg::Conn(crate::sync::Msg::Forget(hs_confirm.clone())),
                            );
                        },
                    ) { "Forget" }
                    button(class="button", on:click=move |_| sm.forget_target.set(None)) {
                        "Cancel"
                    }
                }
            }
        }
    }
}

/// Username + URL popup for adding a custom homeserver.
#[cfg(target_arch = "wasm32")]
#[allow(dead_code)] // kept for potential future use — "Add homeserver" button removed from homepage
fn view_add_hs_modal(model: crate::Model) -> View {
    let sm = model.screens.home;
    view! {
        div(class="modal is-active") {
            div(class="modal-background")
            div(class="modal-card") {
                header(class="modal-card-head") {
                    p(class="modal-card-title") { "Add custom homeserver" }
                    button(class="delete", on:click=move |_| sm.show_add_hs.set(false))
                }
                section(class="modal-card-body") {
                    div(class="field") {
                        label(class="label") { "Username" }
                        div(class="control") {
                            input(
                                class="input",
                                placeholder="e.g. alice",
                                bind:value=sm.username,
                            )
                        }
                        p(class="help") {
                            "Pick a username for this homeserver. If it's already taken, you'll be told."
                        }
                    }
                    div(class="field") {
                        label(class="label") { "Homeserver URL" }
                        div(class="control") {
                            input(
                                class="input",
                                placeholder="http://localhost:8008",
                                bind:value=sm.homeserver,
                            )
                        }
                        p(class="help") {
                            "SSO when the server advertises it, otherwise a fresh account is registered."
                        }
                    }
                }
                footer(class="modal-card-foot") {
                    button(
                        class="button is-link",
                        disabled=sm.busy.get() || sm.username.get_clone().trim().is_empty(),
                        on:click=move |_| {
                            let hs = sm.homeserver.get_clone();
                            let username = sm.username.get_clone();
                            sm.show_add_hs.set(false);
                            crate::update(model, crate::Msg::Conn(crate::sync::Msg::AddHomeserver { hs, username }));
                        },
                    ) {
                        "Add"
                    }
                    button(class="button", on:click=move |_| sm.show_add_hs.set(false)) {
                        "Cancel"
                    }
                }
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[allow(dead_code)]
fn status_html(state: ConnState) -> View {
    match state {
        ConnState::Idle => view! { p(class="help") { "Not connected." } },
        ConnState::Connecting => view! { p(class="help") { "Connecting..." } },
        ConnState::SsoPending => view! { p(class="help") { "Waiting for the sign-in tab…" } },
        ConnState::LoggedIn(_) => view! { p(class="help is-success") { "Logged in." } },
        ConnState::Error(e) => view! { p(class="help is-danger") { (e) } },
    }
}

/// Import an event from another phone as a QR parcel (no network).
fn view_phone_sync(model: crate::Model) -> View {
    view! {
        div(class="box") {
            h2(class="title is-5") {
                "Phone sync"
                span(class="tag is-light is-pulled-right") { "QR parcel" }
            }
            p(class="help") {
                "Import an event carried from another phone by scanning its QR parcel — no network needed."
            }
            div(class="field") {
                div(class="control") {
                    button(
                        class="button is-warning",
                        on:click=move |_| crate::update(model, crate::Msg::ScanStart),
                    ) {
                        span(class="icon") { i(class="fa fa-camera") }
                        span { "Phone sync (QR)" }
                    }
                }
            }
        }
    }
}

/// Paste a join link to go straight to an event, read it from the clipboard,
/// or scan its invite QR code.
fn view_join_by_url(model: crate::Model) -> View {
    let sm = model.screens.home;
    view! {
        div(class="box") {
            h2(class="title is-5") {
                "Join an event"
                span(class="tag is-light is-pulled-right") { "Invite link" }
            }
            p(class="help") {
                "Paste an invite link to go straight to an event, or scan its QR code."
            }
            div(class="field has-addons") {
                div(class="control is-expanded") {
                    input(
                        class="input",
                        placeholder="Paste an invite link…",
                        bind:value=sm.join_url,
                        on:keydown=move |ev: web_sys::KeyboardEvent| {
                            if ev.key_code() == 13 {
                                crate::update(model, crate::Msg::JoinUrl);
                            }
                        },
                    )
                }
                div(class="control") {
                    button(
                        class="button is-link",
                        disabled=sm.busy.get(),
                        on:click=move |_| crate::update(model, crate::Msg::JoinUrl),
                    ) {
                        "Join"
                    }
                }
            }
            div(class="field is-grouped") {
                div(class="control") {
                    button(
                        class="button is-small is-light",
                        disabled=sm.busy.get(),
                        on:click=move |_| copy_from_clipboard(model),
                    ) {
                        span(class="icon is-small") { i(class="fa fa-clipboard") }
                        span { "Paste from clipboard" }
                    }
                }
                div(class="control") {
                    button(
                        class="button is-small is-light",
                        disabled=sm.busy.get(),
                        on:click=move |_| crate::update(model, crate::Msg::ScanStart),
                    ) {
                        span(class="icon is-small") { i(class="fa fa-qrcode") }
                        span { "Scan invite QR" }
                    }
                }
            }
            (move || {
                let msg = sm.join_msg.get_clone();
                if msg.is_empty() {
                    view! {}
                } else {
                    view! { p(class="help is-danger") { (msg) } }
                }
            })
        }
    }
}

/// Read the clipboard into the join-link field (handy on phones).
#[cfg(target_arch = "wasm32")]
fn copy_from_clipboard(model: crate::Model) {
    let sm = model.screens.home;
    let Some(clip) = web_sys::window().map(|w| w.navigator().clipboard()) else {
        sm.join_msg
            .set("Clipboard is unavailable in this browser.".into());
        return;
    };
    let promise = clip.read_text();
    wasm_bindgen_futures::spawn_local(async move {
        match wasm_bindgen_futures::JsFuture::from(promise).await {
            Ok(v) => {
                let text = v.as_string().unwrap_or_default();
                if text.is_empty() {
                    sm.join_msg.set("Clipboard is empty.".into());
                } else {
                    sm.join_url.set(text);
                    sm.join_msg.set(String::new());
                }
            }
            Err(_) => sm
                .join_msg
                .set("Couldn't read the clipboard — paste manually.".into()),
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn copy_from_clipboard(model: crate::Model) {
    model
        .screens
        .home
        .join_msg
        .set("Clipboard is not available.".into());
}

#[cfg(test)]
mod tests {
    use crate::event::{EventInfo, KTime, RunRecord, ScoreData, RUN_FINISH};

    fn sample_scores() -> Vec<ScoreData> {
        serde_json::from_str(
            r#"[{"stage":1,"car":"1","time":{"Time":{"time_ds":450,"flags":0,"garage":false}}},
                 {"stage":2,"car":"1","time":{"Time":{"time_ds":470,"flags":0,"garage":false}}},
                 {"stage":1,"car":"2","time":{"Time":{"time_ds":500,"flags":0,"garage":false}}}]"#,
        )
        .unwrap()
    }

    fn sample_runs() -> Vec<RunRecord> {
        serde_json::from_str(
            r#"[{"uid":"r1","type":"finish","test":1,"car":"1","run":1,"ts":1},
                 {"uid":"r2","type":"finish","test":1,"car":"1","run":2,"ts":2},
                 {"uid":"r3","type":"finish","test":2,"car":"1","run":1,"ts":3},
                 {"uid":"r4","type":"finish","test":2,"car":"1","run":2,"ts":4},
                 {"uid":"r5","type":"finish","test":1,"car":"2","run":1,"ts":5}]"#,
        )
        .unwrap()
    }

    #[test]
    fn scores_and_runs_deserialize() {
        let s = sample_scores();
        assert_eq!(s.len(), 3);
        assert!(matches!(s[0].time, KTime::Time(_)));
        assert!(matches!(sample_runs()[0].r#type, _));
        assert_eq!(sample_runs()[0].r#type, RUN_FINISH);
    }

    #[test]
    fn stage_progress_counts_completed_and_runs() {
        use crate::event::{Stage, TimingStyle};
        let mut ev = EventInfo {
            name: "Demo Event".into(),
            ..Default::default()
        };
        let stage = |num: u8| Stage {
            num,
            name: format!("Test {num}"),
            runs_total: 2,
            runs_scored: 1,
            timing: TimingStyle::Stopwatch,
        };
        ev.stages = vec![stage(1), stage(2)];
        let e1 = crate::event::Entry::new("1", "Alice");
        let e2 = crate::event::Entry::new("2", "Bob");
        let e3 = crate::event::Entry::new("3", "Carol");
        let e4 = crate::event::Entry::new("4", "Dan");
        ev.entries = vec![e1, e2, e3, e4];

        let counts = super::entry_counts(&ev);
        assert_eq!(counts, (4, 4));

        let p = super::stage_progress(&ev, &sample_scores(), &sample_runs());
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].completed, 2);
        assert_eq!(p[1].completed, 1);
        assert_eq!(p[0].scored_runs, 2); // both active have >= 1 run
        assert_eq!(p[0].total_runs, 1); // only Alice has 2 runs (runs_total=2)
        assert_eq!(p[1].scored_runs, 1); // only Alice ran stage 2
        assert_eq!(p[1].total_runs, 1);
    }
}
