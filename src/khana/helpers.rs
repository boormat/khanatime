use sycamore::prelude::*;
use wasm_bindgen::JsCast;

// ---------------------------------------------------------------------------
// Signing helper
// ---------------------------------------------------------------------------

/// Sign a TimingEvent with the device key.
fn sign_timing_event(
    te: &mut crate::khana::timing_event::TimingEvent,
) -> Result<(), crate::signing::SigningError> {
    let keys = crate::signing::DeviceKeys::load_from_storage()
        .ok_or(crate::signing::SigningError::NoPrivateKey)?;
    te.sign_with(&keys)
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Record a start/finish [crate::khana::event::RunRecord] locally (runs signal) and
/// enqueue it to the current event's pending outbox — the durable record until
/// it's flushed to the timing room.  No-op when no event is selected.
pub fn enqueue_run(model: crate::Model, run: &crate::khana::event::RunRecord) {
    let (id, uid) = model.khana.event.with(|e| (e.id.clone(), e.uid.clone()));
    if id.is_empty() || uid.is_empty() {
        return;
    }
    let mut run = run.clone();
    if run.uid.is_empty() {
        run.uid = crate::ids::gen_short_id();
    }
    model.khana.runs.update(|runs| {
        crate::khana::event::add_run(runs, run.clone());
    });
    let mut te = crate::khana::timing_event::TimingEvent {
        r#type: run.r#type.clone(),
        event_id: uid,
        uid: run.uid.clone(),
        target: None,
        test: run.test,
        car: run.car.clone(),
        ts: run.ts,
        time_ds: run.time_ds,
        status: run.status.clone(),
        flags: run.flags,
        official_id: run.official_id.clone(),
        comment: run.comment.clone(),
        refs: run.refs.clone(),
        signing_key: None,
        signature: None,
    };
    sign_timing_event(&mut te).expect("signing key missing");
    let sender = model.sync.identity.get_clone();
    crate::log::enqueue_pending(&id, crate::log::LogMsg::new_pending(te.body(), sender));
    crate::sync::flush_pending(model);
    crate::app::refresh_feed(model);
}

/// Enqueue a `finish` timing message for a stage/car/time (the command-line
/// stopwatch page) to the current event's pending outbox.
pub fn enqueue_ktime(
    model: crate::Model,
    test: u8,
    car: &str,
    time: &crate::khana::event::KTime,
    comment: Option<String>,
) {
    let (id, uid) = model.khana.event.with(|e| (e.id.clone(), e.uid.clone()));
    if id.is_empty() || uid.is_empty() {
        return;
    }
    let mut te = crate::khana::timing_event::TimingEvent::finish(&uid, test, car, time, vec![]);
    te.official_id = Some(model.sync.identity.get_clone());
    te.comment = comment;
    sign_timing_event(&mut te).expect("signing key missing");
    let sender = model.sync.identity.get_clone();
    crate::log::enqueue_pending(&id, crate::log::LogMsg::new_pending(te.body(), sender));
    let run = crate::khana::event::record_from_timing(&te);
    model.khana.runs.update(|runs| {
        crate::khana::event::add_run(runs, run);
    });
    crate::sync::flush_pending(model);
    crate::app::refresh_feed(model);
}

#[allow(dead_code)]
/// Correct an existing observation by `target_uid`: enqueue an `amend` message
/// and patch the local run record in place (the original stays in the log).
pub fn enqueue_amend(
    model: crate::Model,
    target_uid: &str,
    test: u8,
    car: &str,
    time: &crate::khana::event::KTime,
    comment: Option<String>,
) {
    let (id, uid) = model.khana.event.with(|e| (e.id.clone(), e.uid.clone()));
    if id.is_empty() || uid.is_empty() {
        return;
    }
    let mut te = crate::khana::timing_event::TimingEvent::amend(&uid, target_uid, test, car, time);
    te.official_id = Some(model.sync.identity.get_clone());
    te.comment = comment;
    sign_timing_event(&mut te).expect("signing key missing");
    let sender = model.sync.identity.get_clone();
    crate::log::enqueue_pending(&id, crate::log::LogMsg::new_pending(te.body(), sender));
    model.khana.runs.update(|runs| {
        if let Some(r) = runs.iter_mut().find(|r| r.uid == target_uid) {
            r.test = te.test;
            r.car = te.car.clone();
            r.time_ds = te.time_ds;
            r.status = te.status.clone();
            r.flags = te.flags;
            r.official_id = te.official_id.clone();
            r.comment = te.comment.clone();
        }
    });
    crate::sync::flush_pending(model);
    crate::app::refresh_feed(model);
    crate::update(model, crate::Msg::Reload);
}

#[allow(dead_code)]
/// Void an existing observation by `target_uid`: enqueue a `void` message and
/// mark the local run record voided (excluded from pairing/scores).
pub fn enqueue_void(model: crate::Model, target_uid: &str, test: u8, car: &str) {
    let (id, uid) = model.khana.event.with(|e| (e.id.clone(), e.uid.clone()));
    if id.is_empty() || uid.is_empty() {
        return;
    }
    let mut te = crate::khana::timing_event::TimingEvent::void(&uid, target_uid, test, car);
    te.official_id = Some(model.sync.identity.get_clone());
    sign_timing_event(&mut te).expect("signing key missing");
    let sender = model.sync.identity.get_clone();
    crate::log::enqueue_pending(&id, crate::log::LogMsg::new_pending(te.body(), sender));
    model.khana.runs.update(|runs| {
        if let Some(r) = runs.iter_mut().find(|r| r.uid == target_uid) {
            r.voided = true;
        }
    });
    crate::sync::flush_pending(model);
    crate::app::refresh_feed(model);
    crate::update(model, crate::Msg::Reload);
}

/// Return true if the selected car is "?" and the comment is empty, setting
/// feedback when so.  Shared by stopwatch, finish, and start screens.
pub fn check_unknown_comment(
    car: &str,
    comment: &str,
    feedback: &sycamore::prelude::Signal<Option<String>>,
) -> bool {
    if car.trim() == "?" && comment.trim().is_empty() {
        feedback.set(Some("Comment is required for unknown cars".to_string()));
        return true;
    }
    false
}

/// Shared timing log: all starts and finishes for a test, newest first, with
/// void buttons.  Used by all timing screens.
///
/// When `editing_uid` is `Some(signal)`, finish records show an edit button
/// that opens an inline amend form.  Pass `None` for read-only logs.
/// `provisional_uid` is the UID of a provisional finish record that should
/// auto-open in edit mode (the confirm interface).
/// `edit_state` caches the edit form signals across renders.
pub fn view_timing_log(
    model: crate::Model,
    test: u8,
    editing_uid: Option<Signal<Option<String>>>,
    provisional_uid: Option<Signal<Option<String>>>,
    edit_state: Option<Signal<Option<crate::khana::page::stopwatch::EditState>>>,
) -> View {
    use super::super::view as show;
    use crate::event::{KTime, KTimeTime, RunRecord, RUN_FINISH, RUN_START, RUN_STOP};
    use std::collections::HashSet;
    view! {
        div(class="box") {
            h3(class="title is-6") { "Log" }
            (move || {
                let _now = model.tick.get(); // subscribe to tick for live "Xs ago"
                let now = js_sys::Date::now() as i64;
                let mut runs: Vec<RunRecord> = model.khana.runs.with(|runs| {
                    runs.iter()
                        .filter(|r| r.test == test && !r.voided)
                        .filter(|r| r.r#type == RUN_START || r.r#type == RUN_FINISH || r.r#type == RUN_STOP)
                        .cloned()
                        .collect()
                });
                // T16: hide start/stop records already referenced by a finish.
                let finish_refs: HashSet<String> = runs.iter()
                    .filter(|r| r.r#type == RUN_FINISH)
                    .flat_map(|r| r.refs.iter().cloned())
                    .collect();
                runs.retain(|r| r.r#type == RUN_FINISH || !finish_refs.contains(&r.uid));
                runs.sort_by_key(|r| std::cmp::Reverse(r.ts));
                if runs.is_empty() {
                    return view! { p(class="help") { "No timing observations yet." } };
                }
                let editing: Option<String> = editing_uid.as_ref().and_then(|s| s.get_clone());
                // Auto-open provisional records in edit mode.
                let provisional: Option<String> = provisional_uid.as_ref().and_then(|s| s.get_clone());
                let effective_editing = editing.clone().or_else(|| provisional.clone());
                let views: Vec<View> = runs
                    .iter()
                    .map(|r| {
                        let uid = r.uid.clone();
                        if r.r#type == RUN_FINISH && effective_editing.as_deref() == Some(&uid) {
                            let r = r.clone();
                            let is_provisional = r.provisional;
                            return view_edit_row(model, &r, &editing_uid, &provisional_uid, &edit_state, is_provisional, now);
                        }
                        let (icon_char, icon_class) = if r.r#type == RUN_START {
                            ("\u{25B6}", "has-text-success")
                        } else if r.r#type == RUN_STOP {
                            ("\u{23F9}", "has-text-danger")
                        } else {
                            ("\u{25A0}", "")
                        };
                        let car_text = format!(" #{}", r.car);
                        let ts = fmt_ts(r.ts, now);
                        let time_view: View = if r.r#type == RUN_FINISH {
                            let kt = match r.status.as_deref() {
                                Some("dnf") => KTime::DNF,
                                Some("fts") => KTime::FTS,
                                Some("wd") => KTime::WD,
                                Some("nosho") => KTime::NOSHO,
                                _ => match r.time_ds {
                                    Some(ds) => KTime::Time(KTimeTime {
                                        time_ds: ds,
                                        flags: r.flags.unwrap_or(0),
                                        garage: r.status.as_deref() == Some("garage"),
                                    }),
                                    None => KTime::NOSHO,
                                },
                            };
                            show::ktime(&kt)
                        } else if r.r#type == RUN_STOP {
                            match r.time_ds {
                                Some(ds) => {
                                    let text = format!("{:.1}s", ds as f32 / 10.0);
                                    view! { span(class="has-text-grey-light", style="font-style: italic") { (text) } }
                                }
                                None => view! {},
                            }
                        } else {
                            view! {}
                        };
                        let official_view: View = match &r.official_id {
                            Some(o) if !o.is_empty() => {
                                let text = format!("by {}", o);
                                view! { span(class="has-text-grey-light ml-2") { (text) } }
                            }
                            _ => view! {},
                        };
                        let comment_view: View = match &r.comment {
                            Some(c) if !c.is_empty() => {
                                let text = format!("\"{}\"", c);
                                view! { span(class="has-text-grey ml-2 is-size-7") { (text) } }
                            }
                            _ => view! {},
                        };
                        let is_finish = r.r#type == RUN_FINISH;
                        let uid2 = uid.clone();
                        let uid3 = uid.clone();
                        view! {
                            div(class="level is-mobile") {
                                div(class="level-left") {
                                    span(class=icon_class) { (icon_char) }
                                    span(class="has-text-weight-semibold") { (car_text) }
                                    span(class="has-text-grey ml-2") { (ts) }
                                    span(class="ml-2") { (time_view) }
                                    (official_view)
                                    (comment_view)
                                }
                                div(class="level-right") {
                                    span(class="buttons are-small") {
                                        (if is_finish {
                                            if let Some(ref edit_sig) = editing_uid {
                                                let edit_sig = *edit_sig;
                                                view! {
                                                    button(
                                                        class="button is-small is-link is-light",
                                                        on:click=move |_| edit_sig.set(Some(uid2.clone())),
                                                    ) { span(class="icon is-small") { i(class="fa fa-pen") } }
                                                }
                                            } else {
                                                view! {}
                                            }
                                        } else {
                                            view! {}
                                        })
                                        button(
                                            class="button is-small is-light is-danger",
                                            on:click=move |_| crate::update(model, crate::Msg::VoidObservation(uid3.clone())),
                                        ) { span(class="icon is-small") { i(class="fa fa-xmark") } }
                                    }
                                }
                            }
                        }
                    })
                    .collect();
                views.into()
            })
        }
    }
}

struct EditSignals {
    time: Signal<String>,
    flags: Signal<u8>,
    garage: Signal<bool>,
    status: Signal<String>,
    comment: Signal<String>,
}

fn make_edit_state(r: &crate::event::RunRecord) -> crate::khana::page::stopwatch::EditState {
    let status_str = r.status.clone().unwrap_or_default();
    let is_garage = status_str == "garage";
    let time_s = r
        .time_ds
        .map(|ds| format!("{:.1}", ds as f32 / 10.0))
        .unwrap_or_default();
    let flags_val = r.flags.unwrap_or(0);
    let comment = r.comment.clone().unwrap_or_default();
    let status_initial = if is_garage {
        "clean".to_string()
    } else {
        status_str
    };
    crate::khana::page::stopwatch::EditState {
        uid: r.uid.clone(),
        time: create_signal(time_s),
        flags: create_signal(flags_val),
        garage: create_signal(is_garage),
        status: create_signal(status_initial),
        comment: create_signal(comment),
    }
}

/// Buttons for confirming a provisional finish record.
fn view_provisional_buttons(
    model: crate::Model,
    r: crate::event::RunRecord,
    editing_signal: Signal<Option<String>>,
    provisional_signal: Signal<Option<String>>,
    sigs: EditSignals,
) -> View {
    use crate::event::{KTime, KTimeTime};
    let prov_uid = r.uid.clone();
    let prov_uid2 = prov_uid.clone();
    let car = r.car.clone();
    let time_sig = sigs.time;
    let flags_sig = sigs.flags;
    let garage_sig = sigs.garage;
    let status_sig = sigs.status;
    let comment_sig = sigs.comment;
    view! {
        div(class="buttons are-small") {
            button(
                class="button is-link",
                on:click=move |_| {
                    let t: String = time_sig.get_clone();
                    let f = flags_sig.get();
                    let g = garage_sig.get();
                    let st: String = status_sig.get_clone();
                    let time_ds = t.parse::<f32>().map(|s| (10.0 * s) as u16).unwrap_or(0);
                    let kt = if st == "dnf" { KTime::DNF }
                        else if st == "fts" { KTime::FTS }
                        else if st == "wd" { KTime::WD }
                        else { KTime::Time(KTimeTime { time_ds, flags: f, garage: g }) };
                    let c: String = comment_sig.get_clone();
                    let comment_opt = if c.trim().is_empty() { None } else { Some(c) };
                    model.khana.runs.update(|runs| {
                        if let Some(r) = runs.iter_mut().find(|r| r.uid == prov_uid) {
                            r.time_ds = Some(time_ds);
                            r.status = Some(if g { "garage".to_string() } else { st });
                            r.flags = Some(f);
                            r.comment = comment_opt;
                            r.provisional = false;
                        }
                    });
                    let record = model.khana.runs.with(|runs| {
                        runs.iter().find(|r| r.uid == prov_uid).cloned()
                    });
                    if let Some(record) = record {
                        crate::khana::helpers::enqueue_run(model, &record);
                    }
                    let test = model.screens.stopwatch.test.get();
                    model.khana.scores.update(|s| {
                        crate::event::upsert_ktime(s, test, &car, kt);
                    });
                    editing_signal.set(None);
                    provisional_signal.set(None);
                },
            ) { "Confirm" }
            button(
                class="button is-light",
                on:click=move |_| {
                    model.khana.runs.update(|runs| {
                        runs.retain(|r| r.uid != prov_uid2);
                    });
                    editing_signal.set(None);
                    provisional_signal.set(None);
                },
            ) { "Cancel" }
        }
    }
}

/// Buttons for saving edits to an existing (confirmed) finish record.
fn view_edit_buttons(
    model: crate::Model,
    r: crate::event::RunRecord,
    editing_signal: Signal<Option<String>>,
    sigs: EditSignals,
) -> View {
    use crate::event::{KTime, KTimeTime};
    let save_uid = r.uid.clone();
    let car_save = r.car.clone();
    let time_sig = sigs.time;
    let flags_sig = sigs.flags;
    let garage_sig = sigs.garage;
    let status_sig = sigs.status;
    let comment_sig = sigs.comment;
    view! {
        div(class="buttons are-small") {
            button(
                class="button is-link",
                on:click=move |_| {
                    let t: String = time_sig.get_clone();
                    let f = flags_sig.get();
                    let g = garage_sig.get();
                    let st: String = status_sig.get_clone();
                    let time_ds = t.parse::<f32>().map(|s| (10.0 * s) as u16).unwrap_or(0);
                    let kt = if st == "dnf" { KTime::DNF }
                        else if st == "fts" { KTime::FTS }
                        else if st == "wd" { KTime::WD }
                        else { KTime::Time(KTimeTime { time_ds, flags: f, garage: g }) };
                    let c: String = comment_sig.get_clone();
                    let comment_opt = if c.trim().is_empty() { None } else { Some(c) };
                    crate::khana::helpers::enqueue_amend(
                        model, &save_uid, model.screens.stopwatch.test.get(),
                        &car_save, &kt, comment_opt,
                    );
                    editing_signal.set(None);
                },
            ) { "Save" }
            button(
                class="button is-light",
                on:click=move |_| editing_signal.set(None),
            ) { "Cancel" }
        }
    }
}

/// Inline edit form for a finish record (replaces the normal log row).
/// For provisional records, shows Confirm/Cancel instead of Save/Cancel.
fn view_edit_row(
    model: crate::Model,
    r: &crate::event::RunRecord,
    editing_uid: &Option<Signal<Option<String>>>,
    provisional_uid: &Option<Signal<Option<String>>>,
    edit_state: &Option<Signal<Option<crate::khana::page::stopwatch::EditState>>>,
    is_provisional: bool,
    _now: i64,
) -> View {
    use crate::event::RUN_START;
    use crate::khana::page::penalty;
    let signal = editing_uid.unwrap();
    let prov_signal = provisional_uid.and_then(Some);
    let r_for_provisional = r.clone();
    let r_for_edit = r.clone();
    let car = r.car.clone();

    let time_ds_val = r.time_ds.unwrap_or(0);
    let entry_name = model.khana.event.with(|e| {
        e.entries
            .iter()
            .find(|en| en.car == car)
            .map(|en| en.name.clone())
            .unwrap_or_default()
    });
    // Look up attached observations (start/stop records referenced by this finish's refs).
    let attached_uids: Vec<String> = r.refs.clone();
    let runs_clone = model.khana.runs.with(|runs| runs.clone());
    let attached_records: Vec<(String, String, String)> = attached_uids
        .iter()
        .filter_map(|uid| {
            runs_clone.iter().find(|r| r.uid == *uid).map(|r| {
                let icon = if r.r#type == RUN_START {
                    "\u{25B6}"
                } else {
                    "\u{23F9}"
                };
                let ts = fmt_ts(r.ts, js_sys::Date::now() as i64);
                (icon.to_string(), r.car.clone(), ts)
            })
        })
        .collect();
    let time_display_str = format!("{:.1}s", time_ds_val as f32 / 10.0);

    // Use cached edit state if available and matching this record's uid;
    // otherwise create fresh signals and cache them.
    let (time_sig, flags_sig, garage_sig, status_sig, comment_sig) = {
        let cached = edit_state.and_then(|s| s.get_clone());
        if let Some(ref c) = cached {
            if c.uid == r.uid {
                (c.time, c.flags, c.garage, c.status, c.comment)
            } else {
                let state = make_edit_state(r);
                let sigs = (
                    state.time,
                    state.flags,
                    state.garage,
                    state.status,
                    state.comment,
                );
                if let Some(es) = edit_state {
                    es.set(Some(state));
                }
                sigs
            }
        } else {
            let state = make_edit_state(r);
            let sigs = (
                state.time,
                state.flags,
                state.garage,
                state.status,
                state.comment,
            );
            if let Some(es) = edit_state {
                es.set(Some(state));
            }
            sigs
        }
    };

    let car_display = car.clone();

    let attached_obs: View = if !attached_records.is_empty() {
        let views: Vec<View> = attached_records
            .into_iter()
            .map(|(icon, a_car, a_ts)| {
                view! {
                    div(class="level is-mobile mb-0") {
                        div(class="level-left") {
                            span(class="is-size-7") {
                                span { (icon) }
                                span { (format!(" {} {}", a_car, a_ts)) }
                            }
                        }
                    }
                }
            })
            .collect();
        views.into()
    } else {
        view! {}
    };

    let buttons: View = if is_provisional {
        let sigs = EditSignals {
            time: time_sig,
            flags: flags_sig,
            garage: garage_sig,
            status: status_sig,
            comment: comment_sig,
        };
        view_provisional_buttons(model, r_for_provisional, signal, prov_signal.unwrap(), sigs)
    } else {
        let sigs = EditSignals {
            time: time_sig,
            flags: flags_sig,
            garage: garage_sig,
            status: status_sig,
            comment: comment_sig,
        };
        view_edit_buttons(model, r_for_edit, signal, sigs)
    };

    view! {
        div(class="box has-background-light") {
            div(class="level is-mobile mb-1") {
                div(class="level-left") {
                    (crate::view::car_tag(&car_display))
                    span(class="has-text-weight-semibold ml-2") { (entry_name) }
                    span(class="has-text-grey ml-2 is-size-7") { (time_display_str) }
                }
            }
            div(class="columns is-mobile is-vcentered is-gapless mb-2") {
                div(class="column") {
                    p(class="label is-small mb-1") { "Time (s)" }
                    input(
                        class="input is-small",
                        r#type="text",
                        placeholder="e.g. 12.5",
                        prop:value=move || time_sig.get_clone(),
                        on:input=move |e: web_sys::Event| {
                            if let Some(target) = e.target() {
                                if let Some(el) = target.dyn_ref::<web_sys::HtmlInputElement>() {
                                    time_sig.set(el.value());
                                }
                            }
                        },
                    )
                }
                div(class="column") {
                    p(class="label is-small mb-1") { "Comment" }
                    input(
                        class="input is-small",
                        r#type="text",
                        placeholder="comment",
                        prop:value=move || comment_sig.get_clone(),
                        on:input=move |e: web_sys::Event| {
                            if let Some(target) = e.target() {
                                if let Some(el) = target.dyn_ref::<web_sys::HtmlInputElement>() {
                                    comment_sig.set(el.value());
                                }
                            }
                        },
                    )
                }
            }
            (penalty::view_penalty_row(status_sig, flags_sig, garage_sig, time_ds_val, false, || {}))
            (attached_obs)
            (buttons)
        }
    }
}

/// Format an epoch-ms timestamp as `HH:MM:SS` (24h local), optionally
/// appending a relative age for entries under 60 minutes old.
pub fn fmt_ts(ts: i64, now: i64) -> String {
    let d = js_sys::Date::new(&js_sys::Number::from(ts as f64).into());
    let time = format!(
        "{:02}:{:02}:{:02}",
        d.get_hours(),
        d.get_minutes(),
        d.get_seconds()
    );
    if ts > now {
        return time;
    }
    let age_s = (now.saturating_sub(ts)) / 1000;
    if age_s < 60 {
        format!("{} ({}s ago)", time, age_s)
    } else if age_s < 3600 {
        format!("{} ({}m ago)", time, age_s / 60)
    } else {
        time
    }
}

/// Offline handoff box: export the current event's log as a QR parcel, scan or
/// paste one to import.  The exporter and scanner open as full-screen modals
/// (see [`view_handoff_modals`]); this box stays compact.
pub fn view_handoff(model: crate::Model) -> View {
    let status = model.sync.parcel_status.get_clone();
    let status_view = if status.is_empty() {
        view! {}
    } else {
        let s = status.clone();
        view! { p(class="help has-text-info") { (s) } }
    };
    let open_event = model.sync.parcel_open_event.get_clone();
    let open_view = if open_event.is_some() {
        let (_, name) = open_event.clone().unwrap();
        view! {
            div(class="control") {
                button(
                    class="button is-small is-warning",
                    on:click=move |_| crate::update(model, crate::Msg::OpenParcelEvent),
                ) {
                    span(class="icon is-small") { i(class="fa fa-folder-open") }
                    span { (format!("Open {name} and import")) }
                }
            }
        }
    } else {
        view! {}
    };
    view! {
        div(class="box is-hidden-print") {
            h2(class="title is-5") {
                "Offline handoff"
                span(class="tag is-light is-pulled-right") { "QR parcel" }
            }
            p(class="help") {
                "Carry the event's messages device-to-device with no network: export on one phone, scan or paste the parcel on another."
            }
            div(class="field is-grouped") {
                div(class="control") {
                    button(
                        class="button is-primary",
                        on:click=move |_| crate::update(model, crate::Msg::ExportParcel),
                    ) {
                        span(class="icon") { i(class="fa fa-qrcode") }
                        span { "Export parcel" }
                    }
                }
                div(class="control") {
                    button(
                        class="button is-warning",
                        on:click=move |_| crate::update(model, crate::Msg::ScanStart),
                    ) {
                        span(class="icon") { i(class="fa fa-camera") }
                        span { "Scan parcel" }
                    }
                }
            }
            div(class="field") {
                label(class="label is-small") { "Export contains" }
                div(class="control") {
                    div(class="buttons has-addons") {
                        button(
                            class=move || mode_class(model, crate::app::ParcelMode::Full),
                            on:click=move |_| crate::update(model, crate::Msg::SetParcelMode(crate::app::ParcelMode::Full)),
                        ) { "Full event" }
                        button(
                            class=move || mode_class(model, crate::app::ParcelMode::TimingOnly),
                            on:click=move |_| crate::update(model, crate::Msg::SetParcelMode(crate::app::ParcelMode::TimingOnly)),
                        ) { "Timing only" }
                    }
                }
            }
            div(class="field") {
                label(class="label is-small") { "Import a parcel" }
                div(class="control") {
                    textarea(
                        class="textarea is-small",
                        rows="4",
                        bind:value=model.sync.parcel_import,
                        placeholder="Paste a khanatime_parcel:… string",
                    ) {}
                }
                div(class="control") {
                    button(
                        class="button is-small is-link",
                        on:click=move |_| crate::update(model, crate::Msg::ImportParcel),
                    ) {
                        span(class="icon is-small") { i(class="fa fa-download") }
                        span { "Import parcel" }
                    }
                }
                (open_view)
            }
            (status_view)
        }
    }
}

/// Button class for the parcel-mode toggle: active mode is solid.
fn mode_class(model: crate::Model, mode: crate::app::ParcelMode) -> String {
    if model.sync.parcel_mode.get() == mode {
        "button is-small is-primary".to_string()
    } else {
        "button is-small".to_string()
    }
}

/// Full-screen modal overlays for the QR exporter and camera scanner.  Shown
/// whenever either is active; rendered at app level so it covers any screen.
pub fn view_handoff_modals(model: crate::Model) -> View {
    view! {
        (move || {
            if model.sync.scan_active.get() {
                view_scan_modal(model)
            } else if !model.sync.parcel_qr_svgs.with(|v| v.is_empty()) {
                view_qr_modal(model)
            } else {
                view! {}
            }
        })
    }
}

/// The exported parcel's QR modal: the current frame as a large SVG, animated
/// across frames when the parcel spans more than one code.
fn view_qr_modal(model: crate::Model) -> View {
    let svgs = model.sync.parcel_qr_svgs.get_clone();
    let i = model.sync.parcel_qr_index.get();
    let total = model.sync.parcel_qr_total.get();
    let paused = model.sync.parcel_qr_paused.get();
    let svg = svgs.get(i).cloned().unwrap_or_default();
    let exported = model.sync.parcel_export.get_clone();
    let hint = if total > 1 {
        let anim = if paused {
            "paused — resume to cycle frames."
        } else {
            "animated; hold the other phone's camera steady."
        };
        format!("Frame {}/{} — {anim}", i + 1, total)
    } else {
        "Show the other phone's camera at this code.".to_string()
    };
    let pause_label = if paused { "Resume" } else { "Pause" };
    view! {
        div(class="kt-modal") {
            div(class="kt-modal-card") {
                h3(class="title is-5") { "QR export" }
                div(class="kt-qr-box") {
                    div(dangerously_set_inner_html=svg) {}
                }
                p(class="help kt-qr-hint") { (hint) }
                div(class="field is-grouped is-grouped-centered") {
                    (if total > 1 {
                        view! {
                            div(class="control") {
                                button(
                                    class="button is-small is-light",
                                    on:click=move |_| crate::update(model, crate::Msg::QrPauseToggle),
                                ) {
                                    span(class="icon is-small") { i(class="fa fa-pause") }
                                    span { (pause_label) }
                                }
                            }
                        }
                    } else {
                        view! {}
                    })
                    div(class="control") {
                        button(
                            class="button is-small is-link",
                            on:click=move |_| crate::update(model, crate::Msg::QrClear),
                        ) {
                            span(class="icon is-small") { i(class="fa fa-times") }
                            span { "Close" }
                        }
                    }
                    (if exported.is_empty() {
                        view! {}
                    } else {
                        let copy_btn = exported.clone();
                        view! {
                            div(class="control") {
                                button(
                                    class="button is-small is-light",
                                    on:click=move |_| copy_text(&copy_btn),
                                ) {
                                    span(class="icon is-small") { i(class="fa fa-copy") }
                                    span { "Copy text" }
                                }
                            }
                        }
                    })
                }
            }
        }
    }
}

/// Full-screen camera viewfinder modal for scanning a parcel.
fn view_scan_modal(model: crate::Model) -> View {
    let status = model.sync.scan_status.get_clone();
    let status_view = if status.is_empty() {
        view! {}
    } else {
        let s = status.clone();
        view! { p(class="help has-text-white") { (s) } }
    };
    view! {
        div(class="kt-modal kt-modal-dark") {
            div(class="kt-modal-scan") {
                div(class="kt-scan-frame") {
                    video(
                        id="kt-scan-video",
                        autoplay=true,
                        playsinline=true,
                        muted=true,
                    ) {}
                    div(class="kt-scan-corners") {
                        i(class="kt-corner kt-corner-tl") {}
                        i(class="kt-corner kt-corner-tr") {}
                        i(class="kt-corner kt-corner-bl") {}
                        i(class="kt-corner kt-corner-br") {}
                    }
                    div(class="kt-scan-mask") {}
                }
                (status_view)
            }
            div(class="kt-modal-topbar") {
                button(
                    class="button is-light",
                    on:click=move |_| crate::update(model, crate::Msg::ScanStop),
                ) {
                    span(class="icon is-small") { i(class="fa fa-times") }
                    span { "Close" }
                }
            }
        }
    }
}

/// Copy `text` to the clipboard (best effort).
pub fn copy_text(text: &str) {
    if let Some(nav) = web_sys::window().map(|w| w.navigator()) {
        let _ = nav.clipboard().write_text(text);
    }
}
