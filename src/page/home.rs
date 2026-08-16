use std::collections::{HashMap, HashSet};

use sycamore::prelude::*;

use crate::app::ConnState;
use crate::event::{
    Entry, EntryStatus, EventInfo, KTime, RunRecord, ScoreData, ROLE_COMPETITOR, RUN_FINISH,
};

// Home / dashboard: sign-in, event picker, quick actions and a live summary of
// event status.  With no event selected it shows just the picker / sign-in bits.

#[derive(Clone, Copy)]
pub struct Model {
    /// Homeserver for the SSO target or the "add custom homeserver" popup.
    pub homeserver: Signal<String>,
    pub busy: Signal<bool>,
    /// The "add a custom homeserver" URL popup is open.
    pub show_add_hs: Signal<bool>,
}

pub fn init() -> Model {
    Model {
        homeserver: create_signal("http://localhost:8008".to_string()),
        busy: create_signal(false),
        show_add_hs: create_signal(false),
    }
}

pub fn view(model: crate::Model) -> View {
    view! {
        (move || {
            if matches!(model.app.conn.get_clone(), ConnState::LoggedIn(_)) {
                view_dashboard(model)
            } else {
                view! {
                    section(class="hero is-small") {
                        div(class="hero-body") {
                            h1(class="title") { "Khana Time Tracker" }
                            p(class="subtitle") {
                                "Log in to your Matrix account, then open an event to watch live results."
                            }
                        }
                    }
                    (view_sessions(model))
                    (view_pick_events(model))
                }
            }
        })
    }
}

fn view_dashboard(model: crate::Model) -> View {
    view! {
        section(class="hero is-small") {
            div(class="hero-body") {
                h1(class="title") { "Khana Time Tracker" }
            }
        }
        (move || view_event_card(model))
        (move || view_sessions(model))
        (move || view_actions(model))
        (move || view_comms(model))
        (move || view_status_summary(model))
    }
}

fn view_event_card(model: crate::Model) -> View {
    let (id, name, status) = model
        .app
        .event
        .with(|e| (e.id.clone(), e.name.clone(), e.status.to_string()));
    let has_event = !id.is_empty();
    let title = if has_event {
        name.clone()
    } else {
        "No event selected".to_string()
    };
    view! {
        div(class="box") {
            div(class="level") {
                div(class="level-left") {
                    div(class="level-item") {
                        h2(class="title is-5") {
                            (title)
                            (if has_event {
                                view! { span(class="tag is-light is-pulled-right") { (status) } }
                            } else {
                                view! {}
                            })
                        }
                    }
                }
                div(class="level-right") {
                    div(class="level-item") {
                        button(
                            class="button is-small is-link is-outlined",
                            on:click=move |_| crate::update(model, crate::Msg::Show(crate::Screen::Events)),
                        ) {
                            "Change event"
                        }
                        button(
                            class="button is-small is-link",
                            on:click=move |_| crate::update(model, crate::Msg::Show(crate::Screen::Event)),
                        ) {
                            "Event admin"
                        }
                    }
                }
            }
        }
    }
}

/// Accounts block: every stored session (homeserver + user), the active one
/// marked and offering Logout, and logged-out ones offering one-tap Re-login
/// or Forget.  The login entry point lives here too — one-click Matrix.org SSO
/// or a custom-homeserver form that stays hidden until needed.
#[cfg(target_arch = "wasm32")]
fn view_sessions(model: crate::Model) -> View {
    view! {
        (move || {
            let logged_in = matches!(model.app.conn.get_clone(), ConnState::LoggedIn(_));
            let sessions = crate::services::matrix::load_sessions();
            let rows: Vec<View> = sessions
                .iter()
                .map(|s| {
                    let is_active = logged_in
                        && crate::services::matrix::active_hs().as_deref()
                            == Some(s.homeserver.as_str());
                    let hs = s.homeserver.clone();
                    let user = s.user_id.clone();
                    let badge = match &s.kind {
                        crate::services::matrix::StoredAuth::OAuth { .. } => "SSO",
                        crate::services::matrix::StoredAuth::Matrix { .. } => "Password",
                    };
                    let controls = if is_active {
                        view! {
                            span(class="tag is-success is-light") { "active" }
                            button(
                                class="button is-small is-light",
                                on:click=move |_| {
                                    crate::update(
                                        model,
                                        crate::Msg::Conn(crate::sync::Msg::Logout),
                                    )
                                },
                            ) {
                                "Logout"
                            }
                        }
                    } else {
                        let relogin_hs = hs.clone();
                        let forget_hs = hs.clone();
                        view! {
                            button(
                                class="button is-small is-link",
                                on:click=move |_| {
                                    crate::update(
                                        model,
                                        crate::Msg::Conn(crate::sync::Msg::Relogin(relogin_hs.clone())),
                                    )
                                },
                            ) {
                                "Re-login"
                            }
                            button(
                                class="button is-small is-danger is-outlined",
                                on:click=move |_| {
                                    crate::update(
                                        model,
                                        crate::Msg::Conn(crate::sync::Msg::Forget(forget_hs.clone())),
                                    )
                                },
                            ) {
                                "Forget"
                            }
                        }
                    };
                    view! {
                        div(class="level") {
                            div(class="level-left") {
                                div(class="level-item") {
                                    span(class="icon has-text-success") { i(class="fa fa-user-circle") }
                                    span(class="has-text-weight-medium") { (user) }
                                    span(class="tag is-light") { (badge) }
                                    span(class="has-text-grey is-size-7") { (hs) }
                                }
                            }
                            div(class="level-right") {
                                div(class="level-item") { (controls) }
                            }
                        }
                    }
                })
                .collect();
            let login = if logged_in {
                view! {}
            } else {
                view_login(model)
            };
            view! {
                div(class="box") {
                    h2(class="title is-5") {
                        "Accounts"
                        span(class="tag is-light is-pulled-right") { "Matrix" }
                    }
                    (rows)
                    (login)
                }
            }
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn view_sessions(_model: crate::Model) -> View {
    view! {}
}

fn view_actions(model: crate::Model) -> View {
    let has_event = !model.app.event.with(|e| e.is_null());
    let role = crate::event::local_role();
    let official = role != ROLE_COMPETITOR;
    view! {
        div(class="box") {
            div(class="field is-grouped") {
                (if has_event && official {
                    view! {
                        div(class="control") {
                            button(
                                class="button is-primary",
                                on:click=move |_| crate::update(model, crate::Msg::Show(crate::Screen::Start)),
                            ) {
                                span(class="icon") { i(class="fa fa-flag") }
                                span { "Timing mode" }
                            }
                        }
                    }
                } else {
                    view! {}
                })
                (if has_event {
                    view! {
                        div(class="control") {
                            button(
                                class="button is-link",
                                on:click=move |_| crate::update(model, crate::Msg::Show(crate::Screen::Results)),
                            ) {
                                span(class="icon") { i(class="fa fa-trophy") }
                                span { "Results" }
                            }
                        }
                    }
                } else {
                    view! {}
                })
            }
        }
    }
}

fn view_comms(model: crate::Model) -> View {
    let conn = model.app.conn.get_clone();
    let room = model.app.room.get_clone();
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
    min_runs: usize,
    all_runs: usize,
}

/// (total, active competitors, withdrawn, draft, reserve) entry counts.
fn entry_counts(event: &EventInfo) -> (usize, usize, usize, usize, usize) {
    let mut total = 0;
    let mut active = 0;
    let mut withdrawn = 0;
    let mut draft = 0;
    let mut reserve = 0;
    for e in &event.entries {
        total += 1;
        match e.status {
            EntryStatus::Withdrawn => withdrawn += 1,
            EntryStatus::Draft => draft += 1,
            EntryStatus::Reserve => reserve += 1,
            _ => active += 1,
        }
    }
    (total, active, withdrawn, draft, reserve)
}

fn is_active(e: &Entry) -> bool {
    !matches!(
        e.status,
        EntryStatus::Withdrawn | EntryStatus::Draft | EntryStatus::Reserve
    )
}

/// Per-stage progress for active entries:
/// - completed: has a real (non-withdrawn) recorded time
/// - min_runs: finished at least `best_x` runs (the X of best-X-of-Y)
/// - all_runs: finished all `repeats` runs (the Y)
fn stage_progress(
    event: &EventInfo,
    scores: &[ScoreData],
    runs: &[RunRecord],
) -> Vec<StageProgress> {
    let active_cars: HashSet<&str> = event
        .entries
        .iter()
        .filter(|e| is_active(e))
        .map(|e| e.car.as_str())
        .collect();

    let mut completed: HashMap<u8, usize> = HashMap::new();
    for s in scores {
        if matches!(s.time, KTime::Time(_)) && active_cars.contains(s.car.as_str()) {
            *completed.entry(s.stage).or_insert(0) += 1;
        }
    }

    // Distinct finished runs per (stage, car).
    let mut runs_done: HashMap<(u8, String), HashSet<u8>> = HashMap::new();
    for r in runs {
        if r.r#type != RUN_FINISH || r.voided {
            continue;
        }
        runs_done
            .entry((r.test, r.car.clone()))
            .or_default()
            .insert(r.run);
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
                min_runs: done(st.best_x),
                all_runs: done(st.repeats),
            }
        })
        .collect()
}

fn view_status_summary(model: crate::Model) -> View {
    let event = model.app.event.get_clone();
    if event.is_null() {
        return view! {};
    }
    let scores = model.app.scores.get_clone();
    let runs = model.app.runs.get_clone();

    let (total, active, withdrawn, draft, reserve) = entry_counts(&event);
    let stages = stage_progress(&event, &scores, &runs);
    let unassigned = event
        .entries
        .iter()
        .filter(|e| {
            crate::event::is_active_entry(e)
                && e.car.is_empty()
                && e.status != crate::event::EntryStatus::Reserve
        })
        .count();
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
                    p(class="help") { "Drivers sharing a car — run them early or hurry them up." }
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
        let min_cell = s.min_runs.to_string();
        let all_cell = s.all_runs.to_string();
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
                div(class="control") { span(class="tags has-addons") {
                    span(class="tag") { "Withdrawn" }
                    span(class="tag is-danger") { (withdrawn.to_string()) }
                } }
                (if draft > 0 {
                    view! { div(class="control") { span(class="tags has-addons") {
                        span(class="tag") { "Draft" }
                        span(class="tag is-light") { (draft.to_string()) }
                    } } }
                } else {
                    view! {}
                })
                (if reserve > 0 {
                    view! { div(class="control") { span(class="tags has-addons") {
                        span(class="tag") { "Reserve" }
                        span(class="tag is-warning") { (reserve.to_string()) }
                    } } }
                } else {
                    view! {}
                })
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
                        th { "Done min runs (X)" }
                        th { "Done all runs (Y)" }
                    }
                }
                tbody { (rows) }
            }
            p(class="help") {
                "Completed = recorded a time for the test. X = best X of Y runs, Y = runs per test."
            }
        }
    }
}

/// Login entry point inside the Accounts box: one-click Matrix.org SSO (only
/// when matrix.org isn't already a stored session) plus an always-visible
/// "Add custom homeserver" button that opens a URL-only popup.
#[cfg(target_arch = "wasm32")]
fn view_login(model: crate::Model) -> View {
    let sm = model.screens.home;
    let pending = model.app.pending_join.get_clone();
    let pending_hs = pending.as_ref().map(|i| i.homeserver.clone());
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
    let has_matrix_org = crate::services::matrix::load_sessions()
        .iter()
        .any(|s| s.homeserver == "https://matrix.org");
    let primary = if let Some(hs) = &pending_hs {
        let join_hs = hs.clone();
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
                    span(class="icon is-small") { i(class="fa fa-id-badge") }
                    span { "Login with Matrix.org" }
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
            div(class="control") {
                button(
                    class="button is-light",
                    disabled=sm.busy.get(),
                    on:click=move |_| {
                        sm.homeserver.set("http://localhost:8008".to_string());
                        sm.show_add_hs.set(true);
                    },
                ) {
                    span(class="icon is-small") { i(class="fa fa-server") }
                    span { "Add custom homeserver" }
                }
            }
        }
        (move || {
            let state = model.app.conn.get_clone();
            view! {
                div {
                    (if sm.show_add_hs.get() {
                        view_add_hs_modal(model)
                    } else {
                        view! {}
                    })
                    div { (status_html(state.clone())) }
                }
            }
        })
        p(class="help") {
            "matrix.org accounts are passwordless (SSO). The localhost dev server registers a new account for you."
        }
    }
}

/// URL-only popup for adding a custom homeserver: no username or password.
#[cfg(target_arch = "wasm32")]
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
                        disabled=sm.busy.get(),
                        on:click=move |_| {
                            let hs = sm.homeserver.get_clone();
                            sm.show_add_hs.set(false);
                            crate::update(model, crate::Msg::Conn(crate::sync::Msg::AddHomeserver(hs)));
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
fn status_html(state: ConnState) -> View {
    match state {
        ConnState::Idle => view! { p(class="help") { "Not connected." } },
        ConnState::Connecting => view! { p(class="help") { "Connecting..." } },
        ConnState::SsoPending => view! { p(class="help") { "Waiting for the sign-in tab…" } },
        ConnState::LoggedIn(_) => view! { p(class="help is-success") { "Logged in." } },
        ConnState::Error(e) => view! { p(class="help is-danger") { (e) } },
    }
}

/// Picker entry point: opens the Events screen (demo / published / new / saved).
fn view_pick_events(model: crate::Model) -> View {
    view! {
        div(class="box") {
            h2(class="title is-5") {
                "2. Pick an event"
                span(class="tag is-light is-pulled-right") { "Events" }
            }
            p(class="help") {
                "Load the demo event, search for a published event on Matrix, or plan a new one."
            }
            div(class="field") {
                div(class="control") {
                    button(
                        class="button is-link",
                        on:click=move |_| crate::update(model, crate::Msg::Show(crate::Screen::Events)),
                    ) {
                        span(class="icon is-small") { i(class="fa fa-folder-open") }
                        span { "Open event picker" }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::event::{EntryStatus, EventInfo, KTime, RunRecord, ScoreData, RUN_FINISH};

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
            repeats: 2,
            best_x: 1,
            timing: TimingStyle::Stopwatch,
        };
        ev.stages = vec![stage(1), stage(2)];
        let mut e1 = crate::event::Entry::new("1", "Alice");
        e1.status = EntryStatus::Started;
        let mut e2 = crate::event::Entry::new("2", "Bob");
        e2.status = EntryStatus::Submitted;
        let mut e3 = crate::event::Entry::new("3", "Carol");
        e3.status = EntryStatus::Withdrawn;
        let mut e4 = crate::event::Entry::new("4", "Dan");
        e4.status = EntryStatus::Reserve;
        ev.entries = vec![e1, e2, e3, e4];

        let counts = super::entry_counts(&ev);
        assert_eq!(counts, (4, 2, 1, 0, 1));

        let p = super::stage_progress(&ev, &sample_scores(), &sample_runs());
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].completed, 2);
        assert_eq!(p[1].completed, 1);
        assert_eq!(p[0].min_runs, 2); // both active have >= 1 run
        assert_eq!(p[0].all_runs, 1); // only Alice has 2 runs (repeats=2)
        assert_eq!(p[1].min_runs, 1); // only Alice ran stage 2
        assert_eq!(p[1].all_runs, 1);
    }
}
