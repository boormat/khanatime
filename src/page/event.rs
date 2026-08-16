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
    // event details editing
    LoadDetails,
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
    /// Set the event's publish homeserver.
    SetHomeserver(String),
    /// Set the event's registration mode.
    SetReg(crate::event::RegistrationMode),
}

#[derive(Clone, Copy)]
pub struct Model {
    pub class: InputModel,
    pub feedback: Signal<String>,
    /// Success confirmation (e.g. "Saved."), shown as an info line.
    pub saved: Signal<String>,
    pub editing: Signal<bool>,
    // ---- staged edit fields (applied on Save) ----
    pub edit_name: Signal<String>,
    pub edit_club: Signal<String>,
    pub edit_year: Signal<String>,
    pub edit_event_date: Signal<String>,
    pub edit_entry_open: Signal<String>,
    pub edit_entry_close: Signal<String>,
    pub edit_stripe: Signal<String>,
    pub edit_parent_room: Signal<String>,
    pub edit_entries_enabled: Signal<bool>,
    pub edit_homeserver: Signal<String>,
    pub edit_reg: Signal<crate::event::RegistrationMode>,
    pub edit_element_link: Signal<String>,
    pub edit_stages: Signal<Vec<Stage>>,
    /// Bumped when the stage list structure changes (add/remove) so the list
    /// re-renders, while per-keystroke field edits are untracked and don't.
    pub edit_rev: Signal<u8>,
    /// Staged class list (like `edit_stages`), applied on batch send.
    pub edit_classes: Signal<Vec<String>>,
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
    pub show_classes: Signal<bool>,
    pub show_entrants: Signal<bool>,
}

pub fn init() -> Model {
    let year = js_sys::Date::new_0().get_full_year().to_string();
    Model {
        class: crate::input::init(),
        feedback: create_signal(String::new()),
        saved: create_signal(String::new()),
        editing: create_signal(false),
        edit_name: create_signal(String::new()),
        edit_club: create_signal(String::new()),
        edit_year: create_signal(year),
        edit_event_date: create_signal(String::new()),
        edit_entry_open: create_signal(String::new()),
        edit_entry_close: create_signal(String::new()),
        edit_stripe: create_signal(String::new()),
        edit_parent_room: create_signal(String::new()),
        edit_entries_enabled: create_signal(false),
        edit_homeserver: create_signal(String::new()),
        edit_reg: create_signal(crate::event::RegistrationMode::default()),
        edit_element_link: create_signal(String::new()),
        edit_stages: create_signal(crate::event::EventInfo::default().stages),
        edit_rev: create_signal(0),
        edit_classes: create_signal(crate::event::EventInfo::default().classes),
        publish_status: create_signal(None),
        needs_sync: create_signal(false),
        confirm: create_signal(None),
        confirm_warning: create_signal(String::new()),
        pre_create: create_signal(None),
        edit_base: create_signal(None),
        show_tests: create_signal(true),
        show_classes: create_signal(true),
        show_entrants: create_signal(false),
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
            // Add a new class to the staged list (rename isn't supported).
            let input = model.screens.setup.class.input.get_clone();
            model.screens.setup.edit_classes.update(|v| {
                let trimmed = input.trim().to_string();
                if !trimmed.is_empty() && !v.contains(&trimmed) {
                    v.push(trimmed);
                }
            });
            input_clear(model.screens.setup.class);
        }

        Msg::DeleteClass(class) => {
            model
                .screens
                .setup
                .edit_classes
                .update(|v| v.retain(|c| c != &class));
        }
        Msg::CreateDraft => create_draft(model),
        Msg::CopyAsNew => copy_as_new(model),
        Msg::LoadDetails => load_details(model),
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
                em.editing.set(true);
                em.saved.set(String::new());
                load_details(model);
                if is_published(model) {
                    // Base the edit on the latest room state and remember the
                    // snapshot so remote updates made meanwhile are flagged.
                    em.edit_base.set(Some(model.app.event.get_clone()));
                    crate::sync::refresh_from_room(model);
                }
            }
        }
        Msg::StageAdd => {
            let em = model.screens.setup;
            em.edit_stages.update(|v| {
                // A new test duplicates the last test's settings.
                let last = v
                    .last()
                    .cloned()
                    .unwrap_or_else(|| crate::event::Stage::for_test(1));
                let num = v.iter().map(|s| s.num).max().unwrap_or(0) + 1;
                v.push(Stage {
                    num,
                    name: format!("Test {num}"),
                    repeats: last.repeats,
                    best_x: last.best_x,
                    timing: last.timing,
                });
            });
            em.edit_rev.set(em.edit_rev.get().wrapping_add(1));
        }
        Msg::StageDelete(idx) => {
            let num = model
                .screens
                .setup
                .edit_stages
                .with(|v| v.get(idx).map(|s| s.num));
            if let Some(num) = num {
                if crate::event::stage_has_timing(
                    &model.app.scores.get_clone(),
                    &model.app.runs.get_clone(),
                    num,
                ) {
                    model
                        .screens
                        .setup
                        .feedback
                        .set(format!("Test {num} has timing data — amend, don't delete."));
                    return;
                }
            }
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
        Msg::SetHomeserver(hs) => {
            let em = model.screens.setup;
            em.edit_homeserver.set(hs.trim().to_string());
            // Default the Element link for the chosen homeserver when unset.
            if em.edit_element_link.get_clone().trim().is_empty() {
                em.edit_element_link
                    .set(crate::event::element_link_default(&hs));
            }
        }
        Msg::SetReg(reg) => model.screens.setup.edit_reg.set(reg),
    }
}

/// Build the staged event: committed event + edit-form fields (details,
/// stages, classes).  Nothing is written to the event until the batch sends.
fn staged_event(model: crate::Model) -> crate::event::EventInfo {
    let em = model.screens.setup;
    let mut ev = model.app.event.get_clone();
    ev.name = em.edit_name.get_clone().trim().to_string();
    ev.sponsoring_club = em.edit_club.get_clone().trim().to_string();
    ev.year = em.edit_year.get_clone().trim().to_string();
    ev.event_date = em.edit_event_date.get_clone().trim().to_string();
    ev.entry_open = em.edit_entry_open.get_clone().trim().to_string();
    ev.entry_close = em.edit_entry_close.get_clone().trim().to_string();
    ev.stripe_link = em.edit_stripe.get_clone().trim().to_string();
    ev.parent_room = em.edit_parent_room.get_clone().trim().to_string();
    ev.entries_enabled = em.edit_entries_enabled.get();
    ev.homeserver = em.edit_homeserver.get_clone().trim().to_string();
    ev.reg = em.edit_reg.get();
    ev.element_link = em.edit_element_link.get_clone().trim().to_string();
    let mut stages = em.edit_stages.get_clone();
    // Display/ordering is by `num`; stable on ties.
    stages.sort_by_key(|s| s.num);
    for s in stages.iter_mut() {
        if s.best_x > s.repeats {
            s.best_x = s.repeats;
        }
    }
    ev.stages = stages;
    ev.classes = em.edit_classes.get_clone();
    ev
}

/// Compact (no-op) + diff the staged event against the committed one and open
/// the confirm modal.  A brand-new unsaved draft saves directly when there's
/// nothing to diff yet (the draft must still be persisted).
fn save_batch(model: crate::Model) {
    let em = model.screens.setup;
    let committed = model.app.event.get_clone();
    let staged = staged_event(model);
    let diff = crate::batch::event_diff(&committed, &staged);
    if diff.is_empty() {
        if em.pre_create.get_clone().is_some() {
            // Fresh draft with no edits yet: Save Local persists it as-is.
            em.feedback.set(String::new());
            send_batch(model);
        } else {
            em.feedback.set("No changes to save.".to_string());
        }
        return;
    }
    em.feedback.set(String::new());
    // Warn when a published event gained room updates since the edit started;
    // the staged event was built from the latest committed state, so they're
    // merged in best-effort.
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

/// Apply the staged event and enqueue a single setup manifest (the batch).
fn send_batch(model: crate::Model) {
    let em = model.screens.setup;
    let staged = staged_event(model);
    model.app.event.set(staged);
    commit_event(model);
    em.editing.set(false);
    em.confirm.set(None);
    em.confirm_warning.set(String::new());
    em.edit_base.set(None);
    em.pre_create.set(None);
    em.saved.set("Saved.".to_string());
    load_details(model);
}

/// Make `ev` the current event with the edit form open, without writing it to
/// the log — it stays a fresh draft until the user hits Save.  The caller
/// records `pre_create` first so Discard can restore the previous event.
fn switch_to_draft(model: crate::Model, ev: crate::event::EventInfo) {
    let id = ev.id.clone();
    model.app.event.set(ev);
    model.app.scores.set(Vec::new());
    model.app.runs.set(Vec::new());
    crate::event::session_set_event(&id);
    crate::event::session_set_recent(&id);
    model.screens.chat.expanded.set(Default::default());
    model.screens.entries.staged.set(Vec::new());
    model.screens.entries.confirm.set(None);
    model.screens.entries.admin.set(false);
    model.screens.entries.show_form.set(false);
    model.app.parcel_open_event.set(None);
    crate::app::refresh_feed(model);
    let em = model.screens.setup;
    em.editing.set(true);
    em.edit_base.set(None);
    load_details(model);
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
        .set(Some(model.app.event.with(|e| e.id.clone())));
    model.screens.setup.feedback.set(String::new());
    model.screens.setup.saved.set(String::new());
    switch_to_draft(model, e);
}

/// Copy the current event to a fresh draft: new name + id + uid, Matrix links
/// cleared, no timing data.  Entrants and tests are copied; entrant state is
/// reset for the new event.  The original stays untouched.
fn copy_as_new(model: crate::Model) {
    let em = model.screens.setup;
    let src = model.app.event.get_clone();
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
    e.space_alias = None;
    e.timing_id = None;
    e.timing_alias = None;
    // Entrants + tests are copied; entrant state is reset for the fresh event.
    for (i, entry) in e.entries.iter_mut().enumerate() {
        entry.entry_no = (i as u32) + 1;
        entry.status = crate::event::EntryStatus::Submitted;
        entry.order = 0;
    }
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
    em.confirm.set(None);
    em.confirm_warning.set(String::new());
    em.edit_base.set(None);
    let prev = em.pre_create.get_clone();
    em.pre_create.set(None);
    if let Some(prev) = prev {
        if prev.is_empty() {
            // No prior event: back to the no-event picker, on the Event page
            // so the create form is the natural next step.
            crate::update(model, crate::Msg::ClearEvent);
            crate::update(model, crate::Msg::Show(crate::Screen::Event));
        } else {
            crate::update(model, crate::Msg::SetEvent(prev));
        }
    } else {
        load_details(model);
    }
}

/// Refresh the detail-edit fields from the current event.
fn load_details(model: crate::Model) {
    let e = model.app.event.get_clone();
    model.screens.setup.edit_name.set(e.name.clone());
    model.screens.setup.edit_club.set(e.sponsoring_club.clone());
    model.screens.setup.edit_year.set(e.year.clone());
    model
        .screens
        .setup
        .edit_homeserver
        .set(e.homeserver.clone());
    model.screens.setup.edit_reg.set(e.reg);
    model
        .screens
        .setup
        .edit_element_link
        .set(e.element_link.clone());
    model
        .screens
        .setup
        .edit_parent_room
        .set(e.parent_room.clone());
    model
        .screens
        .setup
        .edit_entries_enabled
        .set(e.entries_enabled);
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
    model.screens.setup.edit_stages.set(e.stages.clone());
    model.screens.setup.edit_classes.set(e.classes.clone());
}

/// Publish the current event to a Matrix space + timing room using the
/// identity logged in on the Home page.
fn publish(model: crate::Model) {
    let em = model.screens.setup;
    let event = model.app.event.get_clone();
    let scores = model.app.scores.get_clone();
    let runs = model.app.runs.get_clone();
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
    let mut event = model.app.event.get_clone();
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
            model.app.event.set(event.clone());
            crate::app::enqueue_setup(model);
        }
        match (res, rooms_created) {
            (Ok(_), _) => {
                em.publish_status.set(Some("Published".to_string()));
                em.editing.set(false);
                load_details(model);
                // Join the fresh timing room so the setup (and entrants, which
                // ride in the manifest) flush into it.
                crate::sync::join_current_event(model);
            }
            (Err(e), true) => {
                em.publish_status.set(Some(format!(
                    "Rooms created, but setup sync wasn't confirmed ({e}). The event is marked published — use \"Save and Publish\" to finish syncing."
                )));
                em.editing.set(false);
                load_details(model);
                crate::sync::join_current_event(model);
            }
            (Err(e), false) => {
                em.publish_status.set(Some(format!(
                    "Publish failed: {e} — check you're signed in to {} on Home if the session expired.",
                    event.homeserver
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
            (view_details(model))
            (view_confirm_modal(model))
        }
    }
}

/// The event invite (QR + join URL), shown at the top of the config page once
/// the event is published and it isn't being edited.
fn view_invite(model: crate::Model) -> View {
    #[cfg(target_arch = "wasm32")]
    {
        let em = model.screens.setup;
        if em.editing.get() || !model.app.event.with(|e| e.is_published()) {
            return view! {};
        }
        let event = model.app.event.get_clone();
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
                    on:click=move |_| crate::page::copy_text(&element_c),
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
                                    on:click=move |_| crate::page::copy_text(&url_c),
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
    let invite = crate::event::Invite {
        homeserver: event.homeserver.clone(),
        event: event.id.clone(),
        sid,
        tid: event.timing_id.clone().unwrap_or_default(),
        reg: event.reg,
    };
    let app_base = {
        let window = web_sys::window()?;
        let origin = window.location().origin().ok()?;
        let path = window.location().pathname().ok()?;
        format!("{origin}{path}")
    };
    let url = invite.url(&app_base);
    let svg = crate::services::qr::qr_svg(&url, 320).unwrap_or_default();
    let base = if event.element_link.trim().is_empty() {
        crate::event::element_link_default(&event.homeserver)
    } else {
        event.element_link.clone()
    };
    let link = if base.is_empty() {
        String::new()
    } else {
        // Link directly to the timing room via its alias (#slug-timing:server).
        let slug = crate::event::build_event_id(&event.year, &event.sponsoring_club, &event.name);
        let server = crate::event::server_name_from_homeserver(&event.homeserver);
        if server.is_empty() {
            // Fallback to the space room id if the server name can't be derived.
            format!(
                "{}/#/room/{}",
                base.trim_end_matches('/'),
                event.space_id.clone().unwrap_or_default()
            )
        } else {
            format!(
                "{}/#/room/#{}-timing:{}",
                base.trim_end_matches('/'),
                slug,
                server,
            )
        }
    };
    Some((url, svg, link))
}

/// Header line: event id (edit mode) or name + id (view mode), with the
/// lifecycle status tag.  No action buttons up here — they live at the bottom.
fn view_header(model: crate::Model) -> View {
    view! {
        div(class="level") {
            div(class="level-left") {
                h1(class="title is-4") {
                    (move || {
                        let editing = model.screens.setup.editing.get();
                        let (id, name) = model.app.event.with(|e| (e.id.clone(), e.name.clone()));
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
                    if model.app.event.with(|e| e.is_null()) {
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
                    if model.app.event.with(|e| e.is_null()) {
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
        .app
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
                .app
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

/// True once an event has left the draft stage (published / running / finished).
fn is_published(model: crate::Model) -> bool {
    model
        .app
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
    // No event selected: keep the screen to a create prompt — the detail
    // fields, classes and tests are only meaningful once an event exists.
    if model.app.event.with(|e| e.is_null()) {
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
            (move || view_feedback(model))
            (move || {
                let msg = em.saved.get_clone();
                if msg.is_empty() {
                    view! {}
                } else {
                    view! { p(class="help is-success") { (msg) } }
                }
            })
            div(class="field") {
                label(class="label") { "Name" }
                div(class="control") {
                    input(class="input", placeholder="e.g. Khanacross Round 1", disabled=!editing, bind:value=em.edit_name)
                }
            }
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
                label(class="label") { "Parent room" }
                div(class="field has-addons") {
                    div(class="control is-expanded") {
                        input(class="input", placeholder="Optional — the club/organisation room this event links to", disabled=!editing, bind:value=em.edit_parent_room)
                    }
                    (if editing {
                        view! {
                            div(class="control") {
                                button(
                                    class="button is-light",
                                    title="Clear parent room",
                                    on:click=move |_| em.edit_parent_room.set(String::new()),
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
            div(class="field") {
                label(class="label") { "In-app entries" }
                (move || {
                    let on = em.edit_entries_enabled.get();
                    view! {
                        div(class="control") {
                            label(class="checkbox") {
                                input(
                                    r#type="checkbox",
                                    disabled=!em.editing.get(),
                                    checked=on,
                                    on:change=move |ev: web_sys::Event| {
                                        use wasm_bindgen::JsCast;
                                        let checked = ev
                                            .target()
                                            .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
                                            .map(|i| i.checked())
                                            .unwrap_or(false);
                                        em.edit_entries_enabled.set(checked);
                                    },
                                )
                                " Allow competitors to enter in the app"
                            }
                        }
                    }
                })
                p(class="help") { "Turn this off to close in-app self-entry (officials can still manage entries)." }
            }
            (view_homeserver_fields(model))
            (move || view_tests_section(model))
            (move || view_classes_section(model))
            (move || view_entrants_section(model))
            hr() {}
            (move || view_publish_status(model))
            (move || view_publish_message(model))
            (view_action_bar(model))
        }
    }
}

/// A collapsible section header: chevron + title + count, toggles `open`.
fn view_section_header(open: Signal<bool>, title: &'static str, count: usize) -> View {
    view! {
        button(
            class="button is-fullwidth is-light is-small",
            on:click=move |_| open.set(!open.get()),
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
    // Reactive count: staged stages during editing (tracks add/remove via
    // edit_rev), committed stages otherwise.
    let count = if editing {
        let _ = em.edit_rev.get();
        untrack(|| em.edit_stages.with(|s| s.len()))
    } else {
        model.app.event.with(|e| e.stage_count())
    };
    view! {
        div(class="field") {
            (view_section_header(em.show_tests, "Tests / stages", count))
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

/// Classes — collapsible part of the single event box.
fn view_classes_section(model: crate::Model) -> View {
    let em = model.screens.setup;
    let editing = em.editing.get();
    let count = model.app.event.with(|e| e.classes.len());
    view! {
        div(class="field") {
            (view_section_header(em.show_classes, "Classes", count))
            (move || {
                if !em.show_classes.get() {
                    return view! {};
                }
                view! {
                    div(class="mt-2") {
                        (move || view_class_list(model))
                        (move || {
                            if editing {
                                view! {
                                    div {
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
            })
        }
    }
}

/// Entrants — collapsible read-only list (management lives on the Entries
/// screen; the event only carries the final entrant list).
fn view_entrants_section(model: crate::Model) -> View {
    let em = model.screens.setup;
    let count = model.app.event.with(|e| e.entries.len());
    view! {
        div(class="field") {
            (view_section_header(em.show_entrants, "Entrants", count))
            (move || {
                if !em.show_entrants.get() {
                    return view! {};
                }
                view! {
                    div(class="mt-2") {
                        (move || view_entrant_list_readonly(model))
                        div(class="field") {
                            div(class="control") {
                                button(
                                    class="button is-small is-link",
                                    on:click=move |_| crate::update(model, crate::Msg::Show(crate::Screen::Entries)),
                                ) {
                                    span(class="icon is-small") { i(class="fa fa-users") }
                                    span { "Manage entries" }
                                }
                            }
                        }
                    }
                }
            })
        }
    }
}

/// Read-only entrant list: car number + name + status, in running order.
fn view_entrant_list_readonly(model: crate::Model) -> View {
    let entries = model.app.event.with(|e| {
        let mut v = e.entries.clone();
        v.sort_by_key(crate::event::entry_sort_key);
        v
    });
    if entries.is_empty() {
        return view! {
            p(class="help") { "No entrants yet — manage entries on the Entries screen." }
        };
    }
    let items: Vec<View> = entries
        .iter()
        .map(|e| {
            let car = e.car.clone();
            let name = e.name.clone();
            let status = e.status.to_string();
            let car_tag: View = if car.is_empty() {
                view! { span(class="tag is-light") { "?" } }
            } else {
                view! { span(class="tag is-black") { (car) } }
            };
            view! {
                li {
                    div(class="field is-grouped is-grouped-multiline is-vcentered") {
                        div(class="control") { (car_tag) }
                        div(class="control") { span { (name) } }
                        div(class="control") { span(class="tag is-light") { (status) } }
                    }
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
            let is_null = model.app.event.with(|e| e.is_null());
            let is_demo = model.app.event.with(|e| e.is_demo());
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
                    .app
                    .event
                    .with(|e| e.space_alias.clone())
                    .unwrap_or_default();
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
            if !model.app.event.with(|e| !e.is_null()) || model.screens.setup.editing.get() {
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
            if !model.app.event.with(|e| !e.is_null()) || model.screens.setup.editing.get() {
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
            let (is_null, is_demo, published, has_hs) = model.app.event.with(|e| {
                (e.is_null(), e.is_demo(), e.is_published(), !e.homeserver.trim().is_empty())
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
/// login list (reg mode follows the picked account) and set the Element link.
fn view_homeserver_fields(model: crate::Model) -> View {
    let em = model.screens.setup;
    let editing = em.editing.get();
    view! {
        div(class="field") {
            label(class="label") { "Publish to homeserver" }
            (view_saved_hs_checklist(model))
            div(class="field") {
                label(class="label") { "Element Web link" }
                div(class="control") {
                    input(class="input", placeholder="Optional — e.g. https://app.element.io", disabled=!editing, bind:value=em.edit_element_link)
                }
                p(class="help") {
                    "Optional link for officials/competitors to open the event's rooms in Element. Defaults to app.element.io for Matrix, localhost:8085 for a custom homeserver."
                }
            }
        }
    }
}

/// Saved-login homeserver picker as toggleable tag-style buttons (wasm only:
/// reads stored sessions).  `edit_homeserver` is the single source of truth —
/// the matching tag is filled with a check; clicking a selected tag again
/// clears it (back to offline), so there's always a way to unpick.
fn view_saved_hs_checklist(model: crate::Model) -> View {
    let em = model.screens.setup;
    let editing = em.editing.get();
    let published = is_published(model);
    #[cfg(target_arch = "wasm32")]
    {
        let sessions = crate::services::matrix::load_sessions();
        let buttons: Vec<View> = sessions
            .into_iter()
            .map(|s| {
                view! {
                    (move || {
                        let hs = s.homeserver.clone();
                        let on = em.edit_homeserver.get_clone() == hs;
                        let label = crate::page::home::hs_host_port(&hs);
                        view! {
                            button(
                                class=format!(
                                    "button is-small kt-hs-tag {}",
                                    if on { "is-primary is-selected" } else { "is-light" }
                                ),
                                disabled=!editing || published,
                                title=if on {
                                    "Selected — click to go offline (no homeserver)"
                                } else {
                                    "Publish to this homeserver"
                                },
                                on:click=move |_| {
                                    if on {
                                        crate::update(model, crate::Msg::EventMsg(Msg::SetHomeserver(String::new())));
                                    } else {
                                        let reg = crate::services::matrix::load_session_for(&hs)
                                            .map(|s| s.reg)
                                            .unwrap_or(crate::event::RegistrationMode::Sso);
                                        crate::update(model, crate::Msg::EventMsg(Msg::SetHomeserver(hs.clone())));
                                        crate::update(model, crate::Msg::EventMsg(Msg::SetReg(reg)));
                                    }
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
        view! {
            div(class="field") {
                label(class="label is-small") { "From your logins" }
                div(class="kt-hs-tags") {
                    (buttons)
                }
                (move || {
                    let hs = em.edit_homeserver.get_clone();
                    if hs.is_empty() {
                        view! {
                            p(class="help") {
                                "No homeserver selected — pick one above to enable Publish (or leave offline for a local-only event)."
                            }
                        }
                    } else {
                        view! { p(class="help") { "Publish to: " (hs) " — click the tag again to go offline." } }
                    }
                })
                p(class="help") { "Homeservers are added on the Home page." }
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (model, editing, published);
        view! {}
    }
}

/// Per-test config: name, number, repeats, best-X-of-Y, timing style.
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
                    min="0",
                    value=repeats,
                    on:input=move |ev: web_sys::Event| {
                        let v = input_value(&ev).trim().parse::<u8>().unwrap_or(1);
                        em.edit_stages.update(|st| if let Some(s) = st.get_mut(idx) {
                            s.repeats = v;
                            if s.best_x > v {
                                s.best_x = v;
                            }
                        });
                    },
                )
            }
            div(class="column is-2") {
                input(
                    class="input",
                    r#type="number",
                    min="0",
                    value=best_x,
                    on:input=move |ev: web_sys::Event| {
                        let v = input_value(&ev).trim().parse::<u8>().unwrap_or(1);
                        em.edit_stages.update(|st| if let Some(s) = st.get_mut(idx) {
                            s.best_x = v.min(s.repeats);
                        });
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
                // Untracked: re-reading the stage list here would rebuild the
                // row on every keystroke in another field and reset the input
                // values.  The button highlight is refreshed by `edit_rev`,
                // bumped by the click handler below.
                let on = untrack(|| em.edit_stages.with(|st| st.get(idx).map(|s| s.timing == TimingStyle::Stopwatch).unwrap_or(false)));
                view! {
                    button(
                        class=format!("button is-small {}", if on { "is-primary is-selected" } else { "is-light" }),
                        on:click=move |_| {
                            em.edit_stages.update(|st| if let Some(s) = st.get_mut(idx) { s.timing = TimingStyle::Stopwatch; });
                            em.edit_rev.set(em.edit_rev.get().wrapping_add(1));
                        },
                    ) { "Stopwatch" }
                }
            })
            (move || {
                let on = untrack(|| em.edit_stages.with(|st| st.get(idx).map(|s| s.timing == TimingStyle::Rally).unwrap_or(false)));
                view! {
                    button(
                        class=format!("button is-small {}", if on { "is-primary is-selected" } else { "is-light" }),
                        on:click=move |_| {
                            em.edit_stages.update(|st| if let Some(s) = st.get_mut(idx) { s.timing = TimingStyle::Rally; });
                            em.edit_rev.set(em.edit_rev.get().wrapping_add(1));
                        },
                    ) { "Rally" }
                }
            })
        }
    }
}

fn view_class_list(model: crate::Model) -> View {
    let em = model.screens.setup;
    let editing = em.editing.get();
    // Editing shows the staged class list; read-only shows the committed one.
    let classes = if editing {
        em.edit_classes.get_clone()
    } else {
        model.app.event.with(|event| event.classes.clone())
    };
    let items = classes
        .iter()
        .map(|class| {
            let class = class.clone();
            let class_disp = class.clone();
            let class_del = class.clone();
            view! {
                li {
                    span(class="tag is-medium") {
                        (class_disp)
                        (if editing {
                            view! {
                                button(
                                    class="delete is-danger",
                                    title="Remove class",
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
    view! { (items) }
}
