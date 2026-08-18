use crate::event::Stage;
use crate::event::TimingStyle;
use crate::input::input_box;
use crate::input::input_clear;
use crate::input::InputModel;
use crate::input::InputMsg;

// Event edit view.
// List of Classes. = derived from users?
use sycamore::prelude::*;

#[derive(Clone)]
pub enum Msg {
    // classes (add / delete only — no rename)
    DeleteClass(String),
    ClassInput(InputMsg),
    // draft event creation
    CreateDraft,
    /// Copy the current event to a fresh draft (new id/name, entrants + tests).
    CopyAsNew,
    /// Compact + diff the staged edits and open the confirm modal.
    SaveBatch,
    /// Apply the staged event and enqueue one setup manifest.
    SendBatch,
    /// Close the confirm modal and keep editing.
    CancelBatch,
    /// Revert the edit form to the committed event and stop editing.
    DiscardBatch,
    ToggleEdit,
    // per-test (stage) editing
    StageAdd,
    StageDelete(usize),
    // publish + sync to Matrix
    Publish,
    /// Actually publish after confirmation.
    PublishDo,
    /// Cancel the publish confirmation.
    PublishCancel,
    // quick-add entrant
    QuickAdd(InputMsg),
    /// Load an entry into the text box for editing (keyed by car).
    EditEntry(String),
    /// Toggle a class on/off for an entry (keyed by car).
    ToggleEntryClass(String, String),
    /// Delete an entry (keyed by car).
    DeleteEntry(String),
}

#[derive(Clone, Copy)]
pub struct Model {
    pub class: InputModel,
    pub feedback: Signal<String>,
    /// Success confirmation (e.g. "Saved."), shown as an info line.
    pub saved: Signal<String>,
    pub editing: Signal<bool>,
    /// Copy of the event being edited.  None when not editing.
    pub edit_event: Signal<Option<crate::event::EventInfo>>,
    /// Bumped when the stage list structure changes (add/remove) so the list
    /// re-renders, while per-keystroke field edits are untracked and don't.
    pub edit_rev: Signal<u8>,
    // ---- flow state ----
    pub publish_status: Signal<Option<String>>,
    /// Set when a published event is amended locally and the timing room
    /// hasn't been re-synced yet.
    pub needs_sync: Signal<bool>,
    /// Batch-confirm modal content (the event diff) while open.
    pub confirm: Signal<Option<Vec<String>>>,
    /// Extra text shown in the confirm modal (e.g. remote updates arrived).
    pub confirm_warning: Signal<String>,
    /// Id of the event that was current before a fresh create/copy.  Discard
    /// restores it (dropping the unsaved draft).  `None` for normal edits.
    pub pre_create: Signal<Option<String>>,
    /// Snapshot of the committed event when an edit of a *published* event
    /// started — remote updates made meanwhile are warned about at save time.
    pub edit_base: Signal<Option<crate::event::EventInfo>>,
    /// Collapsible sections within the single event box.
    pub show_tests: Signal<bool>,
    pub show_entrants: Signal<bool>,
    // quick-add entrant
    pub quick_add: InputModel,
    /// car of the entry being edited (click-to-edit); used to preserve
    /// the entry when the edited text is re-submitted via quick-add.
    pub editing_entry_car: Signal<Option<String>>,
    /// Publish confirmation dialog.
    pub show_publish_confirm: Signal<bool>,
}

pub fn init() -> Model {
    Model {
        class: crate::input::init(),
        feedback: create_signal(String::new()),
        saved: create_signal(String::new()),
        editing: create_signal(false),
        edit_event: create_signal(None),
        edit_rev: create_signal(0),
        publish_status: create_signal(None),
        needs_sync: create_signal(false),
        confirm: create_signal(None),
        confirm_warning: create_signal(String::new()),
        pre_create: create_signal(None),
        edit_base: create_signal(None),
        show_tests: create_signal(crate::event::load_collapse("tests", true)),
        show_entrants: create_signal(crate::event::load_collapse("entrants", false)),
        quick_add: crate::input::init(),
        editing_entry_car: create_signal(None),
        show_publish_confirm: create_signal(false),
    }
}

/// Record an edit to the current event: enqueue a setup manifest to the
/// transaction log (the durable record of every edit) and refresh results.
fn commit_event(model: crate::Model) {
    crate::app::enqueue_setup(model);
    crate::update(model, crate::Msg::Reload);
}

pub fn update(model: crate::Model, msg: Msg) {
    // TODO Use a result to update the feedback?
    match msg {
        Msg::ClassInput(InputMsg::CancelEdit) => {
            input_clear(model.screens.setup.class);
        }

        Msg::ClassInput(InputMsg::DoThing) => {
            let em = model.screens.setup;
            let input = em.class.input.get_clone();
            let trimmed = input.trim().to_string();
            if !trimmed.is_empty() {
                em.edit_event.update(|e| {
                    if let Some(ref mut ev) = e {
                        if !ev.classes.contains(&trimmed) {
                            ev.classes.push(trimmed);
                        }
                    }
                });
            }
            input_clear(em.class);
        }

        Msg::DeleteClass(class) => {
            model.screens.setup.edit_event.update(|e| {
                if let Some(ref mut ev) = e {
                    ev.classes.retain(|c| c != &class);
                }
            });
        }
        Msg::CreateDraft => create_draft(model),
        Msg::CopyAsNew => copy_as_new(model),
        Msg::SaveBatch => save_batch(model),
        Msg::SendBatch => send_batch(model),
        Msg::CancelBatch => {
            model.screens.setup.confirm.set(None);
            model.screens.setup.confirm_warning.set(String::new());
        }
        Msg::DiscardBatch => discard_edits(model),
        Msg::ToggleEdit => {
            if model.screens.setup.editing.get() {
                discard_edits(model);
            } else {
                let em = model.screens.setup;
                em.edit_event.set(Some(model.khana.event.get_clone()));
                em.saved.set(String::new());
                em.editing.set(true);
                if is_published(model) {
                    em.edit_base.set(Some(model.khana.event.get_clone()));
                    crate::sync::refresh_from_room(model);
                }
            }
        }
        Msg::StageAdd => {
            let em = model.screens.setup;
            em.edit_event.update(|e| {
                if let Some(ref mut ev) = e {
                    let last = ev
                        .stages
                        .last()
                        .cloned()
                        .unwrap_or_else(|| crate::event::Stage::for_test(1));
                    let num = ev.stages.iter().map(|s| s.num).max().unwrap_or(0) + 1;
                    ev.stages.push(Stage {
                        num,
                        name: format!("Test {num}"),
                        runs_total: last.runs_total,
                        runs_scored: last.runs_scored,
                        timing: last.timing,
                    });
                }
            });
            em.edit_rev.set(em.edit_rev.get().wrapping_add(1));
        }
        Msg::StageDelete(idx) => {
            let em = model.screens.setup;
            let num = em
                .edit_event
                .with(|e| e.as_ref().and_then(|ev| ev.stages.get(idx).map(|s| s.num)));
            if let Some(num) = num {
                if crate::event::stage_has_timing(
                    &model.khana.scores.get_clone(),
                    &model.khana.runs.get_clone(),
                    num,
                ) {
                    em.feedback
                        .set(format!("Test {num} has timing data — amend, don't delete."));
                    return;
                }
            }
            em.edit_event.update(|e| {
                if let Some(ref mut ev) = e {
                    if idx < ev.stages.len() {
                        ev.stages.remove(idx);
                    }
                }
            });
            em.edit_rev.set(em.edit_rev.get().wrapping_add(1));
        }
        Msg::Publish => {
            let em = model.screens.setup;
            em.show_publish_confirm.set(true);
        }
        Msg::PublishDo => {
            model.screens.setup.show_publish_confirm.set(false);
            publish(model);
        }
        Msg::PublishCancel => {
            model.screens.setup.show_publish_confirm.set(false);
        }
        Msg::QuickAdd(InputMsg::DoThing) => {
            let em = model.screens.setup;
            let input = em.quick_add.input.get_clone();
            let input = input.trim();
            if input.is_empty() {
                return;
            }
            let editing_entry_car = em.editing_entry_car.get_clone();
            let edit_ev = em.edit_event.get_clone().unwrap_or_default();
            match parse_quick_entry(input, &edit_ev) {
                Ok(qp) => {
                    let mut entry = qp.entry;
                    if entry.name.is_empty() {
                        em.feedback.set("Driver name required.".into());
                        return;
                    }
                    if entry.classes.is_empty() {
                        em.feedback.set("At least one class required.".into());
                        return;
                    }
                    if entry.car.is_empty() {
                        let used: std::collections::HashSet<String> = edit_ev
                            .entries
                            .iter()
                            .map(|e| e.car.clone())
                            .filter(|c| !c.is_empty())
                            .collect();
                        entry.car = crate::event::next_free_number(&used);
                    }
                    // Preserve car when re-adding an edited entry.
                    if let Some(orig_car) = editing_entry_car {
                        entry.car = orig_car;
                        em.editing_entry_car.set(None);
                    }
                    let car = entry.car.clone();
                    let name = entry.name.clone();
                    let dup = edit_ev
                        .entries
                        .iter()
                        .any(|e| e.car == car || e.name == name);
                    if dup {
                        em.feedback
                            .set(format!("Car {car} or driver {name} already exists."));
                        return;
                    }
                    em.edit_event.update(|e| {
                        if let Some(ref mut ev) = e {
                            ev.entries.push(entry);
                        }
                    });
                    em.feedback.set(format!("Added #{} {}.", car, name));
                    crate::input::input_clear(em.quick_add);
                }
                Err(e) => {
                    if !e.is_empty() {
                        em.feedback.set(e);
                    }
                }
            }
        }
        Msg::QuickAdd(InputMsg::CancelEdit) => {
            crate::input::input_clear(model.screens.setup.quick_add);
            model.screens.setup.editing_entry_car.set(None);
        }
        Msg::EditEntry(car) => {
            let em = model.screens.setup;
            let mut ev = em.edit_event.get_clone().unwrap_or_default();
            if let Some(pos) = ev.entries.iter().position(|e| e.car == car) {
                let entry = ev.entries.remove(pos);
                let text = serialize_entry_for_edit(&entry);
                let car = entry.car.clone();
                let name = entry.name.clone();
                em.quick_add.input.set(text);
                em.edit_event.set(Some(ev));
                em.editing_entry_car.set(Some(car.clone()));
                em.feedback.set(format!("Editing {} {}.", car, name));
            }
        }
        Msg::ToggleEntryClass(car, class) => {
            model.screens.setup.edit_event.update(|e| {
                if let Some(ref mut ev) = e {
                    if let Some(entry) = ev.entries.iter_mut().find(|e| e.car == car) {
                        if entry.classes.contains(&class) {
                            entry.classes.retain(|c| c != &class);
                        } else {
                            entry.classes.push(class);
                        }
                    }
                }
            });
        }
        Msg::DeleteEntry(car) => {
            model.screens.setup.edit_event.update(|e| {
                if let Some(ref mut ev) = e {
                    ev.entries.retain(|e| e.car != car);
                }
            });
        }
    }
}

/// Serialize an entry back into the quick-add text format for editing.
fn serialize_entry_for_edit(entry: &crate::event::Entry) -> String {
    let mut parts: Vec<String> = vec![];
    // Car + Name
    if entry.car.is_empty() {
        parts.push(entry.name.clone());
    } else {
        parts.push(format!("{} {}", entry.car, entry.name));
    }
    // Classes (double-space separated from name)
    if !entry.classes.is_empty() {
        parts.push(entry.classes.join(" "));
    }
    // Vehicle (double-space separated)
    if let Some(ref v) = entry.vehicle {
        if !v.is_empty() {
            parts.push(v.clone());
        }
    }
    // Description (double-space separated)
    if let Some(ref d) = entry.description {
        parts.push(d.clone());
    }
    // Shared (double-space separated)
    if let Some(ref s) = entry.shared {
        parts.push(s.clone());
    }
    parts.join("  ")
}

/// Build the staged event: committed event + edit-form fields (details,
/// stages, classes).  Nothing is written to the event until the batch sends.
/// Compact (no-op) + diff the edited event against the committed one and open
/// the confirm modal.  A brand-new unsaved draft saves directly when there's
/// nothing to diff yet (the draft must still be persisted).
fn save_batch(model: crate::Model) {
    let em = model.screens.setup;
    let committed = model.khana.event.get_clone();
    let staged = em.edit_event.get_clone().unwrap_or_default();
    let diff = crate::batch::event_diff(&committed, &staged);
    if diff.is_empty() {
        if em.pre_create.get_clone().is_some() {
            em.feedback.set(String::new());
            send_batch(model);
        } else {
            em.feedback.set("No changes to save.".to_string());
        }
        return;
    }
    em.feedback.set(String::new());
    let mut warning = String::new();
    if is_published(model) {
        if let Some(base) = em.edit_base.get_clone() {
            if committed != base {
                warning = "Updates arrived from the room while you were editing — merged in best-effort; review the summary before sending."
                    .to_string();
            }
        }
    }
    em.confirm_warning.set(warning);
    em.confirm.set(Some(diff));
}

/// Apply the edited event and enqueue a single setup manifest (the batch).
fn send_batch(model: crate::Model) {
    let em = model.screens.setup;
    let staged = em.edit_event.get_clone().unwrap_or_default();
    model.khana.event.set(staged);
    commit_event(model);
    em.editing.set(false);
    em.edit_event.set(None);
    em.confirm.set(None);
    em.confirm_warning.set(String::new());
    em.edit_base.set(None);
    em.editing_entry_car.set(None);
    em.pre_create.set(None);
    em.saved.set("Saved.".to_string());
}

/// Make `ev` the current event with the edit form open, without writing it to
/// the log — it stays a fresh draft until the user hits Save.  The caller
/// records `pre_create` first so Discard can restore the previous event.
fn switch_to_draft(model: crate::Model, ev: crate::event::EventInfo) {
    let id = ev.id.clone();
    model.khana.event.set(ev.clone());
    model.khana.scores.set(Vec::new());
    model.khana.runs.set(Vec::new());
    crate::event::session_set_event(&id);
    crate::event::session_set_recent(&id);
    model.screens.chat.expanded.set(Default::default());
    model.screens.entry_app.staged.set(Vec::new());
    model.screens.entry_app.confirm.set(None);
    model.screens.entry_app.admin.set(false);
    model.screens.entry_app.show_form.set(false);
    model.sync.parcel_open_event.set(None);
    crate::app::refresh_feed(model);
    let em = model.screens.setup;
    em.edit_event.set(Some(ev));
    em.editing.set(true);
    em.edit_base.set(None);
}

/// Create a fresh draft and open it for editing.  The id is a random unique
/// key (never derived from the human fields); name/club/year stay empty until
/// the organiser fills the details form.  Nothing is saved until Save Local.
fn create_draft(model: crate::Model) {
    let year = js_sys::Date::new_0().get_full_year().to_string();
    let mut e = crate::event::EventInfo {
        id: crate::event::fresh_event_id(),
        year,
        ..Default::default()
    };
    // A fresh event starts with a single test; the organiser adds more.
    e.stages = vec![crate::event::Stage::for_test(1)];
    e.ensure_uid();
    model
        .screens
        .setup
        .pre_create
        .set(Some(model.khana.event.with(|e| e.id.clone())));
    model.screens.setup.feedback.set(String::new());
    model.screens.setup.saved.set(String::new());
    switch_to_draft(model, e);
}

/// Copy the current event to a fresh draft: new name + id + uid, Matrix links
/// cleared, no timing data.  Entrants and tests are copied; entrant state is
/// reset for the new event.  The original stays untouched.
fn copy_as_new(model: crate::Model) {
    let em = model.screens.setup;
    let src = model.khana.event.get_clone();
    if src.is_null() {
        em.feedback.set("No event to copy.".to_string());
        return;
    }
    let mut e = src.clone();
    let copy_name = if src.name.trim().is_empty() {
        "Untitled copy".to_string()
    } else if src.name.ends_with(" Copy") {
        format!("{} 2", src.name)
    } else {
        format!("{} Copy", src.name)
    };
    e.name = copy_name.clone();
    e.id = crate::event::fresh_event_id();
    e.uid = crate::ids::gen_short_id();
    e.status = crate::event::EventStatus::Draft;
    e.space_id = None;
    e.timing_id = None;
    e.event_homeservers = vec![];
    e.event_admins = vec![];
    e.owner = None;
    e.parent_rooms = vec![];
    // Entrants + tests are copied; entrant state is reset for the fresh event.
    em.pre_create.set(Some(src.id.clone()));
    switch_to_draft(model, e);
    em.feedback.set(format!(
        "Copied as \"{}\" — rename and tweak before saving.",
        copy_name
    ));
}

/// Abandon the current edit.  A fresh (unsaved) draft/copy is dropped and the
/// previous event restored; otherwise the form reverts to the committed event.
fn discard_edits(model: crate::Model) {
    let em = model.screens.setup;
    em.editing.set(false);
    em.edit_event.set(None);
    em.confirm.set(None);
    em.confirm_warning.set(String::new());
    em.edit_base.set(None);
    em.editing_entry_car.set(None);
    crate::input::input_clear(em.quick_add);
    let prev = em.pre_create.get_clone();
    em.pre_create.set(None);
    if let Some(prev) = prev {
        if prev.is_empty() {
            crate::update(model, crate::Msg::ClearEvent);
            crate::update(model, crate::Msg::Show(crate::Screen::Event));
        } else {
            crate::update(model, crate::Msg::SetEvent(prev));
        }
    }
}

/// Publish the current event to a Matrix space + timing room using the
/// identity logged in on the Home page.
fn publish(model: crate::Model) {
    let em = model.screens.setup;
    let event = model.khana.event.get_clone();
    let scores = model.khana.scores.get_clone();
    let runs = model.khana.runs.get_clone();
    let errs = crate::event::publish_errors(&event, &scores, &runs);
    if !errs.is_empty() {
        em.publish_status
            .set(Some(format!("Can't publish: {}", errs.join(" "))));
        return;
    }
    #[cfg(target_arch = "wasm32")]
    publish_wasm(model);
    #[cfg(not(target_arch = "wasm32"))]
    em.publish_status.set(Some(
        "Matrix publishing is only available in the web build".to_string(),
    ));
}

#[cfg(target_arch = "wasm32")]
fn publish_wasm(model: crate::Model) {
    use crate::event::EventStatus;

    let em = model.screens.setup;
    let mut event = model.khana.event.get_clone();
    if event.id.is_empty() {
        em.publish_status
            .set(Some("Save the event first (needs a name)".to_string()));
        return;
    }
    em.publish_status.set(Some("Publishing...".to_string()));
    wasm_bindgen_futures::spawn_local(async move {
        let res = crate::services::matrix::publish_current_event(&mut event).await;
        // Rooms are recorded on the event *before* finalize, so a partial
        // publish (rooms created, a later step failed) still has the ids —
        // persist it so a re-publish joins by id instead of alias resolution.
        let rooms_created = event.space_id.is_some();
        if rooms_created {
            event.status = EventStatus::Published;
            model.khana.event.set(event.clone());
            crate::app::enqueue_setup(model);
        }
        match (res, rooms_created) {
            (Ok(_), _) => {
                em.publish_status.set(Some("Published".to_string()));
                em.editing.set(false);
                em.edit_event.set(None);
                // Join the fresh timing room so the setup (and entrants, which
                // ride in the manifest) flush into it.
                crate::sync::join_current_event(model);
            }
            (Err(e), true) => {
                em.publish_status.set(Some(format!(
                    "Rooms created, but setup sync wasn't confirmed ({e}). The event is marked published — use \"Save and Publish\" to finish syncing."
                )));
                em.editing.set(false);
                em.edit_event.set(None);
                crate::sync::join_current_event(model);
            }
            (Err(e), false) => {
                em.publish_status.set(Some(format!(
                    "Publish failed: {e} — check you're signed in to {} on Home if the session expired.",
                    event.primary_homeserver().unwrap_or("a homeserver")
                )));
            }
        }
    });
}

pub fn view(model: crate::Model) -> View {
    view! {
        div {
            (view_header(model))
            (move || view_invite(model))
            (move || view_details(model))
            (view_confirm_modal(model))
            (view_publish_confirm_modal(model))
        }
    }
}

/// The event invite (QR + join URL), shown at the top of the config page once
/// the event is published and it isn't being edited.
fn view_invite(model: crate::Model) -> View {
    #[cfg(target_arch = "wasm32")]
    {
        let em = model.screens.setup;
        if em.editing.get() || !model.khana.event.with(|e| e.is_published()) {
            return view! {};
        }
        let event = model.khana.event.get_clone();
        let Some((url, svg, element_link)) = invite_view_data(&event) else {
            return view! {};
        };
        let url_c = url.clone();
        let element_c = element_link.clone();
        let element_btn: View = if element_link.is_empty() {
            view! {}
        } else {
            view! {
                button(
                    class="button is-small is-light",
                    on:click=move |_| crate::khana::helpers::copy_text(&element_c),
                ) { span(class="icon is-small") { i(class="fa fa-external-link") } span { "Copy Element link" } }
            }
        };
        view! {
            div(class="box") {
                h2(class="title is-5") { "Event invite" }
                div(class="field is-grouped is-vcentered") {
                    div(class="control") {
                        div(class="kt-qr-box") {
                            div(dangerously_set_inner_html=svg) {}
                        }
                    }
                    div(class="control is-expanded") {
                        p(class="help") { "Scan or share this link to join the event." }
                        div(class="field is-grouped") {
                            div(class="control is-expanded") {
                                input(class="input is-small", readonly=true, value=url) {}
                            }
                            div(class="control") {
                                button(
                                    class="button is-small is-light",
                                    on:click=move |_| crate::khana::helpers::copy_text(&url_c),
                                ) { span(class="icon is-small") { i(class="fa fa-copy") } span { "Copy URL" } }
                            }
                            (element_btn)
                        }
                    }
                }
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = model;
        view! {}
    }
}

/// The join-invite data for a published event: `(join_url, qr_svg, element_link)`.
/// `None` when the event has no space room yet.  wasm only (window + QR).
#[cfg(target_arch = "wasm32")]
fn invite_view_data(event: &crate::event::EventInfo) -> Option<(String, String, String)> {
    let sid = event.space_id.clone()?;
    let homeserver = event.primary_homeserver()?.to_string();
    let invite = crate::event::Invite {
        homeserver,
        event: event.id.clone(),
        sid,
        tid: event.timing_id.clone().unwrap_or_default(),
        reg: event.primary_reg(),
        admin_user: None,
        admin_pass: None,
    };
    let app_base = {
        let window = web_sys::window()?;
        let origin = window.location().origin().ok()?;
        let path = window.location().pathname().ok()?;
        format!("{origin}{path}")
    };
    let url = invite.url(&app_base);
    let svg = crate::services::qr::qr_svg(&url, 320).unwrap_or_default();
    let link = String::new(); // Element link is now per-homeserver, not per-event
    Some((url, svg, link))
}

/// Header line: event id (edit mode) or name + id (view mode), with the
/// lifecycle status tag.  Action buttons live at the top of the event box.
fn view_header(model: crate::Model) -> View {
    view! {
        div(class="level") {
            div(class="level-left") {
                h1(class="title is-4") {
                    (move || {
                        let editing = model.screens.setup.editing.get();
                        let (id, name) = model.khana.event.with(|e| (e.id.clone(), e.name.clone()));
                        if editing {
                            format!("event:{id}")
                        } else if id.is_empty() {
                            "Event".to_string()
                        } else {
                            format!("{name} — event:{id}")
                        }
                    })
                }
            }
            div(class="level-right") {
                (move || {
                    if model.khana.event.with(|e| e.is_null()) {
                        view! {}
                    } else {
                        view! {
                            button(
                                class="button is-small is-light",
                                on:click=move |_| crate::update(model, crate::Msg::Show(crate::Screen::Stage)),
                            ) {
                                span(class="icon is-small") { i(class="fa fa-keyboard") }
                                span { "Manual entry" }
                            }
                        }
                    }
                })
                (move || {
                    if model.khana.event.with(|e| e.is_null()) {
                        view! {}
                    } else {
                        view_status_tag(model)
                    }
                })
            }
        }
    }
}

/// Lifecycle status tag (draft / published / demo / amend-only) for the header.
/// "Published" is driven by the room ids, so it's accurate even if the status
/// flag lags a failed/partial publish.
fn view_status_tag(model: crate::Model) -> View {
    let (published, is_demo, status) = model
        .khana
        .event
        .with(|e| (e.is_published(), e.is_demo(), e.status.to_string()));
    let (class, label) = if is_demo {
        ("is-warning", "Demo (local only)")
    } else if published || status == "published" {
        ("is-success", "Published")
    } else if status == "draft" {
        ("is-info", "Draft")
    } else {
        ("is-success", "Amend-only (running/finished)")
    };
    view! { span(class=format!("tag {class}")) { (label) } }
}

fn view_confirm_modal(model: crate::Model) -> View {
    let em = model.screens.setup;
    crate::view::view_confirm_modal(
        em.confirm,
        move || {
            let published = model
                .khana
                .event
                .with(|e| e.status != crate::event::EventStatus::Draft);
            if published {
                "Save and Publish".to_string()
            } else {
                "Save Local".to_string()
            }
        },
        move || crate::update(model, crate::Msg::EventMsg(Msg::SendBatch)),
        move || crate::update(model, crate::Msg::EventMsg(Msg::CancelBatch)),
        move || crate::update(model, crate::Msg::EventMsg(Msg::DiscardBatch)),
        em.confirm_warning,
    )
}

/// Publish confirmation modal: shows errors or summary.  Publish button is
/// only enabled when there are zero errors.
fn view_publish_confirm_modal(model: crate::Model) -> View {
    let em = model.screens.setup;
    if !em.show_publish_confirm.get() {
        return view! {};
    }
    let event = model.khana.event.get_clone();
    let scores = model.khana.scores.get_clone();
    let runs = model.khana.runs.get_clone();
    let errs = crate::event::publish_errors(&event, &scores, &runs);
    let blocked = !errs.is_empty();
    let title = if blocked {
        "Cannot publish"
    } else {
        "Confirm publish"
    };
    // Pre-render body to avoid closure move issues with String values.
    let body = if blocked {
        let mut items: Vec<View> = Vec::new();
        for e in &errs {
            let msg = e.clone();
            items.push(view! { li(class="has-text-danger") { (msg) } });
        }
        view! {
            p(class="has-text-weight-semibold mb-2") { "Fix these issues before publishing:" }
            ul { (items) }
        }
    } else {
        let slug = crate::event::build_event_id(&event.year, &event.sponsoring_club, &event.name);
        let name = event.name.clone();
        let owner = event.owner.clone().unwrap_or_default();
        let space_alias = event.space_alias().unwrap_or_default();
        let timing_alias = event.timing_alias().unwrap_or_default();
        let already_published = event.space_id.is_some();
        let hs_count = event.event_homeservers.len();
        let hs_label = if hs_count == 1 {
            event
                .event_homeservers
                .first()
                .map(|h| crate::page::home::hs_host_port(h))
                .unwrap_or_default()
        } else {
            format!("{} homeservers", hs_count)
        };
        let admin_count = event.event_admins.len();
        let mut parts: Vec<View> = Vec::new();
        parts.push(view! { p { strong { (name) } } });
        if !slug.is_empty() {
            parts.push(view! { p { "Room: " code { "#" (slug) } } });
        }
        parts.push(view! { p { "Publish to: " (hs_label) } });
        if !space_alias.is_empty() {
            parts.push(view! { p { "Space: " code { (space_alias) } } });
            parts.push(view! { p { "Timing: " code { (timing_alias) } } });
        }
        if !owner.is_empty() {
            parts.push(view! { p { "Owner: " code { (owner) } } });
        }
        if admin_count > 0 {
            let admins = event.event_admins.join(", ");
            parts.push(view! { p { "Admins: " code { (admins) } " (invited + admin PL)" } });
        }
        parts.push(
            view! { p { "History: " code { "world_readable" } " (new joiners see everything)" } },
        );
        if already_published {
            parts.push(view! {
                div(class="notification is-warning is-light mt-3") {
                    p { strong { "Already published." } " Room already exists — will join the existing room instead of creating a new one." }
                }
            });
        }
        view! { (parts) }
    };
    let footer = if blocked {
        view! {
            button(class="button is-link", disabled=true) { "Publish" }
            button(class="button", on:click=move |_| {
                crate::update(model, crate::Msg::EventMsg(Msg::PublishCancel));
            }) { "Close" }
        }
    } else {
        view! {
            button(class="button is-link", on:click=move |_| {
                crate::update(model, crate::Msg::EventMsg(Msg::PublishDo));
            }) { "Publish" }
            button(class="button", on:click=move |_| {
                crate::update(model, crate::Msg::EventMsg(Msg::PublishCancel));
            }) { "Cancel" }
        }
    };
    view! {
        div(class="modal is-active") {
            div(class="modal-background", on:click=move |_| {
                crate::update(model, crate::Msg::EventMsg(Msg::PublishCancel));
            })
            div(class="modal-card") {
                header(class="modal-card-head") {
                    p(class="modal-card-title") { (title) }
                    button(class="delete", on:click=move |_| {
                        crate::update(model, crate::Msg::EventMsg(Msg::PublishCancel));
                    })
                }
                section(class="modal-card-body") { (body) }
                footer(class="modal-card-foot") { (footer) }
            }
        }
    }
}

/// True once an event has left the draft stage (published / running / finished).
fn is_published(model: crate::Model) -> bool {
    model
        .khana
        .event
        .with(|e| e.status != crate::event::EventStatus::Draft)
}

fn view_feedback(model: crate::Model) -> View {
    let msg = model.screens.setup.feedback.get_clone();
    if msg.is_empty() {
        view! {}
    } else {
        view! { p(class="help is-danger") { (msg) } }
    }
}

/// Edit the current event's details.  Everything is editable while editing —
/// including a published event (amend-only: no deletions, the class list never
/// renames, and the publish homeserver/reg lock once published).
fn view_details(model: crate::Model) -> View {
    if model.khana.event.with(|e| e.is_null()) {
        return view! {
            div(class="box") {
                p(class="help") { "No event selected — create a new event to configure it." }
                (view_action_bar(model))
            }
        };
    }
    let em = model.screens.setup;
    let editing = em.editing.get();
    view! {
        div(class="box") {
            (view_action_bar(model))
            (move || {
                let name = if editing {
                    untrack(|| em.edit_event.with(|e| e.as_ref().map(|e| e.name.clone()).unwrap_or_default()))
                } else {
                    model.khana.event.with(|e| e.name.clone())
                };
                let em = em;
                view! {
                    div(class="field") {
                        label(class="label") { "Name" }
                        div(class="control") {
                            input(class="input", placeholder="e.g. Khanacross Round 1", disabled=!editing, value=name,
                                on:input=move |ev| {
                                    let v = input_value(&ev);
                                    em.edit_event.update(|e| { if let Some(ref mut ev) = e { ev.name = v; } });
                                },
                            )
                        }
                    }
                }
            })
            (move || {
                let (club, year) = if editing {
                    untrack(|| em.edit_event.with(|e| e.as_ref().map(|e| (e.sponsoring_club.clone(), e.year.clone())).unwrap_or_default()))
                } else {
                    model.khana.event.with(|e| (e.sponsoring_club.clone(), e.year.clone()))
                };
                view! {
                    div(class="field is-grouped") {
                        div(class="control is-expanded") {
                            label(class="label") { "Club / district" }
                            input(class="input", placeholder="e.g. NDC", disabled=!editing, value=club,
                                on:input=move |ev| {
                                    let v = input_value(&ev);
                                    em.edit_event.update(|e| { if let Some(ref mut ev) = e { ev.sponsoring_club = v; } });
                                },
                            )
                        }
                        div(class="control is-expanded") {
                            label(class="label") { "Year" }
                            input(class="input", placeholder="e.g. 2026", disabled=!editing, value=year,
                                on:input=move |ev| {
                                    let v = input_value(&ev);
                                    em.edit_event.update(|e| { if let Some(ref mut ev) = e { ev.year = v; } });
                                },
                            )
                        }
                    }
                }
            })
            (move || {
                let val = if editing {
                    untrack(|| em.edit_event.with(|e| e.as_ref().map(|e| e.event_date.clone()).unwrap_or_default()))
                } else {
                    model.khana.event.with(|e| e.event_date.clone())
                };
                view! {
                    div(class="field") {
                        label(class="label") { "Event date" }
                        div(class="control") {
                            input(class="input", r#type="date", disabled=!editing, value=val,
                                on:input=move |ev| {
                                    let v = input_value(&ev);
                                    em.edit_event.update(|e| { if let Some(ref mut ev) = e { ev.event_date = v; } });
                                },
                            )
                        }
                    }
                }
            })
            (move || {
                let val = if editing {
                    untrack(|| em.edit_event.with(|e| e.as_ref().map(|e| e.parent_rooms.join(", ")).unwrap_or_default()))
                } else {
                    model.khana.event.with(|e| e.parent_rooms.join(", "))
                };
                view! {
                    div(class="field") {
                        label(class="label") { "Parent rooms" }
                        div(class="field has-addons") {
                            div(class="control is-expanded") {
                                input(class="input", placeholder="Optional — club/organisation room aliases (comma-separated)", disabled=!editing, value=val,
                                    on:input=move |ev| {
                                        let v = input_value(&ev);
                                        let rooms: Vec<String> = v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                                        em.edit_event.update(|e| { if let Some(ref mut ev) = e { ev.parent_rooms = rooms; } });
                                    },
                                )
                            }
                            (if editing {
                                view! {
                                    div(class="control") {
                                        button(
                                            class="button is-light",
                                            title="Clear parent rooms",
                                            on:click=move |_| {
                                                em.edit_event.update(|e| { if let Some(ref mut ev) = e { ev.parent_rooms = vec![]; } });
                                            },
                                        ) {
                                            span(class="icon is-small") { i(class="fa fa-eraser") }
                                            span { "Clear" }
                                        }
                                    }
                                }
                            } else {
                                view! {}
                            })
                        }
                    }
                }
            })
            (view_homeserver_fields(model))
            (move || view_tests_section(model))
            (move || view_classes_section(model))
            (move || view_entrants_section(model))
            hr() {}
            (move || view_publish_status(model))
            (move || view_publish_message(model))
            (move || view_feedback(model))
            (move || {
                let msg = em.saved.get_clone();
                if msg.is_empty() {
                    view! {}
                } else {
                    view! { p(class="help is-success") { (msg) } }
                }
            })
        }
    }
}

/// A collapsible section header: chevron + title + count, toggles `open`.
fn view_section_header(
    open: Signal<bool>,
    title: &'static str,
    count: usize,
    storage_key: &'static str,
) -> View {
    view! {
        button(
            class="button is-fullwidth is-light is-small",
            on:click=move |_| {
                let new_val = !open.get();
                open.set(new_val);
                crate::event::save_collapse(storage_key, new_val);
            },
        ) {
            span(class="icon is-small") {
                i(class=move || if open.get() { "fa fa-chevron-down" } else { "fa fa-chevron-right" })
            }
            span { (title) }
            span(class="tag is-light is-pulled-right") { (count) }
        }
    }
}

/// Tests/stages — collapsible part of the single event box.
fn view_tests_section(model: crate::Model) -> View {
    let em = model.screens.setup;
    let editing = em.editing.get();
    let published = is_published(model);
    // Reactive count: edit_event stages during editing (tracks add/remove via
    // edit_rev), committed stages otherwise.
    let count = if editing {
        let _ = em.edit_rev.get();
        untrack(|| {
            em.edit_event
                .with(|e| e.as_ref().map(|e| e.stages.len()).unwrap_or_default())
        })
    } else {
        model.khana.event.with(|e| e.stage_count())
    };
    view! {
        div(class="field") {
            (view_section_header(em.show_tests, "Tests / stages", count, "tests"))
            (move || {
                if !em.show_tests.get() {
                    return view! {};
                }
                view! {
                    div(class="mt-2") {
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
                            } else if published {
                                view! {
                                    p(class="help") {
                                        "Tests are editable, but can't be removed once published (amend, not delete)."
                                    }
                                }
                            } else {
                                view! {}
                            }
                        })
                    }
                }
            })
        }
    }
}

/// Classes — always visible, horizontal chips.
fn view_classes_section(model: crate::Model) -> View {
    let em = model.screens.setup;
    let editing = em.editing.get();
    view! {
        div(class="field") {
            label(class="label is-small") { "Classes" }
            (move || view_class_chips(model, editing))
            (move || {
                if editing {
                    view! {
                        div(class="control mt-2") {
                            (input_box(
                                em.class,
                                "New class",
                                move |msg| crate::update(model, crate::Msg::EventMsg(Msg::ClassInput(msg))),
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

/// Render classes as horizontal chips with whitespace between them.
fn view_class_chips(model: crate::Model, editing: bool) -> View {
    let classes = if editing {
        model
            .screens
            .setup
            .edit_event
            .with(|e| e.as_ref().map(|e| e.classes.clone()).unwrap_or_default())
    } else {
        model.khana.event.with(|e| e.classes.clone())
    };
    let items: Vec<View> = classes
        .iter()
        .map(|cl| {
            let cl = cl.clone();
            let cl_del = cl.clone();
            view! {
                span(class="tag is-info is-light mr-2") {
                    (cl.clone())
                    (if editing {
                        view! {
                            button(
                                class="delete is-small ml-1",
                                title="Remove class",
                                on:click=move |_| {
                                    crate::update(model, crate::Msg::EventMsg(Msg::DeleteClass(cl_del.clone())))
                                },
                            )
                        }
                    } else {
                        view! {}
                    })
                }
            }
        })
        .collect();
    view! { div(class="tags") { (items) } }
}

/// Quick-add entrant: text field + staged entries preview (edit mode only).
fn view_quick_add(model: crate::Model) -> View {
    let em = model.screens.setup;
    view! {
        (move || {
            if !em.editing.get() {
                return view! {};
            }
            let dispatch = move |msg: InputMsg| {
                crate::update(model, crate::Msg::EventMsg(Msg::QuickAdd(msg)))
            };
            view! {
                div(class="field") {
                    label(class="label is-small") { "Quick add entrant" }
                    div(class="control") {
                        (input_box(
                            em.quick_add,
                            "123 John Smith  Outright  Toyota GR Yaris  Rego  Group",
                            dispatch,
                        ))
                    }
                    p(class="help") {
                        "Format: Number Name  Class1 Class2  Vehicle  Description  SharedGroup — double-space between fields."
                    }
                    (move || {
                        let fb = em.feedback.get_clone();
                        if fb.is_empty() {
                            view! {}
                        } else {
                            view! { p(class="help is-danger") { (fb) } }
                        }
                    })
                    // Live preview: parse on every keystroke and show result
                    (move || {
                        let input = em.quick_add.input.get_clone();
                        let input = input.trim().to_string();
                        if input.is_empty() {
                            return view! {};
                        }
                        let event = if model.screens.setup.editing.get() {
                            model.screens.setup.edit_event.with(|e| e.clone().unwrap_or_default())
                        } else {
                            model.khana.event.get_clone()
                        };
                        match parse_quick_entry(&input, &event) {
                            Ok(qp) => {
                                let entry = &qp.entry;
                                let cf = qp.cursor_field;
                                let defaulted = &qp.defaulted;
                                let name_present = !entry.name.is_empty();
                                let classes_present = !entry.classes.is_empty();
                                let is_ready = name_present && classes_present;

                                let mut tags: Vec<View> = vec![];

                                // Car (black if typed, light warning if auto-assigned)
                                if entry.car.is_empty() {
                                    let used: std::collections::HashSet<String> = event
                                        .entries
                                        .iter()
                                        .map(|e| e.car.clone())
                                        .filter(|c| !c.is_empty())
                                        .collect();
                                    let predicted = crate::event::next_free_number(&used);
                                    let msg = format!("#{predicted}\u{26A1}");
                                    tags.push(view! { span(class="tag is-warning is-light") { (msg) } });
                                } else {
                                    let c = format!("#{}", entry.car);
                                    tags.push(view! { span(class="tag is-black") { (c) } });
                                }

                                // Name (link-blue)
                                if !entry.name.is_empty() {
                                    let n = format!("Name: {}", entry.name);
                                    let cls = if cf == 0 { "tag is-link" } else { "tag is-link is-light" };
                                    tags.push(view! { span(class=cls) { (n) } });
                                }

                                // Classes (info-blue, one per class)
                                for cl in &entry.classes {
                                    let cl_text = format!("Class: {cl}");
                                    let cls = if cf == 1 { "tag is-info" } else { "tag is-info is-light" };
                                    tags.push(view! { span(class=cls) { (cl_text) } });
                                }

                                // Ready indicator (green)
                                if is_ready {
                                    tags.push(view! { span(class="tag is-success is-light") { "\u{2713} Ready" } });
                                }

                                // Vehicle (always show)
                                if let Some(ref v) = entry.vehicle {
                                    if !v.is_empty() {
                                        let v_text = format!("Vehicle: {v}");
                                        let cls = if cf == 2 { "tag is-link" } else { "tag is-light" };
                                        tags.push(view! { span(class=cls) { (v_text) } });
                                    } else if cf == 2 {
                                        tags.push(view! { span(class="tag is-link") { "Vehicle: ?" } });
                                    } else if cf > 2 {
                                        tags.push(view! { span(class="tag is-light") { "Vehicle: ?" } });
                                    }
                                } else if cf == 2 {
                                    tags.push(view! { span(class="tag is-link") { "Vehicle: ?" } });
                                } else if cf > 2 {
                                    tags.push(view! { span(class="tag is-light") { "Vehicle: ?" } });
                                }

                                // Description (always show)
                                let has_desc_default = defaulted.contains(&"Description");
                                if let Some(ref d) = entry.description {
                                    let d_text = format!("Desc: {d}");
                                    let cls = if cf == 3 {
                                        "tag is-link"
                                    } else if has_desc_default {
                                        "tag is-warning is-light"
                                    } else {
                                        "tag is-light"
                                    };
                                    let suffix = if has_desc_default && cf != 3 { "\u{26A1}" } else { "" };
                                    let text = format!("{d_text}{suffix}");
                                    tags.push(view! { span(class=cls) { (text) } });
                                } else if cf == 3 {
                                    tags.push(view! { span(class="tag is-link") { "Desc: ?" } });
                                } else if cf > 3 {
                                    tags.push(view! { span(class="tag is-warning is-light") { "Desc: ?" } });
                                }

                                // Shared car group (always show)
                                if let Some(ref s) = entry.shared {
                                    let s_text = format!("Shared: {s}");
                                    let cls = if cf == 4 { "tag is-link" } else { "tag is-light" };
                                    tags.push(view! { span(class=cls) { (s_text) } });
                                } else if cf == 4 {
                                    tags.push(view! { span(class="tag is-link") { "Shared: ?" } });
                                } else if !is_ready {
                                    // Show ⚡ for shared when not ready and not yet reached
                                }

                                // Error indicator (red)
                                let error_view = match &qp.extra_warning {
                                    Some(w) => {
                                        let w = w.clone();
                                        view! { span(class="tag is-danger is-light ml-2") { (w) } }
                                    }
                                    None => view! {},
                                };

                                view! {
                                    div(class="tags are-small mt-1") {
                                        (tags)
                                        (error_view)
                                    }
                                }
                            }
                            Err(e) => {
                                if e.is_empty() {
                                    view! {}
                                } else {
                                    view! { p(class="help is-danger mt-1") { (e) } }
                                }
                            }
                        }
                    })
                }
            }
        })
    }
}

/// Entrants — collapsible read-only list (management lives on the Entries
/// screen; the event only carries the final entrant list).
fn view_entrants_section(model: crate::Model) -> View {
    let em = model.screens.setup;
    let editing = em.editing.get();
    let count = if editing {
        em.edit_event
            .with(|e| e.as_ref().map(|e| e.entries.len()).unwrap_or_default())
    } else {
        model.khana.event.with(|e| e.entries.len())
    };
    view! {
        div(class="field") {
            (view_section_header(em.show_entrants, "Entrants", count, "entrants"))
            (move || {
                if !em.show_entrants.get() {
                    return view! {};
                }
                view! {
                    div(class="mt-2") {
                        (view_quick_add(model))
                        (move || view_entrant_list_readonly(model))
                    }
                }
            })
        }
    }
}

/// Read-only entrant list: merged staged + committed entries, sorted, with
/// class checkboxes, delete button, and click-to-edit.
fn view_entrant_list_readonly(model: crate::Model) -> View {
    let em = model.screens.setup;
    let editing = em.editing.get();
    let event_classes = if editing {
        em.edit_event
            .with(|e| e.as_ref().map(|e| e.classes.clone()).unwrap_or_default())
    } else {
        model.khana.event.with(|e| e.classes.clone())
    };

    let entries = if editing {
        em.edit_event
            .with(|e| e.as_ref().map(|e| e.entries.clone()).unwrap_or_default())
    } else {
        model.khana.event.with(|e| e.entries.clone())
    };

    if entries.is_empty() {
        return view! {
            p(class="help") { "No entrants yet." }
        };
    }

    let items: Vec<View> = entries
        .iter()
        .map(|e| {
            let car = e.car.clone();
            let name = e.name.clone();
            let classes = e.classes.clone();
            let vehicle = e.vehicle.clone().unwrap_or_default();
            let desc = e.description.clone().unwrap_or_default();
            let shared = e.shared.clone().unwrap_or_default();

            // Car tag
            let car_tag_text = car.clone();
            let car_tag: View = if car.is_empty() {
                view! { span(class="tag is-light") { "?" } }
            } else {
                view! { span(class="tag is-black") { (car_tag_text) } }
            };

            // Class checkboxes (if editing) or class tags (if viewing)
            let class_display: Vec<View> = if editing {
                event_classes
                    .iter()
                    .map(|cl| {
                        let cl = cl.clone();
                        let on = classes.contains(&cl);
                        let c1 = cl.clone();
                        let car_for_cls = car.clone();
                        view! {
                            label(class="checkbox is-small") {
                                input(
                                    r#type="checkbox",
                                    checked=on,
                                    on:change=move |_| {
                                        crate::update(model, crate::Msg::EventMsg(Msg::ToggleEntryClass(car_for_cls.clone(), c1.clone())));
                                    },
                                )
                                (cl)
                            }
                        }
                    })
                    .collect()
            } else {
                classes
                    .iter()
                    .map(|cl| {
                        let cl = cl.clone();
                        view! { span(class="tag is-info is-light mr-1") { (cl) } }
                    })
                    .collect()
            };

            // Delete button (if editing)
            let delete_btn: View = if editing {
                let car_for_del = car.clone();
                view! {
                    button(
                        class="delete is-small ml-2",
                        title="Withdraw entry",
                        on:click=move |_| {
                            crate::update(model, crate::Msg::EventMsg(Msg::DeleteEntry(car_for_del.clone())));
                        },
                    )
                }
            } else {
                view! {}
            };

            // Click-to-edit: clicking the name loads entry into text box
            let name_click = if editing {
                let car_for_edit = car.clone();
                view! {
                    span(
                        class="has-text-link",
                        style="cursor: pointer; text-decoration: underline;",
                        title="Click to edit",
                        on:click=move |_| {
                            crate::update(model, crate::Msg::EventMsg(Msg::EditEntry(car_for_edit.clone())));
                        },
                    ) { (name) }
                }
            } else {
                view! { span { (name) } }
            };

            view! {
                li {
                    div(class="field is-grouped is-grouped-multiline is-vcentered") {
                        div(class="control") { (car_tag) }
                        div(class="control") { (name_click) }
                    }
                    div(class="field is-grouped is-grouped-multiline is-vcentered") {
                        (class_display)
                    }
                    (if !vehicle.is_empty() || !desc.is_empty() || !shared.is_empty() {
                        let mut parts: Vec<String> = vec![];
                        if !vehicle.is_empty() {
                            parts.push(format!("Vehicle: {}", vehicle));
                        }
                        if !desc.is_empty() {
                            parts.push(format!("Desc: {}", desc));
                        }
                        if !shared.is_empty() {
                            parts.push(format!("Shared: {}", shared));
                        }
                        view! { p(class="help") { (parts.join("  ")) } }
                    } else {
                        view! {}
                    })
                    (delete_btn)
                }
            }
        })
        .collect();
    view! { ul(class="todo-list") { (items) } }
}

/// Publish status, grouped above the bottom action bar.
fn view_publish_status(model: crate::Model) -> View {
    let em = model.screens.setup;
    view! {
        (move || {
            if em.editing.get() {
                return view! {
                    p(class="help") {
                        "Save to finish editing; a published event's save re-syncs it to the room."
                    }
                };
            }
            let is_null = model.khana.event.with(|e| e.is_null());
            let is_demo = model.khana.event.with(|e| e.is_demo());
            let published = is_published(model);
            if is_null {
                view! { p(class="help") { "Create or open an event first." } }
            } else if is_demo {
                view! {
                    div(class="control") {
                        span(class="tag is-warning") { "Demo — local only, never published" }
                    }
                }
            } else if published {
                let alias = model
                    .khana
                    .event
                    .with(|e| e.space_alias().unwrap_or_default());
                view! {
                    div(class="field is-grouped") {
                        div(class="control") {
                            span(class="tag is-success") {
                                (if alias.is_empty() {
                                    "Published".to_string()
                                } else {
                                    format!("Published {alias}")
                                })
                            }
                        }
                        (if em.needs_sync.get() {
                            view! {
                                div(class="control") {
                                    span(class="tag is-danger") { "unsynced — edit & save to re-sync" }
                                }
                            }
                        } else {
                            view! {}
                        })
                    }
                    p(class="help") {
                        "Published — edits re-sync to the room; entrants and results are amended (no deletion)."
                    }
                }
            } else {
                view! {
                    p(class="help") {
                        "Publish to Matrix to create the event's rooms and push the setup (including the entrant list) into them."
                    }
                }
            }
        })
    }
}

/// The publish status message as a colored notification: danger on failure,
/// success on "Published", info otherwise — prominent, not a tiny help line.
fn view_publish_message(model: crate::Model) -> View {
    let em = model.screens.setup;
    view! {
        (move || {
            let Some(msg) = em.publish_status.get_clone() else {
                return view! {};
            };
            let is_error = msg.starts_with("Publish failed")
                || msg.starts_with("Can't publish")
                || msg.starts_with("Rooms created");
            let is_success = msg.starts_with("Published");
            let class = if is_error {
                "notification is-danger is-light p-2"
            } else if is_success {
                "notification is-success is-light p-2"
            } else {
                "notification is-info is-light p-2"
            };
            view! {
                div(class=class) {
                    span(class="icon") {
                        i(class=if is_error { "fa fa-triangle-exclamation" } else if is_success { "fa fa-circle-check" } else { "fa fa-info" })
                    }
                    span { (msg) }
                }
            }
        })
    }
}

/// Bottom action bar: every action lives here.  Edit mode → Cancel + Save;
/// view mode → Edit + Publish, then (after a divider) Clone Event + Create New
/// event — the new-event actions are less related to the event being edited.
fn view_action_bar(model: crate::Model) -> View {
    let em = model.screens.setup;
    view! {
        div {
            (move || {
                if em.editing.get() {
                    view! {
                        div(class="field is-grouped") {
                            div(class="control") {
                                button(
                                    class="button is-light",
                                    on:click=move |_| crate::update(model, crate::Msg::EventMsg(Msg::DiscardBatch)),
                                ) {
                                    "Cancel"
                                }
                            }
                            div(class="control") {
                                button(
                                    class="button is-primary",
                                    on:click=move |_| crate::update(model, crate::Msg::EventMsg(Msg::SaveBatch)),
                                ) {
                                    (if is_published(model) {
                                        "Save and Publish"
                                    } else {
                                        "Save Local"
                                    })
                                }
                            }
                        }
                    }
                } else {
                    view! {
                        div(class="field is-grouped") {
                            (view_edit_btn(model))
                            (view_publish_btn(model))
                        }
                    }
                }
            })
            (move || {
                if !em.editing.get() {
                    view! {
                        div {
                            hr() {}
                            div(class="field is-grouped") {
                                (view_clone_btn(model))
                                (view_create_btn(model))
                            }
                        }
                    }
                } else {
                    view! {}
                }
            })
        }
    }
}

/// "Edit" — enter edit mode (view mode only).
fn view_edit_btn(model: crate::Model) -> View {
    view! {
        (move || {
            if !model.khana.event.with(|e| !e.is_null()) || model.screens.setup.editing.get() {
                return view! {};
            }
            view! {
                button(
                    class="button is-light",
                    on:click=move |_| crate::update(model, crate::Msg::EventMsg(Msg::ToggleEdit)),
                ) {
                    "Edit"
                }
            }
        })
    }
}

/// "Clone Event" — copy the opened event into a fresh editable draft.
fn view_clone_btn(model: crate::Model) -> View {
    view! {
        (move || {
            if !model.khana.event.with(|e| !e.is_null()) || model.screens.setup.editing.get() {
                return view! {};
            }
            view! {
                button(
                    class="button is-light",
                    title="Clone this event into a fresh editable draft (entrants + tests included)",
                    on:click=move |_| crate::update(model, crate::Msg::EventMsg(Msg::CopyAsNew)),
                ) {
                    "Clone Event"
                }
            }
        })
    }
}

/// "Create New event" — a fresh editable draft (view mode only).
fn view_create_btn(model: crate::Model) -> View {
    view! {
        (move || {
            if model.screens.setup.editing.get() {
                return view! {};
            }
            view! {
                button(
                    class="button is-primary",
                    on:click=move |_| crate::update(model, crate::Msg::EventMsg(Msg::CreateDraft)),
                ) {
                    "Create New event"
                }
            }
        })
    }
}

/// "Publish" — first publish of a saved, non-demo draft (view mode only).
/// Hidden once the event has room ids (published).  Disabled until a homeserver
/// is selected, so "no homeserver" never surfaces as a late error.
fn view_publish_btn(model: crate::Model) -> View {
    view! {
        (move || {
            let em = model.screens.setup;
            if em.editing.get() {
                return view! {};
            }
            let (is_null, is_demo, published, has_hs) = model.khana.event.with(|e| {
                (e.is_null(), e.is_demo(), e.is_published(), !e.event_homeservers.is_empty())
            });
            if is_null || is_demo || published {
                return view! {};
            }
            view! {
                button(
                    class="button is-link",
                    disabled=!has_hs,
                    title=if has_hs { "" } else { "Select a homeserver in Edit first" },
                    on:click=move |_| crate::update(model, crate::Msg::EventMsg(Msg::Publish)),
                ) {
                    span(class="icon is-small") { i(class="fa fa-paper-plane") }
                    span { "Publish" }
                }
            }
        })
    }
}

/// Publish-to-Matrix config, in the details: pick a saved homeserver from the
/// login list (reg mode follows the picked account).
fn view_homeserver_fields(model: crate::Model) -> View {
    let _em = model.screens.setup;
    view! {
        div(class="field") {
            label(class="label") { "Publish to homeserver" }
            (view_saved_hs_checklist(model))
        }
    }
}

/// Saved-login homeserver picker as toggleable tag-style buttons (wasm only:
/// reads stored sessions).  `edit_event.event_homeservers` is the source of truth.
fn view_saved_hs_checklist(model: crate::Model) -> View {
    let em = model.screens.setup;
    let editing = em.editing.get();
    let published = is_published(model);
    let is_demo = model.khana.event.with(|e| e.is_demo());
    let locked = published || is_demo;
    #[cfg(target_arch = "wasm32")]
    {
        let sessions = crate::services::matrix::load_sessions();

        if locked || !editing {
            let hs_list = model.khana.event.with(|e| e.event_homeservers.clone());
            let locked_reason = if is_demo {
                "Demo events are local-only and cannot be published."
            } else if published {
                "Homeservers cannot be changed after publishing."
            } else {
                "Enter Edit mode to change the homeservers."
            };
            return if hs_list.is_empty() {
                view! {
                    div(class="field") {
                        label(class="label is-small") { "Publish to homeserver" }
                        p(class="help") {
                            "No homeserver selected. " (locked_reason)
                        }
                    }
                }
            } else {
                let label = hs_list
                    .iter()
                    .map(|h| crate::page::home::hs_host_port(h))
                    .collect::<Vec<_>>()
                    .join(", ");
                view! {
                    div(class="field") {
                        label(class="label is-small") { "Publish to homeserver" }
                        div(class="control") {
                            input(
                                class="input is-small",
                                disabled=true,
                                value=label,
                                title=locked_reason,
                            )
                        }
                        p(class="help") { (locked_reason) }
                    }
                }
            };
        }

        // Editable: show tag-style toggle buttons.
        let buttons: Vec<View> = sessions
            .into_iter()
            .map(|s| {
                view! {
                    (move || {
                        let hs = s.homeserver.clone();
                        let on = em.edit_event.with(|e| e.as_ref().map(|e| e.event_homeservers.contains(&hs)).unwrap_or(false));
                        let label = crate::page::home::hs_host_port(&hs);
                        view! {
                            button(
                                class=format!(
                                    "button is-small kt-hs-tag {}",
                                    if on { "is-primary is-selected" } else { "is-light" }
                                ),
                                title=if on {
                                    "Selected — click to remove"
                                } else {
                                    "Publish to this homeserver"
                                },
                                on:click=move |_| {
                                    em.edit_event.update(|e| {
                                        if let Some(ref mut ev) = e {
                                            if on {
                                                ev.event_homeservers.retain(|h| h != &hs);
                                            } else {
                                                ev.event_homeservers.push(hs.clone());
                                            }
                                        }
                                    });
                                },
                            ) {
                                span(class="icon is-small") {
                                    i(class=if on { "fa fa-square-check" } else { "fa fa-square" })
                                }
                                span { (label) }
                            }
                        }
                    })
                }
            })
            .collect();
        let hs_list = em.edit_event.with(|e| {
            e.as_ref()
                .map(|e| e.event_homeservers.clone())
                .unwrap_or_default()
        });
        view! {
            div(class="field") {
                label(class="label is-small") { "From your logins" }
                div(class="kt-hs-tags") {
                    (buttons)
                }
                (if hs_list.is_empty() {
                    view! {
                        p(class="help") {
                            "No homeserver selected — pick one above to enable Publish (or leave offline for a local-only event)."
                        }
                    }
                } else {
                    let h = hs_list.iter().map(|h| crate::page::home::hs_host_port(h)).collect::<Vec<_>>().join(", ");
                    view! { p(class="help") { "Publish to: " (h) " — click a tag again to remove." } }
                })
                p(class="help") { "Homeservers are added on the Home page." }
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (model, editing, published, locked, is_demo);
        view! {}
    }
}

/// Per-test config: name, number, total runs, scored runs, timing style.
fn view_stage_list(model: crate::Model) -> View {
    let em = model.screens.setup;
    let editing = em.editing.get();
    let _ = em.edit_rev.get();
    // Untracked: per-keystroke field edits must not rebuild (and so not lose
    // focus on) the inputs.  Structural changes bump `edit_rev` instead.
    let stages = if editing {
        untrack(|| {
            em.edit_event
                .with(|e| e.as_ref().map(|e| e.stages.clone()).unwrap_or_default())
        })
    } else {
        model.khana.event.with(|e| e.stages.clone())
    };
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
                        div(class="column is-2") { strong { "Total runs" } }
                        div(class="column is-2") { strong { "Scored runs" } }
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

fn view_stage_row(model: crate::Model, idx: usize, stage: &Stage) -> View {
    let em = model.screens.setup;
    let num = stage.num.to_string();
    let name = stage.name.clone();
    let runs_total = stage.runs_total.to_string();
    let runs_scored = stage.runs_scored.to_string();
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
                        em.edit_event.update(|e| { if let Some(ref mut ev) = e { if let Some(s) = ev.stages.get_mut(idx) { s.num = v; } } });
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
                        em.edit_event.update(|e| { if let Some(ref mut ev) = e { if let Some(s) = ev.stages.get_mut(idx) { s.name = v; } } });
                    },
                )
            }
            div(class="column is-2") {
                input(
                    class="input",
                    r#type="number",
                    min="0",
                    value=runs_total,
                    on:input=move |ev: web_sys::Event| {
                        let v = input_value(&ev).trim().parse::<u8>().unwrap_or(1);
                        em.edit_event.update(|e| { if let Some(ref mut ev) = e { if let Some(s) = ev.stages.get_mut(idx) {
                            s.runs_total = v;
                            if s.runs_scored > v { s.runs_scored = v; }
                        } } });
                    },
                )
            }
            div(class="column is-2") {
                input(
                    class="input",
                    r#type="number",
                    min="0",
                    value=runs_scored,
                    on:input=move |ev: web_sys::Event| {
                        let v = input_value(&ev).trim().parse::<u8>().unwrap_or(1);
                        em.edit_event.update(|e| { if let Some(ref mut ev) = e { if let Some(s) = ev.stages.get_mut(idx) {
                            s.runs_scored = v.min(s.runs_total);
                        } } });
                    },
                )
            }
            div(class="column is-2") {
                (view_timing_buttons(model, idx))
            }
            div(class="column is-1") {
                (if is_published(model) {
                    view! {}
                } else {
                    view! {
                        button(
                            class="delete is-danger",
                            title="Remove test",
                            on:click=move |_| crate::update(model, crate::Msg::EventMsg(Msg::StageDelete(idx))),
                        )
                    }
                })
            }
        }
    }
}

fn view_stage_row_readonly(stage: &Stage) -> View {
    let num = stage.num.to_string();
    let name = stage.name.clone();
    let runs_total = stage.runs_total.to_string();
    let runs_scored = stage.runs_scored.to_string();
    let timing_label = match stage.timing {
        TimingStyle::Stopwatch => "Stopwatch",
        TimingStyle::Rally => "Rally",
    };
    view! {
        div(class="columns is-vcentered") {
            div(class="column is-1") { span(class="tag is-light") { (num) } }
            div(class="column is-4") { (name) }
            div(class="column is-2") { (runs_total) }
            div(class="column is-2") { (runs_scored) }
            div(class="column is-3") { span(class="tag is-light") { (timing_label) } }
        }
    }
}

fn view_timing_buttons(model: crate::Model, idx: usize) -> View {
    let em = model.screens.setup;
    view! {
        div(class="buttons has-addons") {
            (move || {
                let on = untrack(|| em.edit_event.with(|e| e.as_ref().and_then(|ev| ev.stages.get(idx)).map(|s| s.timing == TimingStyle::Stopwatch).unwrap_or(false)));
                view! {
                    button(
                        class=format!("button is-small {}", if on { "is-primary is-selected" } else { "is-light" }),
                        on:click=move |_| {
                            em.edit_event.update(|e| { if let Some(ref mut ev) = e { if let Some(s) = ev.stages.get_mut(idx) { s.timing = TimingStyle::Stopwatch; } } });
                            em.edit_rev.set(em.edit_rev.get().wrapping_add(1));
                        },
                    ) { "Stopwatch" }
                }
            })
            (move || {
                let on = untrack(|| em.edit_event.with(|e| e.as_ref().and_then(|ev| ev.stages.get(idx)).map(|s| s.timing == TimingStyle::Rally).unwrap_or(false)));
                view! {
                    button(
                        class=format!("button is-small {}", if on { "is-primary is-selected" } else { "is-light" }),
                        on:click=move |_| {
                            em.edit_event.update(|e| { if let Some(ref mut ev) = e { if let Some(s) = ev.stages.get_mut(idx) { s.timing = TimingStyle::Rally; } } });
                            em.edit_rev.set(em.edit_rev.get().wrapping_add(1));
                        },
                    ) { "Rally" }
                }
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Quick-add entrant parsing
// ---------------------------------------------------------------------------

/// Capitalize first letter of each word, lowercase the rest.
/// A trailing `_` disables capitalisation for that word (the `_` is stripped).
fn title_case(s: &str) -> String {
    s.split_whitespace()
        .map(|word| {
            if let Some(raw) = word.strip_suffix('_') {
                return raw.to_string();
            }
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    let rest: String = chars.collect();
                    format!("{}{}", first.to_uppercase(), rest.to_lowercase())
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Format vehicle text: words with ≤3 alpha chars → UPPERCASE, longer → title case.
/// A trailing `_` disables formatting for that word (the `_` is stripped).
fn format_vehicle(s: &str) -> String {
    s.split_whitespace()
        .map(|w| {
            if let Some(raw) = w.strip_suffix('_') {
                return raw.to_string();
            }
            let alpha_count = w.chars().filter(|c| c.is_ascii_alphabetic()).count();
            if alpha_count <= 3 {
                w.to_uppercase()
            } else {
                title_case(w)
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Resolve a class token against the event's class list.
/// Exact match (case-insensitive) first, then prefix match (shortest if ambiguous).
fn resolve_class(token: &str, event_classes: &[String]) -> Result<String, String> {
    if let Some(c) = event_classes.iter().find(|c| c.eq_ignore_ascii_case(token)) {
        return Ok(c.clone());
    }
    let lower = token.to_lowercase();
    let mut matches: Vec<&String> = event_classes
        .iter()
        .filter(|c| c.to_lowercase().starts_with(&lower))
        .collect();
    match matches.len() {
        0 => Err(format!(
            "Unknown class '{}' (available: {})",
            token,
            event_classes.join(", ")
        )),
        1 => Ok(matches.remove(0).clone()),
        _ => {
            matches.sort_by_key(|c| c.len());
            Ok(matches.remove(0).clone())
        }
    }
}

/// Result of parsing a quick-add entrant line.
pub struct QuickParse {
    pub entry: crate::event::Entry,
    /// Field index where the next character will go.
    /// 0=Car/Name, 1=Classes, 2=Vehicle, 3=Description, 4=Shared
    pub cursor_field: usize,
    /// Which fields had magic defaults applied (for ⚡ display).
    pub defaulted: Vec<&'static str>,
    pub extra_warning: Option<String>,
}

/// Parse a quick-add entrant line into a QuickParse.
///
/// Format: `Number Name  Class1 Class2  Vehicle  Description  SharedCarGroup`
/// Spaces are meaningful:
/// - Single space: within a field (e.g. "John Smith")
/// - Double-space: move to next field
/// - Triple+-space: skip next field(s) — each extra space beyond 2 skips one more.
///
/// Defaults for skipped fields:
/// - Car: auto-assign
/// - Description: copy from Vehicle
///
/// Returns `cursor_field` indicating where the next character will go.
fn parse_quick_entry(input: &str, event: &crate::event::EventInfo) -> Result<QuickParse, String> {
    // Check for trailing spaces BEFORE any processing — these determine cursor field.
    let trailing_space_count = input.len() - input.trim_end().len();
    let ends_with_space = trailing_space_count > 0;
    let input = input.trim_start();

    // --- Phase 1: scan the raw input to find tokens and their field positions ---
    // Field positions: 0=Car/Name, 1=Classes, 2=Vehicle, 3=Description, 4=Shared
    struct Token<'a> {
        field: usize,
        text: &'a str,
    }
    let mut tokens: Vec<Token<'_>> = vec![];
    let mut field_pos = 0;
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] == b' ' {
            // Count consecutive spaces
            let start = i;
            while i < len && bytes[i] == b' ' {
                i += 1;
            }
            let space_count = i - start;
            if space_count >= 2 {
                // Separator: advance field position
                field_pos += space_count - 1;
            }
            // Single space within field: no advancement
        } else {
            // Start of a token
            let tok_start = i;
            while i < len && bytes[i] != b' ' {
                i += 1;
            }
            let tok_text = &input[tok_start..i];
            if !tok_text.is_empty() {
                tokens.push(Token {
                    field: field_pos,
                    text: tok_text,
                });
            }
        }
    }

    // --- Phase 2: determine cursor field (deferred to after Phase 3) ---
    // We need to know if the last field-0 token was a class or name,
    // which requires class matching. Cursor field is computed below.

    // --- Phase 3: process tokens into fields ---
    if tokens.is_empty() {
        return Err(String::new());
    }

    // Collect field-0 tokens (car/name/classes before any double-space)
    let field0_tokens: Vec<&str> = tokens
        .iter()
        .filter(|t| t.field == 0)
        .map(|t| t.text)
        .collect();

    // Parse car number from the start of field 0
    let first_joined = field0_tokens.join(" ");
    let mut car_end = 0;
    let chars: Vec<char> = first_joined.chars().collect();
    while car_end < chars.len() && chars[car_end].is_ascii_digit() {
        car_end += 1;
    }
    if car_end > 0 {
        while car_end < chars.len() && chars[car_end].is_ascii_alphabetic() {
            car_end += 1;
        }
    }

    let (car, after_car) = if car_end > 0 && car_end < chars.len() && chars[car_end] == ' ' {
        let car = crate::event::normalize_car_number(&first_joined[..car_end])?;
        (car, first_joined[car_end..].trim().to_string())
    } else if car_end > 0 && car_end == first_joined.len() {
        let car = crate::event::normalize_car_number(&first_joined)?;
        (car, String::new())
    } else {
        (String::new(), first_joined.clone())
    };

    // Smart detection: split remaining field-0 words into name vs classes
    let mut name_words: Vec<String> = vec![];
    let mut field0_classes: Vec<String> = vec![];

    if !after_car.is_empty() {
        for word in after_car.split_whitespace() {
            if let Ok(resolved) = resolve_class(word, &event.classes) {
                field0_classes.push(resolved);
            } else {
                name_words.push(title_case(word));
            }
        }
    }

    let name = name_words.join(" ");

    if name.is_empty() && car.is_empty() {
        return Err(String::new());
    }

    if !car.is_empty() && event.entries.iter().any(|e| e.car == car) {
        return Err(format!("Car number {car} already exists."));
    }
    if !name.is_empty() && event.entries.iter().any(|e| e.name == name) {
        return Err(format!("Driver '{name}' already exists."));
    }

    // Process remaining tokens by field (field 1+)
    let mut vehicle_raw = String::new();
    let mut description_raw = String::new();
    let mut shared_raw = String::new();
    let mut extra_warning = None;

    for tok in &tokens {
        if tok.field == 0 {
            continue; // already processed
        }
        match tok.field {
            1 => {
                // Classes from double-space separated field
                if let Ok(resolved) = resolve_class(tok.text, &event.classes) {
                    field0_classes.push(resolved);
                } else {
                    extra_warning = Some(format!("Unknown class '{}'", tok.text));
                }
            }
            2 => {
                if !vehicle_raw.is_empty() {
                    vehicle_raw.push(' ');
                }
                vehicle_raw.push_str(tok.text);
            }
            3 => {
                if !description_raw.is_empty() {
                    description_raw.push(' ');
                }
                description_raw.push_str(tok.text);
            }
            4 => {
                if !shared_raw.is_empty() {
                    shared_raw.push(' ');
                }
                shared_raw.push_str(tok.text);
            }
            _ => {
                extra_warning = Some(format!("Extra text ignored: \"{}\"", tok.text));
            }
        }
    }

    // --- Cursor field computation ---
    // Determine the field of the last token for cursor position.
    let last_token_field = if let Some(last) = tokens.last() {
        if last.field == 0 {
            // Field 0: check if last word was a class via smart detection
            let last_field0_word = field0_tokens.last().copied().unwrap_or("");
            if resolve_class(last_field0_word, &event.classes).is_ok() {
                1 // class
            } else {
                0 // name
            }
        } else {
            last.field
        }
    } else {
        0
    };

    // Cursor field: where the next character will go.
    // Trailing spaces: each space beyond 1 advances one field.
    // 1 space = stay on current (with special cases), 2 = next, 3 = skip 1, etc.
    // Special cases:
    // - After first class (field0_classes <= 1) with single space, stay on classes
    // - After vehicle with single space, stay on vehicle
    let cursor_field = if ends_with_space {
        let advance = trailing_space_count.saturating_sub(1);
        if last_token_field == 1 && field0_classes.len() <= 1 && trailing_space_count < 2 {
            1 // stay on classes for more
        } else if last_token_field == 2 && trailing_space_count < 2 {
            2 // stay on vehicle for more text (single space)
        } else {
            (last_token_field + advance).min(4)
        }
    } else {
        last_token_field.min(4)
    };

    // --- Phase 4: apply defaults for skipped fields ---
    let mut defaulted = Vec::new();

    // Vehicle: no default, just empty
    let vehicle = if vehicle_raw.is_empty() {
        String::new()
    } else {
        format_vehicle(&vehicle_raw)
    };

    // Description: default is copy from Vehicle if skipped
    let description = if description_raw.is_empty() {
        if !vehicle.is_empty() && cursor_field > 3 {
            // Description was skipped — copy from Vehicle
            defaulted.push("Description");
            Some(vehicle.clone())
        } else {
            None
        }
    } else {
        Some(description_raw)
    };

    // Shared: no default, just empty
    let shared = if shared_raw.is_empty() {
        None
    } else {
        Some(shared_raw)
    };

    // Car: default is auto-assign if no number was typed
    let car = if car.is_empty() {
        defaulted.push("Car");
        String::new() // auto-assigned later in DoThing handler
    } else {
        car
    };

    Ok(QuickParse {
        entry: crate::event::Entry {
            car,
            name,
            vehicle: if vehicle.is_empty() {
                None
            } else {
                Some(vehicle)
            },
            description,
            shared,
            classes: field0_classes,
            passenger: None,
        },
        cursor_field,
        defaulted,
        extra_warning,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::EventInfo;

    fn test_event() -> EventInfo {
        let mut e = EventInfo {
            classes: vec![
                "Outright".into(),
                "Female".into(),
                "Junior".into(),
                "Provisional".into(),
            ],
            ..Default::default()
        };
        // Add an existing entry for shared-car lookup tests.
        let mut existing = crate::event::Entry::new("99", "Existing Driver");
        existing.shared = Some("My Shared Car".into());
        e.entries.push(existing);
        e
    }

    #[test]
    fn title_case_basic() {
        assert_eq!(title_case("john"), "John");
        assert_eq!(title_case("JOHN"), "John");
        assert_eq!(title_case(""), "");
    }

    #[test]
    fn format_vehicle_short_words_uppercase() {
        assert_eq!(format_vehicle("gr yaris"), "GR Yaris");
        assert_eq!(format_vehicle("RX 8"), "RX 8");
    }

    #[test]
    fn format_vehicle_long_words_title_case() {
        assert_eq!(format_vehicle("toyota"), "Toyota");
        assert_eq!(format_vehicle("focus rs"), "Focus RS");
    }

    #[test]
    fn format_vehicle_mixed() {
        assert_eq!(format_vehicle("mazda rx8"), "Mazda RX8");
        assert_eq!(format_vehicle("ford focus rs"), "Ford Focus RS");
    }

    #[test]
    fn resolve_class_exact() {
        let classes = vec!["Outright".into(), "Female".into()];
        assert_eq!(resolve_class("Outright", &classes).unwrap(), "Outright");
        assert_eq!(resolve_class("outright", &classes).unwrap(), "Outright");
    }

    #[test]
    fn resolve_class_prefix() {
        let classes = vec!["Outright".into(), "Junior".into(), "Female".into()];
        assert_eq!(resolve_class("Out", &classes).unwrap(), "Outright");
        assert_eq!(resolve_class("Ju", &classes).unwrap(), "Junior");
        assert_eq!(resolve_class("F", &classes).unwrap(), "Female");
    }

    #[test]
    fn resolve_class_prefix_ambiguous_picks_shortest() {
        let classes = vec!["Pro".into(), "Provisional".into()];
        assert_eq!(resolve_class("Pro", &classes).unwrap(), "Pro");
    }

    #[test]
    fn resolve_class_unknown_errors() {
        let classes = vec!["Outright".into()];
        assert!(resolve_class("XYZ", &classes).is_err());
    }

    #[test]
    fn parse_basic() {
        let ev = test_event();
        let qp = parse_quick_entry("123 John Smith  Outright", &ev).unwrap();
        assert_eq!(qp.entry.car, "123");
        assert_eq!(qp.entry.name, "John Smith");
        assert_eq!(qp.entry.classes, vec!["Outright"]);
        assert!(qp.entry.vehicle.is_none());
        assert!(qp.entry.shared.is_none());
        assert_eq!(qp.cursor_field, 1); // cursor on classes
    }

    #[test]
    fn parse_full() {
        let ev = test_event();
        let qp = parse_quick_entry(
            "7B Alice Wang  Outright Junior  Toyota GR Yaris  Rego ABC  Shared",
            &ev,
        )
        .unwrap();
        assert_eq!(qp.entry.car, "7B");
        assert_eq!(qp.entry.name, "Alice Wang");
        assert_eq!(qp.entry.classes, vec!["Outright", "Junior"]);
        assert_eq!(qp.entry.vehicle.as_deref(), Some("Toyota GR Yaris"));
        assert_eq!(qp.entry.description.as_deref(), Some("Rego ABC"));
        assert_eq!(qp.entry.shared.as_deref(), Some("Shared"));
    }

    #[test]
    fn parse_car_uppercased() {
        let ev = test_event();
        let qp = parse_quick_entry("7b alice  Outright", &ev).unwrap();
        assert_eq!(qp.entry.car, "7B");
    }

    #[test]
    fn parse_name_title_cased() {
        let ev = test_event();
        let qp = parse_quick_entry("1 john smith  Outright", &ev).unwrap();
        assert_eq!(qp.entry.name, "John Smith");
    }

    #[test]
    fn parse_name_only_no_car() {
        let ev = test_event();
        let qp = parse_quick_entry("John Smith  Outright", &ev).unwrap();
        assert!(qp.entry.car.is_empty());
        assert_eq!(qp.entry.name, "John Smith");
        assert_eq!(qp.entry.classes, vec!["Outright"]);
        assert!(qp.defaulted.contains(&"Car"));
    }

    #[test]
    fn parse_name_starting_with_word_no_car() {
        let ev = test_event();
        let qp = parse_quick_entry("Smith  Female", &ev).unwrap();
        assert!(qp.entry.car.is_empty());
        assert_eq!(qp.entry.name, "Smith");
    }

    #[test]
    fn parse_name_autocapitalise_disabled() {
        let ev = test_event();
        let qp = parse_quick_entry("1 john_ smith  Outright", &ev).unwrap();
        assert_eq!(qp.entry.name, "john Smith");
    }

    #[test]
    fn parse_class_abbreviation() {
        let ev = test_event();
        let qp = parse_quick_entry("1 x  Out Ju  car", &ev).unwrap();
        assert_eq!(qp.entry.classes, vec!["Outright", "Junior"]);
    }

    #[test]
    fn parse_vehicle_formatting() {
        let ev = test_event();
        let qp = parse_quick_entry("1 x  Outright  mazda rx8", &ev).unwrap();
        assert_eq!(qp.entry.vehicle.as_deref(), Some("Mazda RX8"));
    }

    #[test]
    fn parse_vehicle_autocapitalise_disabled() {
        let ev = test_event();
        let qp = parse_quick_entry("1 x  Outright  toyota gr_ yaris", &ev).unwrap();
        assert_eq!(qp.entry.vehicle.as_deref(), Some("Toyota gr Yaris"));
    }

    #[test]
    fn parse_description_and_shared() {
        let ev = test_event();
        let qp = parse_quick_entry("1 x  Outright  car  Rego  Group", &ev).unwrap();
        assert_eq!(qp.entry.description.as_deref(), Some("Rego"));
        assert_eq!(qp.entry.shared.as_deref(), Some("Group"));
    }

    #[test]
    fn parse_duplicate_car_errors() {
        let ev = test_event();
        assert!(parse_quick_entry("99 x  Outright", &ev).is_err());
    }

    #[test]
    fn parse_duplicate_name_errors() {
        let ev = test_event();
        assert!(parse_quick_entry("1 Existing Driver  Outright", &ev).is_err());
    }

    #[test]
    fn parse_name_only_gets_auto_assign() {
        let ev = test_event();
        let qp = parse_quick_entry("abc  Outright", &ev).unwrap();
        assert!(qp.entry.car.is_empty());
        assert_eq!(qp.entry.name, "Abc");
        assert_eq!(qp.entry.classes, vec!["Outright"]);
        assert!(qp.defaulted.contains(&"Car"));
    }

    #[test]
    fn parse_missing_name_succeeds_during_typing() {
        let ev = test_event();
        let qp = parse_quick_entry("123  Outright", &ev).unwrap();
        assert_eq!(qp.entry.car, "123");
        assert!(qp.entry.name.is_empty());
        assert_eq!(qp.entry.classes, vec!["Outright"]);
    }

    #[test]
    fn parse_missing_classes_succeeds_during_typing() {
        let ev = test_event();
        let qp = parse_quick_entry("123 John", &ev).unwrap();
        assert_eq!(qp.entry.car, "123");
        assert_eq!(qp.entry.name, "John");
        assert!(qp.entry.classes.is_empty());
    }

    #[test]
    fn parse_empty_input_errors() {
        let ev = test_event();
        assert!(parse_quick_entry("", &ev).is_err());
    }

    #[test]
    fn parse_unknown_class_shows_error() {
        let ev = test_event();
        let qp = parse_quick_entry("1 x  BadClass", &ev).unwrap();
        // Unknown class shows error but doesn't fail parsing
        assert!(qp.extra_warning.is_some());
        assert!(qp.extra_warning.unwrap().contains("BadClass"));
    }

    #[test]
    fn parse_optional_groups_missing() {
        let ev = test_event();
        let qp = parse_quick_entry("5 Bob  Female", &ev).unwrap();
        assert_eq!(qp.entry.car, "5");
        assert_eq!(qp.entry.name, "Bob");
        assert_eq!(qp.entry.classes, vec!["Female"]);
        assert!(qp.entry.vehicle.is_none());
        assert!(qp.entry.shared.is_none());
    }

    #[test]
    fn parse_extra_text_warning() {
        let ev = test_event();
        // Triple-space skips field 4 (shared), putting "extra" at field 5 = beyond max
        let qp = parse_quick_entry("1 x  Outright  car  desc   extra", &ev).unwrap();
        assert!(qp.extra_warning.is_some());
        assert!(qp.extra_warning.unwrap().contains("extra"));
    }

    // --- Cursor field tests ---
    #[test]
    fn cursor_on_classes_after_double_space() {
        let ev = test_event();
        let qp = parse_quick_entry("123 john  o", &ev).unwrap();
        assert_eq!(qp.cursor_field, 1); // classes
    }

    #[test]
    fn cursor_stays_on_classes_with_single_space() {
        let ev = test_event();
        let qp = parse_quick_entry("123 john  o ", &ev).unwrap();
        assert_eq!(qp.cursor_field, 1); // still classes
    }

    #[test]
    fn cursor_on_vehicle_after_double_space() {
        let ev = test_event();
        let qp = parse_quick_entry("123 john  o  fg", &ev).unwrap();
        assert_eq!(qp.cursor_field, 2); // vehicle
    }

    #[test]
    fn cursor_stays_on_vehicle_with_single_space() {
        let ev = test_event();
        let qp = parse_quick_entry("123 john  o  wrx ", &ev).unwrap();
        assert_eq!(qp.cursor_field, 2); // still vehicle
    }

    #[test]
    fn cursor_on_description_after_double_space() {
        let ev = test_event();
        let qp = parse_quick_entry("123 john  o  wrx  ", &ev).unwrap();
        assert_eq!(qp.cursor_field, 3); // description
    }

    #[test]
    fn cursor_skips_description_with_triple_space() {
        let ev = test_event();
        let qp = parse_quick_entry("123 john  o  wrx   ", &ev).unwrap();
        assert_eq!(qp.cursor_field, 4); // shared (skipped description)
        assert!(qp.defaulted.contains(&"Description"));
        assert_eq!(qp.entry.description.as_deref(), Some("WRX")); // copied from vehicle
    }

    #[test]
    fn cursor_on_shared_after_triple_space_with_value() {
        let ev = test_event();
        let qp = parse_quick_entry("123 john  o  wrx   b", &ev).unwrap();
        assert_eq!(qp.cursor_field, 4); // shared
        assert_eq!(qp.entry.shared.as_deref(), Some("b"));
    }
}
