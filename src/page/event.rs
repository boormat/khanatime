use crate::event::Entry;
use crate::event::EntryStatus;
use crate::event::Stage;
use crate::event::TimingStyle;
use crate::input::input_box;
use crate::input::input_clear;
use crate::input::input_feedback;
use crate::input::InputModel;
use crate::input::InputMsg;

// Event edit view.
// List of Classes. = derived from users?
// List of Entrants.
use lazy_regex::regex;
use sycamore::prelude::*;

#[derive(Clone)]
pub enum Msg {
    // classes
    EditClass(String), // borkish
    DeleteClass(String),
    ClassInput(InputMsg),
    // entry stuff
    EntryInput(InputMsg),
    ToggleClass { car: String, class: String },
    SetEntryStatus { car: String, status: EntryStatus },
    DeleteEntry(String),
    // draft event creation
    CreateDraft,
    // event details editing
    LoadDetails,
    SaveLocal,
    ToggleEdit,
    // per-test (stage) editing
    StageAdd,
    StageDelete(usize),
    // publish to Matrix
    Publish,
}

#[derive(Clone, Copy)]
pub struct Model {
    pub class: InputModel,
    pub entrant: InputModel,
    pub new_name: Signal<String>,
    pub new_club: Signal<String>,
    pub new_year: Signal<String>,
    pub feedback: Signal<String>,
    pub show_create: Signal<bool>,
    pub editing: Signal<bool>,
    pub edit_stages: Signal<Vec<Stage>>,
    /// Bumped when the stage list structure changes (add/remove) so the list
    /// re-renders, while per-keystroke field edits are untracked and don't.
    pub edit_rev: Signal<u8>,
    pub edit_club: Signal<String>,
    pub edit_year: Signal<String>,
    pub edit_event_date: Signal<String>,
    pub edit_entry_open: Signal<String>,
    pub edit_entry_close: Signal<String>,
    pub edit_stripe: Signal<String>,
    pub publish_status: Signal<Option<String>>,
}

pub fn init() -> Model {
    let year = js_sys::Date::new_0().get_full_year().to_string();
    Model {
        class: crate::input::init(),
        entrant: crate::input::init(),
        new_name: create_signal(String::new()),
        new_club: create_signal(String::new()),
        new_year: create_signal(year.clone()),
        feedback: create_signal(String::new()),
        show_create: create_signal(false),
        editing: create_signal(false),
        edit_stages: create_signal(crate::event::EventInfo::default().stages),
        edit_rev: create_signal(0),
        edit_club: create_signal(String::new()),
        edit_year: create_signal(year),
        edit_event_date: create_signal(String::new()),
        edit_entry_open: create_signal(String::new()),
        edit_entry_close: create_signal(String::new()),
        edit_stripe: create_signal(String::new()),
        publish_status: create_signal(None),
    }
}

fn save_event(model: crate::Model) {
    crate::event::save_event(&model.app.event.get_clone());
}

pub fn update(model: crate::Model, msg: Msg) {
    // TODO Use a result to update the feedback?
    match msg {
        Msg::ClassInput(InputMsg::CancelEdit) => {
            input_clear(model.screens.setup.class);
        }
        Msg::EntryInput(InputMsg::CancelEdit) => {
            input_clear(model.screens.setup.entrant);
        }
        Msg::EntryInput(InputMsg::DoThing) => {
            let input = model.screens.setup.entrant.input.get_clone();
            if let Some((car, name)) = parse_car_and(&input[..]) {
                let mut ok = false;
                model.app.event.update(|e| ok = e.add_entry(car, name));
                if ok {
                    save_event(model);
                    input_clear(model.screens.setup.entrant);
                } else {
                    input_feedback(model.screens.setup.entrant, "Duplicate Entry.");
                }
            } else {
                input_feedback(
                    model.screens.setup.entrant,
                    "Can't parse Entry. Car#<space>Name",
                );
            }
        }
        Msg::EditClass(class) => {
            model.screens.setup.class.input.set(class.clone());
            model
                .screens
                .setup
                .class
                .feedback
                .set(format!("Editing class {class}"));
        }

        Msg::ClassInput(InputMsg::DoThing) => {
            // new or rename... if key not null?
            let key = model.screens.setup.class.key.get_clone();
            let input = model.screens.setup.class.input.get_clone();
            if key.is_empty() {
                model.app.event.update(|e| e.add_class(&input));
            } else {
                // can't remove without removing drivers first?
                model.app.event.update(|e| {
                    e.rename_class(&key, &input);
                });
                save_event(model);
            }
            input_clear(model.screens.setup.class);
        }

        Msg::DeleteClass(class) => {
            khanatime::log!("delete {}", class);
            // can't remove without removing drivers first?
            let mut removed = false;
            model.app.event.update(|e| removed = e.remove_class(&class));
            if removed {
                save_event(model);
            }
        }
        Msg::ToggleClass { car, class } => {
            khanatime::log!("toggle {} {}", car, class);
            model.app.event.update(|e| {
                if let Some(entry) = e.entries.iter_mut().find(|entry| entry.car == car) {
                    if entry.classes.contains(&class) {
                        entry.classes.retain(|x| x != &class);
                    } else {
                        entry.classes.push(class.clone());
                    }
                }
            });
            save_event(model);
        }
        Msg::CreateDraft => create_draft(model),
        Msg::LoadDetails => load_details(model),
        Msg::SaveLocal => save_details(model),
        Msg::ToggleEdit => {
            if model.screens.setup.editing.get() {
                save_details(model);
                model.screens.setup.editing.set(false);
            } else {
                model.screens.setup.editing.set(true);
                load_details(model);
            }
        }
        Msg::StageAdd => {
            model.screens.setup.edit_stages.update(|v| {
                let num = v.len() as u8 + 1;
                v.push(Stage {
                    num,
                    name: format!("Test {num}"),
                    repeats: 1,
                    best_x: 1,
                    timing: TimingStyle::Stopwatch,
                });
            });
            model
                .screens
                .setup
                .edit_rev
                .set(model.screens.setup.edit_rev.get().wrapping_add(1));
        }
        Msg::StageDelete(idx) => {
            model.screens.setup.edit_stages.update(|v| {
                if idx < v.len() {
                    v.remove(idx);
                }
            });
            model
                .screens
                .setup
                .edit_rev
                .set(model.screens.setup.edit_rev.get().wrapping_add(1));
        }
        Msg::Publish => publish(model),
        Msg::DeleteEntry(car) => {
            khanatime::log!("delete entry {}", car);
            let mut removed = false;
            model.app.event.update(|e| removed = e.remove_entry(&car));
            if removed {
                save_event(model);
                crate::update(model, crate::Msg::Reload);
            }
        }
        Msg::SetEntryStatus { car, status } => {
            let mut updated = false;
            model
                .app
                .event
                .update(|e| updated = e.set_entry_status(&car, status));
            if updated {
                save_event(model);
            }
        }
    }
}

/// Build (or select) a draft event from the form fields and switch to it.
fn create_draft(model: crate::Model) {
    let name = model.screens.setup.new_name.get_clone().trim().to_string();
    let club = model.screens.setup.new_club.get_clone().trim().to_string();
    let year = model.screens.setup.new_year.get_clone().trim().to_string();
    if name.is_empty() {
        model
            .screens
            .setup
            .feedback
            .set("Event name is required".to_string());
        return;
    }
    let id = crate::event::build_event_id(&year, &club, &name);
    if !crate::event::valid_event_id(&id) {
        model
            .screens
            .setup
            .feedback
            .set("Can't build an event id from that — include a year (e.g. 2026)".to_string());
        return;
    }
    // Deterministic id: an existing event with the same id is selected, not duplicated.
    if crate::event::list_events().contains(&id) {
        model.screens.setup.feedback.set(String::new());
        model.screens.setup.show_create.set(false);
        crate::update(model, crate::Msg::SetEvent(id));
        return;
    }
    let e = crate::event::EventInfo {
        name,
        sponsoring_club: club,
        year,
        id: id.clone(),
        ..Default::default()
    };
    crate::event::save_event(&e);
    model.screens.setup.new_name.set(String::new());
    model.screens.setup.new_club.set(String::new());
    model.screens.setup.feedback.set(String::new());
    model.screens.setup.show_create.set(false);
    crate::update(model, crate::Msg::SetEvent(id));
}

/// Refresh the detail-edit fields from the current event.
fn load_details(model: crate::Model) {
    let e = model.app.event.get_clone();
    model.screens.setup.edit_club.set(e.sponsoring_club.clone());
    model.screens.setup.edit_year.set(e.year.clone());
    model
        .screens
        .setup
        .edit_event_date
        .set(e.event_date.clone());
    model
        .screens
        .setup
        .edit_entry_open
        .set(e.entry_open.clone());
    model
        .screens
        .setup
        .edit_entry_close
        .set(e.entry_close.clone());
    model.screens.setup.edit_stripe.set(e.stripe_link.clone());
    let mut stages = e.stages.clone();
    if stages.is_empty() {
        // Legacy event: migrate the global count/best-X-Y into per-stage rows.
        let mut ev = e.clone();
        ev.ensure_stages();
        stages = ev.stages;
    }
    model.screens.setup.edit_stages.set(stages);
}

/// Write the detail-edit fields back to the event.
fn save_details(model: crate::Model) {
    let em = model.screens.setup;
    let club = em.edit_club.get_clone().trim().to_string();
    let year = em.edit_year.get_clone().trim().to_string();
    let event_date = em.edit_event_date.get_clone().trim().to_string();
    let entry_open = em.edit_entry_open.get_clone().trim().to_string();
    let entry_close = em.edit_entry_close.get_clone().trim().to_string();
    let stripe = em.edit_stripe.get_clone().trim().to_string();
    let mut stages = em.edit_stages.get_clone();
    // Display/ordering is by `num`; stable on ties.
    stages.sort_by_key(|s| s.num);
    for s in stages.iter_mut() {
        if s.best_x > s.repeats {
            s.best_x = s.repeats;
        }
    }
    model.app.event.update(|e| {
        e.stages = stages.clone();
        e.sponsoring_club = club;
        e.year = year;
        e.event_date = event_date;
        e.entry_open = entry_open;
        e.entry_close = entry_close;
        e.stripe_link = stripe;
    });
    save_event(model);
    load_details(model);
    crate::update(model, crate::Msg::Reload);
}

/// Publish the current event to a Matrix space + timing room using the
/// identity logged in on the Home page.
fn publish(model: crate::Model) {
    #[cfg(target_arch = "wasm32")]
    publish_wasm(model);
    #[cfg(not(target_arch = "wasm32"))]
    model.screens.setup.publish_status.set(Some(
        "Matrix publishing is only available in the web build".to_string(),
    ));
}

#[cfg(target_arch = "wasm32")]
fn publish_wasm(model: crate::Model) {
    use crate::event::EventStatus;

    let em = model.screens.setup;
    if crate::services::matrix::client().is_none() {
        em.publish_status
            .set(Some("Log in on the Home page first".to_string()));
        return;
    }
    let event = model.app.event.get_clone();
    if event.id.is_empty() {
        em.publish_status
            .set(Some("Save the event first (needs a name)".to_string()));
        return;
    }
    em.publish_status.set(Some("Publishing...".to_string()));
    wasm_bindgen_futures::spawn_local(async move {
        let res = crate::services::matrix::publish_current_event(&event).await;
        match res {
            Ok(rooms) => {
                let mut event = event;
                event.space_id = Some(rooms.space.room_id().to_string());
                event.space_alias = Some(rooms.space_alias.to_string());
                event.timing_id = Some(rooms.timing.room_id().to_string());
                event.timing_alias = Some(rooms.timing_alias.to_string());
                event.status = EventStatus::Published;
                crate::event::save_event(&event);
                model.app.event.set(event);
                em.publish_status.set(Some("Published".to_string()));
            }
            Err(e) => em.publish_status.set(Some(format!("Publish failed: {e}"))),
        }
    });
}

pub fn view(model: crate::Model) -> View {
    view! {
        div {
            div(class="level") {
                div(class="level-left") {
                    h1(class="title is-4") {
                        (move || {
                            format!(
                                "Event: {}  Tests:{}",
                                model.app.event.with(|e| e.name.clone()),
                                model.app.event.with(|e| e.stage_count())
                            )
                        })
                    }
                }
                div(class="level-right") {
                    (view_edit_button(model))
                    (view_create_button(model))
                }
            }
            (move || {
                if model.screens.setup.show_create.get() {
                    view_draft(model)
                } else {
                    view! {}
                }
            })
            (view_details(model))
            (view_stages(model))
            (view_publish(model))
            (view_entrants(model))
        }
    }
}

/// "Edit"/"Done" toggle for the read-only event config.
fn view_edit_button(model: crate::Model) -> View {
    let em = model.screens.setup;
    view! {
        (move || {
            let editing = em.editing.get();
            view! {
                button(
                    class=format!("button {}", if editing { "is-success" } else { "is-light" }),
                    on:click=move |_| crate::update(model, crate::Msg::EventMsg(Msg::ToggleEdit)),
                ) {
                    (if editing { "Done" } else { "Edit" })
                }
            }
        })
    }
}

/// "Create New event" button toggling the draft form.
fn view_create_button(model: crate::Model) -> View {
    let em = model.screens.setup;
    view! {
        (move || {
            let open = em.show_create.get();
            view! {
                button(
                    class=format!("button {}", if open { "is-light" } else { "is-primary" }),
                    on:click=move |_| em.show_create.set(!em.show_create.get()),
                ) {
                    (if open { "Cancel" } else { "Create New event" })
                }
            }
        })
    }
}

/// Create a new draft event (name / club / year) and switch to it.
fn view_draft(model: crate::Model) -> View {
    let em = model.screens.setup;
    view! {
        div(class="box") {
            h2(class="title is-5") { "Draft Event" }
            div(class="field") {
                label(class="label") { "Name" }
                div(class="control") {
                    input(
                        class="input",
                        placeholder="e.g. Khanacross Round 1",
                        bind:value=em.new_name,
                        on:keydown=move |ev: web_sys::KeyboardEvent| {
                            if ev.key_code() == 13 {
                                crate::update(model, crate::Msg::EventMsg(Msg::CreateDraft));
                            }
                        },
                    )
                }
            }
            div(class="field") {
                label(class="label") { "Club / district" }
                div(class="control") {
                    input(class="input", placeholder="e.g. NDC", bind:value=em.new_club)
                }
            }
            div(class="field") {
                label(class="label") { "Year" }
                div(class="control") {
                    input(class="input", placeholder="e.g. 2026", bind:value=em.new_year)
                }
            }
            div(class="field") {
                div(class="control") {
                    button(
                        class="button is-primary",
                        on:click=move |_| crate::update(model, crate::Msg::EventMsg(Msg::CreateDraft)),
                    ) {
                        "Create event"
                    }
                }
            }
            (move || view_feedback(model))
        }
    }
}

fn view_feedback(model: crate::Model) -> View {
    let msg = model.screens.setup.feedback.get_clone();
    if msg.is_empty() {
        view! {}
    } else {
        view! { p(class="help is-danger") { (msg) } }
    }
}

/// Edit the current event's fixed details (tests, dates, club, year, stripe
/// link, classes).  Everything is read-only until "Edit" is pressed.
fn view_details(model: crate::Model) -> View {
    let em = model.screens.setup;
    let editing = em.editing.get();
    view! {
        div(class="box") {
            h2(class="title is-5") { "Event details" }
            div(class="field is-grouped") {
                div(class="control is-expanded") {
                    label(class="label") { "Club / district" }
                    input(class="input", placeholder="e.g. NDC", disabled=!editing, bind:value=em.edit_club)
                }
                div(class="control is-expanded") {
                    label(class="label") { "Year" }
                    input(class="input", placeholder="e.g. 2026", disabled=!editing, bind:value=em.edit_year)
                }
            }
            div(class="field") {
                label(class="label") { "Event date" }
                div(class="control") {
                    input(class="input", r#type="date", disabled=!editing, bind:value=em.edit_event_date)
                }
            }
            div(class="field is-grouped") {
                div(class="control is-expanded") {
                    label(class="label") { "Entry open" }
                    input(class="input", r#type="date", disabled=!editing, bind:value=em.edit_entry_open)
                }
                div(class="control is-expanded") {
                    label(class="label") { "Entry close" }
                    input(class="input", r#type="date", disabled=!editing, bind:value=em.edit_entry_close)
                }
            }
            div(class="field") {
                label(class="label") { "Stripe link" }
                div(class="control") {
                    input(class="input", placeholder="https://buy.stripe.com/...", disabled=!editing, bind:value=em.edit_stripe)
                }
            }
            div(class="field") {
                h3(class="title is-6") { "Classes" }
                (move || view_class_list(model))
                (move || {
                    if em.editing.get() {
                        view! {
                            div {
                                (input_box(
                                    em.class,
                                    "New Class?",
                                    move |msg| crate::update(model, crate::Msg::EventMsg(Msg::ClassInput(msg))),
                                ))
                            }
                        }
                    } else {
                        view! {}
                    }
                })
            }
            div(class="field is-grouped") {
                div(class="control") {
                    (move || {
                        if em.editing.get() {
                            view! {
                                button(
                                    class="button is-primary",
                                    on:click=move |_| crate::update(model, crate::Msg::EventMsg(Msg::SaveLocal)),
                                ) {
                                    "Save Local"
                                }
                            }
                        } else {
                            view! {}
                        }
                    })
                }
                div(class="control") {
                    (move || {
                        let st = model.app.event.with(|e| e.status.to_string());
                        let id = model.app.event.with(|e| e.id.clone());
                        view! { span(class="tag is-info") { (st) (if id.is_empty() { " · unsaved" } else { "" }) } }
                    })
                }
            }
        }
    }
}

/// Per-test config: name, number, repeats, best-X-of-Y, timing style.
fn view_stages(model: crate::Model) -> View {
    let em = model.screens.setup;
    let editing = em.editing.get();
    view! {
        div(class="box") {
            h2(class="title is-5") { "Tests / stages" }
            (move || view_stage_list(model))
            (move || {
                if em.editing.get() {
                    view! {
                        div(class="field") {
                            div(class="control") {
                                button(
                                    class="button is-small is-link",
                                    on:click=move |_| crate::update(model, crate::Msg::EventMsg(Msg::StageAdd)),
                                ) {
                                    span(class="icon is-small") { i(class="fa fa-plus") }
                                    span { "Add test" }
                                }
                            }
                        }
                    }
                } else {
                    view! {}
                }
            })
            (move || {
                if !editing {
                    view! { p(class="help") { "Press Edit to change tests." } }
                } else {
                    view! {}
                }
            })
        }
    }
}

fn view_stage_list(model: crate::Model) -> View {
    let em = model.screens.setup;
    let editing = em.editing.get();
    let _ = em.edit_rev.get();
    // Untracked: per-keystroke field edits must not rebuild (and so not lose
    // focus on) the inputs.  Structural changes bump `edit_rev` instead.
    let stages = untrack(|| em.edit_stages.get_clone());
    let rows: Vec<View> = stages
        .iter()
        .enumerate()
        .map(|(idx, s)| {
            if editing {
                view_stage_row(model, idx, s)
            } else {
                view_stage_row_readonly(s)
            }
        })
        .collect();
    view! {
        div {
            (if editing {
                view! {
                    div(class="columns is-vcentered is-hidden-mobile") {
                        div(class="column is-1") { strong { "No." } }
                        div(class="column is-4") { strong { "Name" } }
                        div(class="column is-2") { strong { "Repeats" } }
                        div(class="column is-2") { strong { "Best X of Y" } }
                        div(class="column is-2") { strong { "Timing" } }
                        div(class="column is-1") { }
                    }
                }
            } else {
                view! {}
            })
            (rows)
        }
    }
}

/// The value a text/number input currently holds (from its `input` event).
fn input_value(ev: &web_sys::Event) -> String {
    use wasm_bindgen::JsCast;
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|e| e.value())
        .unwrap_or_default()
}

/// The value a `<select>` currently holds (from its `change` event).
fn select_value(ev: &web_sys::Event) -> String {
    use wasm_bindgen::JsCast;
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlSelectElement>().ok())
        .map(|e| e.value())
        .unwrap_or_default()
}

/// Entry statuses in the order shown in the status dropdown.
const ENTRY_STATUSES: &[(&str, &str)] = &[
    ("draft", "Draft Entry"),
    ("submitted", "Entry Submitted"),
    ("accepted", "Accepted"),
    ("reserve", "Reserve"),
    ("confirmed", "Confirmed"),
    ("started", "Started"),
    ("withdrawn", "Withdrawn"),
];

fn entry_status_from(value: &str) -> EntryStatus {
    match value {
        "draft" => EntryStatus::Draft,
        "accepted" => EntryStatus::Accepted,
        "reserve" => EntryStatus::Reserve,
        "confirmed" => EntryStatus::Confirmed,
        "started" => EntryStatus::Started,
        "withdrawn" => EntryStatus::Withdrawn,
        _ => EntryStatus::Submitted,
    }
}

fn view_stage_row(model: crate::Model, idx: usize, stage: &Stage) -> View {
    let em = model.screens.setup;
    let num = stage.num.to_string();
    let name = stage.name.clone();
    let repeats = stage.repeats.to_string();
    let best_x = stage.best_x.to_string();
    view! {
        div(class="columns is-vcentered") {
            div(class="column is-1") {
                input(
                    class="input",
                    r#type="number",
                    min="1",
                    value=num,
                    on:input=move |ev: web_sys::Event| {
                        let v = input_value(&ev).trim().parse::<u8>().unwrap_or(1);
                        em.edit_stages.update(|st| if let Some(s) = st.get_mut(idx) { s.num = v; });
                    },
                )
            }
            div(class="column is-4") {
                input(
                    class="input",
                    placeholder="Test name",
                    value=name,
                    on:input=move |ev: web_sys::Event| {
                        let v = input_value(&ev);
                        em.edit_stages.update(|st| if let Some(s) = st.get_mut(idx) { s.name = v; });
                    },
                )
            }
            div(class="column is-2") {
                input(
                    class="input",
                    r#type="number",
                    min="1",
                    value=repeats,
                    on:input=move |ev: web_sys::Event| {
                        let v = input_value(&ev).trim().parse::<u8>().unwrap_or(1).max(1);
                        em.edit_stages.update(|st| if let Some(s) = st.get_mut(idx) { s.repeats = v; });
                    },
                )
            }
            div(class="column is-2") {
                input(
                    class="input",
                    r#type="number",
                    min="1",
                    value=best_x,
                    on:input=move |ev: web_sys::Event| {
                        let v = input_value(&ev).trim().parse::<u8>().unwrap_or(1).max(1);
                        em.edit_stages.update(|st| if let Some(s) = st.get_mut(idx) { s.best_x = v; });
                    },
                )
            }
            div(class="column is-2") {
                (view_timing_buttons(model, idx))
            }
            div(class="column is-1") {
                button(
                    class="delete is-danger",
                    title="Remove test",
                    on:click=move |_| crate::update(model, crate::Msg::EventMsg(Msg::StageDelete(idx))),
                )
            }
        }
    }
}

fn view_stage_row_readonly(stage: &Stage) -> View {
    let num = stage.num.to_string();
    let name = stage.name.clone();
    let repeats = stage.repeats.to_string();
    let best_x = stage.best_x.to_string();
    let timing_label = match stage.timing {
        TimingStyle::Stopwatch => "Stopwatch",
        TimingStyle::Rally => "Rally",
    };
    view! {
        div(class="columns is-vcentered") {
            div(class="column is-1") { span(class="tag is-light") { (num) } }
            div(class="column is-4") { (name) }
            div(class="column is-2") { (repeats) }
            div(class="column is-2") { (best_x) }
            div(class="column is-3") { span(class="tag is-light") { (timing_label) } }
        }
    }
}

fn view_timing_buttons(model: crate::Model, idx: usize) -> View {
    let em = model.screens.setup;
    view! {
        div(class="buttons has-addons") {
            (move || {
                let on = em.edit_stages.with(|st| st.get(idx).map(|s| s.timing == TimingStyle::Stopwatch).unwrap_or(false));
                view! {
                    button(
                        class=format!("button is-small {}", if on { "is-primary is-selected" } else { "is-light" }),
                        on:click=move |_| {
                            em.edit_stages.update(|st| if let Some(s) = st.get_mut(idx) { s.timing = TimingStyle::Stopwatch; });
                        },
                    ) { "Stopwatch" }
                }
            })
            (move || {
                let on = em.edit_stages.with(|st| st.get(idx).map(|s| s.timing == TimingStyle::Rally).unwrap_or(false));
                view! {
                    button(
                        class=format!("button is-small {}", if on { "is-primary is-selected" } else { "is-light" }),
                        on:click=move |_| {
                            em.edit_stages.update(|st| if let Some(s) = st.get_mut(idx) { s.timing = TimingStyle::Rally; });
                        },
                    ) { "Rally" }
                }
            })
        }
    }
}

/// Publish the event (space + timing room) using the Home-page identity.
fn view_publish(model: crate::Model) -> View {
    let em = model.screens.setup;
    view! {
        div(class="box") {
            h2(class="title is-5") { "Publish" }
            div(class="field is-grouped") {
                div(class="control") {
                    button(
                        class="button is-primary",
                        on:click=move |_| crate::update(model, crate::Msg::EventMsg(Msg::Publish)),
                    ) {
                        "Publish to Matrix"
                    }
                }
                div(class="control") {
                    (move || {
                        let status = model.app.event.with(|e| e.status.to_string());
                        let space = model.app.event.with(|e| e.space_alias.clone());
                        match space {
                            Some(alias) => view! { span(class="tag is-success") { "Published " (alias) } },
                            None => view! { span(class="tag is-light") { (status) } },
                        }
                    })
                }
            }
            (move || match em.publish_status.get_clone() {
                Some(s) => view! { p(class="help") { (s) } },
                None => view! { p(class="help") { "Publishes the event space + timing room with the logged-in account." } },
            })
        }
    }
}

fn view_class_list(model: crate::Model) -> View {
    let classes = model.app.event.with(|event| event.classes.clone());
    let editing = model.screens.setup.editing.get();
    let items = classes
        .iter()
        .map(|class| {
            let class = class.clone();
            let class_disp = class.clone();
            let class_del = class.clone();
            view! {
                li {
                    span(class="tag is-medium") {
                        (if editing {
                            view! {
                                i(
                                    class="fa fa-pen-to-square",
                                    on:click=move |_| {
                                        crate::update(model, crate::Msg::EventMsg(Msg::EditClass(class.clone())))
                                    },
                                )
                            }
                        } else {
                            view! {}
                        })
                        (class_disp)
                        (if editing {
                            view! {
                                button(
                                    class="delete is-danger",
                                    on:click=move |_| {
                                        crate::update(model, crate::Msg::EventMsg(Msg::DeleteClass(class_del.clone())))
                                    },
                                )
                            }
                        } else {
                            view! {}
                        })
                    }
                }
            }
        })
        .collect::<Vec<View>>();
    view! { ul(class="todo-list") { (items) } }
}

/// Entrants are part of the event: list with class toggles + remove, and an
/// add input.  Everything except the list is hidden unless editing.
fn view_entrants(model: crate::Model) -> View {
    let em = model.screens.setup;
    view! {
        div(class="box") {
            h2(class="title is-5") { "Entrants" }
            (move || view_entrant_list(model))
            (move || {
                if em.editing.get() {
                    view! {
                        div {
                            (input_box(
                                em.entrant,
                                "New Entrant?",
                                move |msg| crate::update(model, crate::Msg::EventMsg(Msg::EntryInput(msg))),
                            ))
                        }
                    }
                } else {
                    view! {}
                }
            })
        }
    }
}

fn view_entrant_list(model: crate::Model) -> View {
    let (entries, classes) = model
        .app
        .event
        .with(|event| (event.entries.clone(), event.classes.clone()));
    let items = entries
        .iter()
        .map(|entry| view_entry(model, entry, &classes))
        .collect::<Vec<View>>();
    view! {
        div {
            ul(class="todo-list") { (items) }
        }
    }
}

fn view_entry(model: crate::Model, entry: &Entry, classes: &Vec<String>) -> View {
    let car = entry.car.clone();
    let name = entry.name.clone();
    let entry_classes = entry.classes.clone();
    let status = entry.status.clone();
    let car_disp = car.clone();
    let car_del = car.clone();
    let car_st = car.clone();
    let editing = model.screens.setup.editing.get();

    let status_value = status.as_str().to_string();
    let status_label = status.to_string();
    let mut status_options: Vec<View> = vec![];
    for (value, label) in ENTRY_STATUSES {
        let value = value.to_string();
        let label = label.to_string();
        status_options.push(view! { option(value=value) { (label) } });
    }

    let mut class_checks: Vec<View> = vec![];
    if editing {
        for class in classes {
            let class = class.clone();
            let on = entry_classes.contains(&class);
            let c1 = class.clone();
            let car_c = car.clone();
            class_checks.push(view! {
                label(class="checkbox") {
                    input(
                        r#type="checkbox",
                        checked=on,
                        on:change=move |_| crate::update(model, crate::Msg::EventMsg(Msg::ToggleClass { car: car_c.clone(), class: c1.clone() })),
                    )
                    (class)
                }
            });
        }
    }

    view! {
        li {
            span(class="tag is-black") {
                i(class="fa fa-car", style="width: 20px")
                (car_disp)
            }
            span(style="width: 80px; margin: 10px") { (name) }
            (if editing {
                view! {
                    select(
                        class="select is-small",
                        value=status_value,
                        on:change=move |ev| {
                            let value = select_value(&ev);
                            crate::update(model, crate::Msg::EventMsg(Msg::SetEntryStatus {
                                car: car_st.clone(),
                                status: entry_status_from(&value),
                            }));
                        },
                    ) { (status_options) }
                }
            } else {
                view! {
                    span(class="tag is-light") { (status_label.clone()) }
                }
            })
            (class_checks)
            (if editing {
                view! {
                    button(
                        class="delete is-danger",
                        title="Remove entry",
                        on:click=move |_| crate::update(model, crate::Msg::EventMsg(Msg::DeleteEntry(car_del.clone()))),
                    )
                }
            } else {
                view! {}
            })
        }
    }
}

pub fn parse_car_and(cmd: &str) -> Option<(&str, &str)> {
    let re = regex!(r"^\d+[A-Z]? ");
    let s = cmd.trim();
    match re.find(s) {
        None => None,
        Some(m) => {
            let number = &s[0..m.end()].trim();
            let rest = &s[m.end()..].trim();
            Some((number, rest))
        }
    }
}
