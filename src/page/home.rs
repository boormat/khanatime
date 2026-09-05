use sycamore::prelude::*;

use crate::app::ConnState;
use crate::event::{EventInfo, RunRecord};

// Home / dashboard: sign-in, event picker, quick actions and a live summary of
// event status.  With no event selected it shows just the picker / sign-in bits.

#[derive(Clone, Copy)]
pub struct Model {
    /// Homeserver for the SSO target.
    pub homeserver: Signal<String>,
    pub busy: Signal<bool>,
    /// Pasted join-link URL on the QR page (shared with `Msg::JoinUrl`).
    pub join_url: Signal<String>,
    /// Feedback line for a pasted join link.
    pub join_msg: Signal<String>,
    /// A saved event awaiting a Delete confirmation ("are you sure" modal).
    pub delete_target: Signal<Option<String>>,
    /// Bumped after the local event list changes so the picker re-renders.
    pub refresh: Signal<u8>,
    /// "Organise an event" confirmation modal is open.
    pub show_organise: Signal<bool>,
    /// Burger menu open state.
    pub burger_open: Signal<bool>,
}

pub fn init() -> Model {
    Model {
        homeserver: create_signal("http://localhost:8008".to_string()),
        busy: create_signal(false),
        join_url: create_signal(String::new()),
        join_msg: create_signal(String::new()),
        delete_target: create_signal(None),
        refresh: create_signal(0),
        show_organise: create_signal(false),
        burger_open: create_signal(false),
    }
}

pub fn view(model: crate::Model) -> View {
    view! {
        (move || view_hub(model))
    }
}

/// Home is one fixed layout whether or not an event is open:
/// 1. identity & comms status, 2. current event, 3. create, 4. saved events.
///
/// The run-progress summary lives on the Timing hub instead.
fn view_hub(model: crate::Model) -> View {
    view! {
        div {
            (view_identity_status(model))
            (view_current_event(model))
            (view_saved_events(model))
        }
    }
}

/// Identity + comms status: homeserver status tags up top, then the user id
/// (as a tag) + connection line, with a pending-outbox tag that links to Chat.
/// Not signed in → a sign-in prompt (this is the login area, not a banner).
#[cfg(target_arch = "wasm32")]
fn view_identity_status(model: crate::Model) -> View {
    use crate::page::accounts::ConnStatus;
    let hs_tags: Vec<View> = crate::services::matrix::load_homeservers()
        .iter()
        .map(|hs| {
            let label = hs_host_port(&hs.url);
            let status = model
                .screens
                .accounts
                .hs_status
                .get_clone()
                .get(&hs.url)
                .cloned()
                .unwrap_or(ConnStatus::Unknown);
            view! {
                div(class="control") {
                    span(class="tags has-addons") {
                        span(class="tag is-dark is-small") { (label) }
                        (crate::page::accounts::view_hs_status(status.clone()))
                    }
                }
            }
        })
        .collect();
    let hs_row: View = if hs_tags.is_empty() {
        view! { p(class="help") { "No homeservers saved yet." } }
    } else {
        view! { div(class="field is-grouped is-grouped-multiline mb-2") { (hs_tags) } }
    };

    let identity = model.sync.app_identity.get_clone();
    if identity.is_empty() {
        view! {
            div(class="box") {
                (hs_row)
                h2(class="title is-5") { "Sign in" }
                p(class="help") {
                    "Sign in once so the times you record are attributed to you. Works offline too — joining an event gives you a local account."
                }
                div(class="buttons") {
                    button(
                        class="button is-link",
                        on:click=move |_| {
                            crate::update(model, crate::Msg::Conn(crate::sync::Msg::SsoLoginFor("https://matrix.org".to_string())));
                        },
                    ) {
                        span(class="icon is-small") { i(class="fa fa-id-badge") }
                        span { "Sign in to Matrix.org" }
                    }
                }
            }
        }
    } else {
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
            _ => ("has-text-grey", "Not connected".to_string()),
        };
        // Unsent outbox backlog for the open event — links to Chat to examine.
        let pending_count = model.khana.event.with(|e| {
            if e.is_null() {
                0
            } else {
                crate::log::load_pending(&e.id).len()
            }
        });
        view! {
            div(class="box") {
                (hs_row)
                div(class="level is-mobile") {
                    div(class="level-left") {
                        span(class="tag is-link") { (identity) }
                        span(class=format!("help ml-2 {cls}")) { (text) }
                        (if pending_count > 0 {
                            view! {
                                a(
                                    class="tag is-warning ml-2",
                                    title="Open the message log to see unsent messages",
                                    on:click=move |_| {
                                        crate::update(model, crate::Msg::Show(crate::Screen::Chat));
                                    },
                                ) {
                                    span(class="icon is-small") { i(class="fa fa-clock") }
                                    span { (format!(" {pending_count} pending")) }
                                }
                            }
                        } else {
                            view! {}
                        })
                    }
                }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn view_identity_status(_model: crate::Model) -> View {
    view! {}
}

/// Current event: name + role + status (+ Close), with per-test FIA status
/// tags and, for running tests, the on-course + outstanding car chips beside
/// the tag.  Same box in both states.
fn view_current_event(model: crate::Model) -> View {
    let event = model.khana.event.get_clone();
    if event.is_null() {
        return view! {
            div(class="box") {
                h2(class="title is-5") { "Current event" }
                p(class="help") {
                    "No event open — open one below, or create a new one."
                }
            }
        };
    }
    let name = if event.name.is_empty() {
        "Untitled event".to_string()
    } else {
        event.name.clone()
    };
    let status = event.status.to_string();
    let role_tag = match model.role.get() {
        crate::app::Role::Organiser => "Organiser",
        crate::app::Role::Official => "Official",
    };
    // Per-test progress tags (FIA colours) + on-course/outstanding car chips.
    let runs = model.khana.runs.get_clone();
    let statuses = stage_status(&event, &runs);
    let mut top_tags: Vec<View> = Vec::new();
    let mut running_rows: Vec<View> = Vec::new();
    for s in &statuses {
        let running = !s.cancelled && (s.pct > 0 || !s.active.is_empty());
        let label = if s.name.is_empty() {
            format!("T{}", s.num)
        } else {
            format!("T{}: {}", s.num, s.name)
        };
        if running {
            // On-course chips always show; the pending (cars-left) chips only
            // show when they'd fit beside them — active + pending < 5.
            let show_pending = s.active.len() + s.outstanding.len() < 5;
            let mut chips: Vec<View> = Vec::new();
            for car in &s.active {
                chips.push(car_chip(car.clone(), "is-warning"));
            }
            if show_pending {
                for car in &s.outstanding {
                    chips.push(crate::view::car_tag(car));
                }
            }
            let chips_view: View = chips.into();
            let pct_label = format!("{} · {}%", label, s.pct);
            running_rows.push(view! {
                div(class="is-flex is-align-items-center is-flex-wrap-wrap mt-1", style="gap: 0.5rem;") {
                    span(class="tag is-warning") { (pct_label) }
                    div(class="is-flex is-flex-wrap-wrap", style="gap: 0.25rem;") { (chips_view) }
                }
            });
        } else {
            let cls = if s.cancelled {
                "is-danger"
            } else if s.pct >= 100 {
                "is-info"
            } else {
                "is-white"
            };
            top_tags.push(view! { span(class=format!("tag {cls}")) { (label) } });
        }
    }
    let has_top_tags = !top_tags.is_empty();
    let top_tags_view: View = top_tags.into();
    let running_rows_view: View = running_rows.into();
    view! {
        div(class="box") {
            div(class="level is-mobile") {
                div(class="level-left") {
                    span(class="has-text-weight-semibold") { (name) }
                    span(class="tag is-link is-light ml-2") { (role_tag) }
                    span(class="tag is-light ml-2") { (status) }
                }
                div(class="level-right") {
                    button(
                        class="button is-small is-danger is-outlined",
                        on:click=move |_| crate::update(model, crate::Msg::ClearEvent),
                    ) {
                        span(class="icon is-small") { i(class="fa fa-xmark") }
                        span { "Close" }
                    }
                }
            }
            (if !has_top_tags {
                view! {}
            } else {
                view! { div(class="tags mt-2") { (top_tags_view) } }
            })
            (running_rows_view)
        }
    }
}

/// A small car chip (icon + number) with the given tag colour.
fn car_chip(car: String, cls: &'static str) -> View {
    view! {
        span(class=format!("tag {cls} kt-car-tag")) { i(class="fa fa-car") { " " } (car) }
    }
}

/// Per-test status for the Home block: % of active cars done, cancelled flag,
/// cars on course now, and the still-to-go cars.  Uses the same attempt rule as
/// `stage_progress` (`car_attempts_done`).
struct StageStatus {
    num: u8,
    name: String,
    pct: u8,
    cancelled: bool,
    active: Vec<String>,
    outstanding: Vec<String>,
}

fn stage_status(event: &EventInfo, runs: &[RunRecord]) -> Vec<StageStatus> {
    use crate::event::cmp_car_number;
    let active_cars: Vec<&str> = event
        .entries
        .iter()
        .filter(|e| !e.car.is_empty())
        .map(|e| e.car.as_str())
        .collect();
    event
        .stages
        .iter()
        .map(|st| {
            let cancelled = st.runs_total == 0;
            // Cars on course right now (a pending start, no finish yet).
            let mut active: Vec<String> = crate::event::pending_starts(runs, st.num)
                .iter()
                .map(|r| r.car.clone())
                .collect();
            active.sort_by(|a, b| cmp_car_number(a, b));
            active.dedup();
            let done: Vec<&str> = active_cars
                .iter()
                .copied()
                .filter(|car| {
                    crate::event::car_attempts_done(runs, st.num, car) >= st.runs_total as usize
                })
                .collect();
            let mut outstanding: Vec<String> = active_cars
                .iter()
                .copied()
                .filter(|car| !done.contains(car))
                // A car on course now isn't "pending" too.
                .filter(|car| !active.iter().any(|a| a == car))
                .map(str::to_string)
                .collect();
            outstanding.sort_by(|a, b| cmp_car_number(a, b));
            let pct = if cancelled || active_cars.is_empty() {
                0
            } else {
                (done.len() * 100 / active_cars.len()).min(100) as u8
            };
            StageStatus {
                num: st.num,
                name: st.name.clone(),
                pct,
                cancelled,
                active,
                outstanding,
            }
        })
        .collect()
}

/// Saved events on this device — always visible, the event switcher.
fn view_saved_events(model: crate::Model) -> View {
    let sm = model.screens.home;
    let _ = sm.refresh.get();
    let mut ids: Vec<String> = crate::event::list_events().into_iter().collect();
    ids.sort();
    let mut rows: Vec<View> = Vec::new();
    for id in ids {
        let e = crate::event::load_event(&id);
        let name = if e.name.is_empty() {
            id.clone()
        } else {
            e.name.clone()
        };
        let hs_tag = if e.is_published() {
            Some(hs_host_port(e.primary_homeserver().unwrap_or_default()))
        } else {
            None
        };
        // Unsent outbox count, shown for published events.
        let pending = if e.is_published() {
            crate::log::load_pending(&id).len()
        } else {
            0
        };
        rows.push(view_event_row(
            model,
            id,
            name,
            hs_tag,
            Some(e.status.to_string()),
            pending,
        ));
    }
    let body = if rows.is_empty() {
        view! { p(class="help") { "No events on this device yet." } }
    } else {
        view! { div(class="mt-2") { (rows) } }
    };
    // The Demo button appears only until the demo event has been created (once
    // it exists it shows as a normal saved-event row).  "+ organise" is hidden
    // while an event is open — kept out of sight of average users.
    let demo_missing = crate::log::load_log(crate::event::DEMO_EVENT_ID).is_empty()
        && crate::log::load_pending(crate::event::DEMO_EVENT_ID).is_empty();
    let no_event = model.khana.event.with(|e| e.is_null());
    view! {
        div(class="box") {
            h2(class="title is-5") {
                "Saved events"
                (if demo_missing {
                    view! {
                        button(
                            class="button is-small is-warning is-pulled-right",
                            title="Try the demo event",
                            on:click=move |_| crate::update(model, crate::Msg::LoadDemo),
                        ) {
                            span(class="icon is-small") { i(class="fa fa-flask") }
                            span { "Demo" }
                        }
                    }
                } else {
                    view! {}
                })
                (if no_event {
                    view! {
                        button(
                            class="button is-small is-light is-pulled-right",
                            title="Organise an event",
                            on:click=move |_| sm.show_organise.set(true),
                        ) {
                            span(class="icon is-small") { i(class="fa fa-plus") }
                            span { "organise" }
                        }
                    }
                } else {
                    view! {}
                })
            }
            p(class="help") {
                "Open an event saved on this device."
            }
            (body)
            (view_delete_modal(model))
            (view_organise_modal(model))
        }
    }
}

/// "Organise an event" confirmation modal — a small + button opens this rather
/// than a big call-to-action.  Requires an identity (an event always has a user).
#[cfg(target_arch = "wasm32")]
fn view_organise_modal(model: crate::Model) -> View {
    let sm = model.screens.home;
    if !sm.show_organise.get() {
        return view! {};
    }
    let has_identity = model.sync.app_identity.with(|a| !a.is_empty());
    view! {
        div(class="modal is-active") {
            div(class="modal-background", on:click=move |_| sm.show_organise.set(false))
            div(class="modal-card") {
                header(class="modal-card-head") {
                    p(class="modal-card-title") { "Organise an event" }
                    button(class="delete", on:click=move |_| sm.show_organise.set(false))
                }
                section(class="modal-card-body") {
                    p {
                        "Plan a new event for the timing day. You'll be its owner and a key official — publish it to a homeserver when timing starts."
                    }
                    p(class="help mt-2") {
                        "Starts a local draft with defaults you can edit and save."
                    }
                    (if has_identity {
                        view! {}
                    } else {
                        view! { p(class="help is-warning") {
                            "Sign in first — every event needs a user as its owner."
                        } }
                    })
                }
                footer(class="modal-card-foot is-justify-content-center") {
                    button(
                        class="button is-primary",
                        disabled=!has_identity,
                        on:click=move |_| {
                            sm.show_organise.set(false);
                            crate::update(model, crate::Msg::EventMsg(crate::khana::page::event::Msg::CreateDraft));
                            crate::update(model, crate::Msg::Show(crate::Screen::Event));
                        },
                    ) { "Create" }
                    button(class="button", on:click=move |_| sm.show_organise.set(false)) {
                        "Cancel"
                    }
                }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn view_organise_modal(_model: crate::Model) -> View {
    view! {}
}

fn view_event_row(
    model: crate::Model,
    id: String,
    name: String,
    hs_tag: Option<String>,
    status: Option<String>,
    pending: usize,
) -> View {
    let sm = model.screens.home;
    let open_id = id.clone();
    let del_id = id.clone();
    let mut tags: Vec<View> = vec![];
    if let Some(hs) = hs_tag {
        tags.push(view! { span(class="tag is-link is-light") { (hs) } });
    }
    if let Some(s) = status {
        tags.push(view! { span(class="tag is-light") { (s) } });
    }
    if pending > 0 {
        tags.push(
            view! { span(class="tag is-warning is-light") { (format!("{pending} pending")) } },
        );
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
                        let id = open_id.clone();
                        crate::update(model, crate::Msg::OpenSaved(id));
                    },
                ) { "Open" }
            }
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
/// Homeserver as `host[:port]` for a badge (strips scheme and trailing slash).
pub(crate) fn hs_host_port(hs: &str) -> String {
    let s = hs
        .strip_prefix("https://")
        .or_else(|| hs.strip_prefix("http://"))
        .unwrap_or(hs);
    let end = s.find(['/', '?']).unwrap_or(s.len());
    s[..end].to_string()
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

/// (total entries, active competitors — entries with an assigned car).
fn entry_counts(event: &EventInfo) -> (usize, usize) {
    let total = event.entries.len();
    let active = event.entries.iter().filter(|e| !e.car.is_empty()).count();
    (total, active)
}

/// Per-stage progress for active entries (entries with an assigned car):
/// - completed: has at least one accounted attempt (a finish, or a DNS no-show)
/// - scored_runs: at least `runs_scored` attempts done
/// - total_runs: at least `runs_total` attempts done
fn stage_progress(event: &EventInfo, runs: &[RunRecord]) -> Vec<StageProgress> {
    let active_cars: Vec<&str> = event
        .entries
        .iter()
        .filter(|e| !e.car.is_empty())
        .map(|e| e.car.as_str())
        .collect();

    event
        .stages
        .iter()
        .map(|st| {
            let done = |min: u8| {
                active_cars
                    .iter()
                    .filter(|car| {
                        crate::event::car_attempts_done(runs, st.num, car) >= min as usize
                    })
                    .count()
            };
            StageProgress {
                num: st.num,
                name: st.name.clone(),
                completed: done(1),
                scored_runs: done(st.runs_scored),
                total_runs: done(st.runs_total),
            }
        })
        .collect()
}

/// Run-progress summary for the open event (entry counts + per-stage table).
/// Rendered on the Timing hub's stage list (moved off Home).
pub fn view_status_summary(model: crate::Model) -> View {
    let event = model.khana.event.get_clone();
    if event.is_null() {
        return view! {};
    }
    let runs = model.khana.runs.get_clone();

    let (total, active) = entry_counts(&event);
    let stages = stage_progress(&event, &runs);
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
                "Completed = has a recorded result for the test (time or DNS/DNF). Scored = at least the scored-run count of attempts done. Total = all runs done (finishes + DNS)."
            }
        }
    }
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

        let p = super::stage_progress(&ev, &sample_runs());
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].completed, 2);
        assert_eq!(p[1].completed, 1);
        assert_eq!(p[0].scored_runs, 2); // both active have >= 1 run
        assert_eq!(p[0].total_runs, 1); // only Alice has 2 runs (runs_total=2)
        assert_eq!(p[1].scored_runs, 1); // only Alice ran stage 2
        assert_eq!(p[1].total_runs, 1);
    }

    #[test]
    fn stage_status_pct_and_outstanding() {
        use crate::event::{Stage, TimingStyle};
        let stage = |num: u8, runs_total: u8| Stage {
            num,
            name: format!("Test {num}"),
            runs_total,
            runs_scored: 1,
            timing: TimingStyle::Stopwatch,
        };
        let mut ev = EventInfo {
            name: "Demo".into(),
            stages: vec![stage(1, 1), stage(2, 2), stage(3, 1)],
            ..Default::default()
        };
        for n in ["1", "2", "3", "4"] {
            ev.entries.push(crate::event::Entry::new(n, "R"));
        }
        let finish = |t: u8, car: &str, uid: &str, ts: i64| RunRecord {
            uid: uid.into(),
            r#type: crate::event::RUN_FINISH.into(),
            test: t,
            car: car.into(),
            ts,
            ..Default::default()
        };
        let dns = |t: u8, car: &str, uid: &str, ts: i64| RunRecord {
            uid: uid.into(),
            r#type: crate::event::RUN_START.into(),
            test: t,
            car: car.into(),
            ts,
            status: Some("dns".into()),
            ..Default::default()
        };
        // Stage 1 (runs_total=1): cars 1-3 finished → 75%, outstanding 4.
        // Stage 2 (runs_total=2): car1 two finishes (done), car2 finish + dns
        // (done), car3/4 nothing → 50%, outstanding 3,4.
        // Stage 3: nothing → 0% not started (no outstanding row).
        let runs = vec![
            finish(1, "1", "f11", 1),
            finish(1, "2", "f12", 2),
            finish(1, "3", "f13", 3),
            finish(2, "1", "f21a", 4),
            finish(2, "1", "f21b", 5),
            finish(2, "2", "f22", 6),
            dns(2, "2", "d22", 7),
        ];
        let s = super::stage_status(&ev, &runs);
        assert_eq!(s.len(), 3);
        assert_eq!(s[0].pct, 75);
        assert_eq!(s[0].outstanding, vec!["4"]);
        assert_eq!(s[1].pct, 50);
        assert_eq!(s[1].outstanding, vec!["3", "4"]);
        assert_eq!(s[2].pct, 0);
        // Not started: every car is outstanding in the data; the render only
        // shows the row for running tests (0 < pct < 100).
        assert_eq!(s[2].outstanding, vec!["1", "2", "3", "4"]);
    }

    #[test]
    fn stage_status_cancelled_and_active() {
        use crate::event::{Stage, TimingStyle};
        let stage = |num: u8, runs_total: u8| Stage {
            num,
            name: format!("Test {num}"),
            runs_total,
            runs_scored: 1,
            timing: TimingStyle::Stopwatch,
        };
        let mut ev = EventInfo {
            name: "Demo".into(),
            stages: vec![stage(1, 0), stage(2, 1)],
            ..Default::default()
        };
        for n in ["1", "2"] {
            ev.entries.push(crate::event::Entry::new(n, "R"));
        }
        // Stage 1: runs_total = 0 → cancelled.
        // Stage 2: car 1 has a pending start (on course, no finish) — not an
        // attempt yet, so pct 0, but it appears in `active`.
        let runs = vec![RunRecord {
            uid: "s1".into(),
            r#type: crate::event::RUN_START.into(),
            test: 2,
            car: "1".into(),
            ts: 1,
            ..Default::default()
        }];
        let s = super::stage_status(&ev, &runs);
        assert!(s[0].cancelled);
        assert_eq!(s[0].pct, 0);
        assert!(!s[1].cancelled);
        assert_eq!(s[1].pct, 0);
        assert_eq!(s[1].active, vec!["1"]);
        // On-course car 1 isn't duplicated in outstanding.
        assert_eq!(s[1].outstanding, vec!["2"]);
    }
}
