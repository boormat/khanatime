pub mod batch;
pub mod sync;
pub mod types;

use crate::input::{input_value, select_value};
use types::*;

use batch::{apply_ops, compact_ops, entry_diff, EditOp};
use std::collections::HashSet;
use sycamore::prelude::*;

#[derive(Clone)]
pub enum Msg {
    AdminToggle,
    ShowForm,
    CancelForm,
    SubmitEntry,
    WithdrawOwn(u32),
    SetStatus { entry_no: u32, status: EntryStatus },
    ToggleClass { entry_no: u32, class: String },
    SetCar { entry_no: u32, car: String },
    SetVehicle { entry_no: u32, vehicle: String },
    SetShared { entry_no: u32, shared: String },
    Move { entry_no: u32, up: bool },
    AssignNumbers,
    DeleteEntry(u32),
    SaveBatch,
    SendBatch,
    CancelBatch,
    DiscardBatch,
}

#[derive(Clone, Copy)]
pub struct Model {
    pub name: Signal<String>,
    pub preferred: Signal<String>,
    pub vehicle: Signal<String>,
    pub shared: Signal<String>,
    pub show_form: Signal<bool>,
    pub admin: Signal<bool>,
    pub staged: Signal<Vec<EditOp>>,
    pub confirm: Signal<Option<Vec<String>>>,
    pub feedback: Signal<String>,
}

pub fn init() -> Model {
    Model {
        name: create_signal(String::new()),
        preferred: create_signal(String::new()),
        vehicle: create_signal(String::new()),
        shared: create_signal(String::new()),
        show_form: create_signal(false),
        admin: create_signal(false),
        staged: create_signal(Vec::new()),
        confirm: create_signal(None),
        feedback: create_signal(String::new()),
    }
}

fn is_published(model: crate::Model) -> bool {
    model.entry_app.event.with(|e| e.is_published())
}

pub fn update(model: crate::Model, msg: Msg) {
    let em = model.screens.entry_app;
    match msg {
        Msg::AdminToggle => {
            let admin = !em.admin.get();
            em.admin.set(admin);
            if !admin {
                em.staged.set(Vec::new());
                em.confirm.set(None);
                em.feedback.set(String::new());
            }
        }
        Msg::ShowForm => em.show_form.set(true),
        Msg::CancelForm => {
            em.name.set(String::new());
            em.preferred.set(String::new());
            em.vehicle.set(String::new());
            em.shared.set(String::new());
            em.feedback.set(String::new());
            em.show_form.set(false);
        }
        Msg::SubmitEntry => competitor_submit(model),
        Msg::WithdrawOwn(entry_no) => {
            if let Some(mut entry) = model
                .entry_app
                .event
                .with(|e| e.find_entry(entry_no).cloned())
            {
                entry.status = EntryStatus::Withdrawn;
                sync::enqueue_entry(model, &entry, false);
            }
        }
        Msg::SetStatus { entry_no, status } => {
            staged_set(model, entry_no, |e| e.status = status.clone())
        }
        Msg::ToggleClass { entry_no, class } => staged_set(model, entry_no, |e| {
            if e.classes.contains(&class) {
                e.classes.retain(|x| x != &class);
            } else {
                e.classes.push(class.clone());
            }
        }),
        Msg::SetCar { entry_no, car } => set_car(model, entry_no, &car),
        Msg::SetVehicle { entry_no, vehicle } => {
            staged_set(model, entry_no, |e| e.vehicle = vehicle.trim().to_string())
        }
        Msg::SetShared { entry_no, shared } => staged_set(model, entry_no, |e| {
            let s = shared.trim();
            e.shared = if s.is_empty() {
                None
            } else {
                Some(s.to_string())
            };
        }),
        Msg::Move { entry_no, up } => staged_move(model, entry_no, up),
        Msg::AssignNumbers => assign_numbers(model),
        Msg::DeleteEntry(entry_no) => staged_delete(model, entry_no),
        Msg::SaveBatch => save_batch(model),
        Msg::SendBatch => send_batch(model),
        Msg::CancelBatch => em.confirm.set(None),
        Msg::DiscardBatch => {
            em.staged.set(Vec::new());
            em.confirm.set(None);
            em.feedback.set(String::new());
        }
    }
}

fn effective_entries(model: crate::Model) -> Vec<Entry> {
    let committed = model.entry_app.event.with(|e| e.entries.clone());
    apply_ops(&committed, &model.screens.entry_app.staged.get_clone())
}

fn staged_set(model: crate::Model, entry_no: u32, f: impl Fn(&mut Entry)) {
    let em = model.screens.entry_app;
    let mut entry = match effective_entries(model)
        .into_iter()
        .find(|e| e.entry_no == entry_no)
    {
        Some(e) => e,
        None => return,
    };
    f(&mut entry);
    em.staged.update(|v| v.push(EditOp::Upsert(entry)));
}

fn used_car_numbers(model: crate::Model, excluding: u32) -> HashSet<String> {
    effective_entries(model)
        .iter()
        .filter(|e| e.entry_no != excluding && !e.car.is_empty())
        .map(|e| e.car.clone())
        .collect()
}

fn set_car(model: crate::Model, entry_no: u32, raw: &str) {
    let em = model.screens.entry_app;
    let car = match normalize_car_number(raw) {
        Ok(c) => c,
        Err(e) => {
            em.feedback.set(e);
            return;
        }
    };
    let committed = model
        .entry_app
        .event
        .with(|e| e.find_entry(entry_no).cloned())
        .unwrap_or_else(|| Entry::new("", ""));
    if committed.car == car {
        return;
    }
    if !car.is_empty() && used_car_numbers(model, entry_no).contains(&car) {
        em.feedback
            .set(format!("Car number {car} is already assigned."));
        return;
    }
    em.feedback.set(String::new());
    staged_set(model, entry_no, |e| e.car = car.clone());
}

fn staged_delete(model: crate::Model, entry_no: u32) {
    let em = model.screens.entry_app;
    if is_published(model) {
        staged_set(model, entry_no, |e| e.status = EntryStatus::Withdrawn);
        return;
    }
    em.staged.update(|v| v.push(EditOp::Delete(entry_no)));
}

fn staged_move(model: crate::Model, entry_no: u32, up: bool) {
    let em = model.screens.entry_app;
    let mut sorted = effective_entries(model);
    sorted.sort_by_key(entry_sort_key);
    let Some(pos) = sorted.iter().position(|e| e.entry_no == entry_no) else {
        return;
    };
    let swap = if up {
        pos.checked_sub(1)
    } else {
        (pos + 1 < sorted.len()).then_some(pos + 1)
    };
    let Some(swap) = swap else {
        return;
    };
    sorted.swap(pos, swap);
    let with_order: Vec<Entry> = sorted
        .iter()
        .enumerate()
        .map(|(idx, e)| {
            let mut e2 = e.clone();
            e2.order = (idx as u32 + 1) * 10;
            e2
        })
        .collect();
    let committed = model.entry_app.event.with(|e| e.entries.clone());
    em.staged.update(|v| {
        for e in &with_order {
            let changed = committed
                .iter()
                .find(|c| c.entry_no == e.entry_no)
                .map(|c| c.order != e.order)
                .unwrap_or(true);
            if changed {
                v.push(EditOp::Upsert(e.clone()));
            }
        }
    });
}

fn assign_numbers(model: crate::Model) {
    let em = model.screens.entry_app;
    let mut sorted = effective_entries(model);
    sorted.sort_by_key(entry_sort_key);
    let mut used: HashSet<String> = sorted
        .iter()
        .map(|e| e.car.clone())
        .filter(|c| !c.is_empty())
        .collect();
    let mut changed_any = false;
    let mut collisions: Vec<String> = vec![];
    for (idx, e) in sorted.iter_mut().enumerate() {
        let active = matches!(
            e.status,
            EntryStatus::Submitted
                | EntryStatus::Accepted
                | EntryStatus::Confirmed
                | EntryStatus::Started
        );
        if !active {
            continue;
        }
        let mut changed = false;
        if e.order == 0 {
            e.order = (idx as u32 + 1) * 10;
            changed = true;
        }
        if e.car.is_empty() {
            let suggest = suggest_car_number(&used, &e.preferred_car);
            if !e.preferred_car.is_empty() && suggest != e.preferred_car {
                collisions.push(format!(
                    "{} wanted {}, got {}",
                    e.name, e.preferred_car, suggest
                ));
            }
            e.car = suggest;
            used.insert(e.car.clone());
            changed = true;
        }
        if changed {
            em.staged.update(|v| v.push(EditOp::Upsert(e.clone())));
            changed_any = true;
        }
    }
    if changed_any {
        em.feedback.set(if collisions.is_empty() {
            "Numbers (and order) suggested for unassigned entries.".to_string()
        } else {
            format!(
                "Suggested \u{2014} resolve collisions: {}",
                collisions.join("; ")
            )
        });
    } else {
        em.feedback
            .set("All active entries already have numbers.".to_string());
    }
}

fn competitor_submit(model: crate::Model) {
    let em = model.screens.entry_app;
    let identity = model.sync.identity.get_clone();
    if identity.is_empty() {
        em.feedback
            .set("Log in on the Home page to enter.".to_string());
        return;
    }
    let name = em.name.get_clone().trim().to_string();
    if name.is_empty() {
        em.feedback.set("Name is required.".to_string());
        return;
    }
    let preferred_raw = em.preferred.get_clone();
    let preferred = if preferred_raw.trim().is_empty() {
        String::new()
    } else {
        match normalize_car_number(&preferred_raw) {
            Ok(c) => c,
            Err(e) => {
                em.feedback.set(e);
                return;
            }
        }
    };
    let vehicle = em.vehicle.get_clone().trim().to_string();
    let shared_raw = em.shared.get_clone();
    let shared = {
        let s = shared_raw.trim();
        if s.is_empty() {
            None
        } else {
            Some(s.to_string())
        }
    };

    let existing = model.entry_app.event.with(|e| {
        e.entries
            .iter()
            .find(|e| {
                e.owner.as_deref() == Some(identity.as_str()) && e.status != EntryStatus::Withdrawn
            })
            .cloned()
    });
    let entry = match existing {
        Some(mut e) => {
            e.name = name;
            e.preferred_car = preferred;
            e.vehicle = vehicle;
            e.shared = shared;
            e
        }
        None => Entry {
            entry_no: 0,
            preferred_car: preferred,
            name,
            vehicle,
            shared,
            owner: Some(identity),
            ..Entry::new("", "")
        },
    };
    sync::enqueue_entry(model, &entry, false);
    em.name.set(String::new());
    em.preferred.set(String::new());
    em.vehicle.set(String::new());
    em.shared.set(String::new());
    em.feedback.set(String::new());
    em.show_form.set(false);
}

fn save_batch(model: crate::Model) {
    let em = model.screens.entry_app;
    let committed = model.entry_app.event.with(|e| e.entries.clone());
    let compacted = compact_ops(&em.staged.get_clone(), &committed);
    if compacted.is_empty() {
        em.feedback.set("No changes to send.".to_string());
        return;
    }
    em.feedback.set(String::new());
    em.staged.set(compacted.clone());
    em.confirm.set(Some(entry_diff(&compacted, &committed)));
}

fn send_batch(model: crate::Model) {
    let em = model.screens.entry_app;
    let ops = em.staged.get_clone();
    for op in ops {
        match op {
            EditOp::Upsert(entry) => sync::enqueue_entry(model, &entry, false),
            EditOp::Delete(no) => {
                let entry = model
                    .entry_app
                    .event
                    .with(|e| e.find_entry(no).cloned())
                    .unwrap_or_else(|| {
                        let mut e = Entry::new("", "");
                        e.entry_no = no;
                        e
                    });
                sync::enqueue_entry(model, &entry, true);
            }
        }
    }
    em.staged.set(Vec::new());
    em.confirm.set(None);
    em.feedback.set(String::new());
}

// --- Views ---

pub fn view(model: crate::Model) -> View {
    let names: Vec<String> = {
        let mut seen: Vec<String> = vec![];
        for e in effective_entries(model) {
            if let Some(n) = &e.shared {
                let trimmed = n.trim();
                if !trimmed.is_empty() && !seen.iter().any(|x| x == trimmed) {
                    seen.push(trimmed.to_string());
                }
            }
        }
        seen
    };
    let options: Vec<View> = names
        .iter()
        .map(|n| {
            let val = n.clone();
            view! { option(value=val) }
        })
        .collect();
    view! {
        div {
            datalist(id="ea-shared-names") { (options) }
            div(class="level") {
                div(class="level-left") {
                    h1(class="title is-4") { "Entries" }
                }
                div(class="level-right") {
                    (view_admin_toggle(model))
                }
            }
            (move || view_entrant_list(model))
            (move || view_enter_form(model))
            (move || view_batch_controls(model))
            (view_confirm_modal(model))
        }
    }
}

fn view_confirm_modal(model: crate::Model) -> View {
    let em = model.screens.entry_app;
    let no_warning = create_signal(String::new());
    crate::view::view_confirm_modal(
        em.confirm,
        || "Send".to_string(),
        move || crate::update(model, crate::Msg::EntryAppMsg(Msg::SendBatch)),
        move || crate::update(model, crate::Msg::EntryAppMsg(Msg::CancelBatch)),
        move || crate::update(model, crate::Msg::EntryAppMsg(Msg::DiscardBatch)),
        no_warning,
    )
}

fn view_admin_toggle(model: crate::Model) -> View {
    let em = model.screens.entry_app;
    view! {
        (move || {
            let admin = em.admin.get();
            view! {
                button(
                    class=format!("button {}", if admin { "is-link" } else { "is-light" }),
                    on:click=move |_| crate::update(model, crate::Msg::EntryAppMsg(Msg::AdminToggle)),
                ) {
                    (if admin { "View entries" } else { "Entry administration" })
                }
            }
        })
    }
}

fn view_entrant_list(model: crate::Model) -> View {
    let em = model.screens.entry_app;
    let admin = em.admin.get();
    let identity = model.sync.identity.get_clone();
    let classes = model.entry_app.event.with(|e| e.classes.clone());
    let mut entries = effective_entries(model);
    entries.sort_by_key(entry_sort_key);
    let shared = shared_entry_nos(&entries);
    let items: Vec<View> = entries
        .iter()
        .map(|e| view_entry(model, e, &classes, admin, &identity, &shared))
        .collect();
    view! {
        div(class="box") {
            h2(class="title is-5") { "Entry list" }
            ul(class="todo-list") { (items) }
        }
    }
}

fn view_entry(
    model: crate::Model,
    entry: &Entry,
    classes: &[String],
    admin: bool,
    identity: &str,
    shared_nos: &HashSet<u32>,
) -> View {
    let entry_no = entry.entry_no;
    let car = entry.car.clone();
    let preferred = entry.preferred_car.clone();
    let vehicle = entry.vehicle.clone();
    let shared = entry.shared.clone();
    let name = entry.name.clone();
    let status = entry.status.clone();
    let entry_classes = entry.classes.clone();
    let mine = !identity.is_empty() && entry.owner.as_deref() == Some(identity);
    let published = is_published(model);
    let status_value = status.as_str().to_string();
    let is_shared = shared_nos.contains(&entry_no);

    let status_options: Vec<View> = if admin {
        ENTRY_STATUSES
            .iter()
            .map(|(v, label)| view! { option(value=v.to_string()) { (label.to_string()) } })
            .collect()
    } else {
        vec![]
    };

    let class_checks: Vec<View> = if admin {
        classes
            .iter()
            .map(|cl| {
                let cl = cl.clone();
                let on = entry_classes.contains(&cl);
                let c1 = cl.clone();
                view! {
                    label(class="checkbox") {
                        input(
                            r#type="checkbox",
                            checked=on,
                            on:change=move |_| {
                                crate::update(model, crate::Msg::EntryAppMsg(Msg::ToggleClass {
                                    entry_no,
                                    class: c1.clone(),
                                }))
                            },
                        )
                        (cl.clone())
                    }
                }
            })
            .collect()
    } else {
        vec![]
    };

    let number_tag: View = {
        let car_tag = car.clone();
        let pref_tag = preferred.clone();
        if !car_tag.is_empty() {
            view! {
                span(class="tag is-black") {
                    i(class="fa fa-car", style="width: 20px")
                    (car_tag)
                }
            }
        } else if !pref_tag.is_empty() {
            view! {
                span(class="tag is-light", title="preferred \u{2014} not yet assigned") {
                    i(class="fa fa-pen", style="width: 20px")
                    (pref_tag)
                }
            }
        } else {
            view! {
                span(class="tag is-light") {
                    i(class="fa fa-car", style="width: 20px")
                    "?"
                }
            }
        }
    };

    let mine_tag: View = if mine {
        view! { span(class="tag is-primary is-light") { "mine" } }
    } else {
        view! {}
    };

    let prefers_hint: View = {
        let car_tag = car.clone();
        let pref_tag = preferred.clone();
        if car_tag.is_empty() && !pref_tag.is_empty() {
            view! { span(class="help is-inline-block") { "prefers " (pref_tag) } }
        } else {
            view! {}
        }
    };

    let shared_badge: View = {
        let shared_cl = shared.clone();
        match &shared_cl {
            Some(sh) => {
                let sh = sh.clone();
                let badge_class = if is_shared {
                    "tag is-warning"
                } else {
                    "tag is-light"
                };
                view! {
                    span(class=badge_class) {
                        i(class="fa fa-users", style="width: 18px")
                        (sh)
                    }
                }
            }
            None => view! {},
        }
    };

    let status_selector: View = {
        let status_val = status_value.clone();
        if admin {
            view! {
                select(
                    class="select is-small",
                    value=status_val,
                    on:change=move |ev| {
                        let value = select_value(&ev);
                        crate::update(model, crate::Msg::EntryAppMsg(Msg::SetStatus {
                            entry_no,
                            status: entry_status_from(&value),
                        }));
                    },
                ) { (status_options) }
            }
        } else {
            let status_str = status.to_string();
            view! { span(class="tag is-light") { (status_str) } }
        }
    };

    let admin_fields: View = {
        let car_val = car.clone();
        let vehicle_val = vehicle.clone();
        let shared_val = shared.clone().unwrap_or_default();
        if admin {
            view! {
                div(class="field is-grouped is-grouped-multiline") {
                    div(class="control") {
                        input(
                            class="input is-small",
                            placeholder="Assigned #",
                            value=car_val,
                            on:change=move |ev| {
                                let v = input_value(&ev);
                                crate::update(
                                    model,
                                    crate::Msg::EntryAppMsg(Msg::SetCar { entry_no, car: v }),
                                );
                            },
                        )
                    }
                    div(class="control") {
                        input(
                            class="input is-small",
                            placeholder="Vehicle",
                            value=vehicle_val,
                            on:change=move |ev| {
                                let v = input_value(&ev);
                                crate::update(
                                    model,
                                    crate::Msg::EntryAppMsg(Msg::SetVehicle { entry_no, vehicle: v }),
                                );
                            },
                        )
                    }
                    div(class="control") {
                        input(
                            class="input is-small",
                            placeholder="Shared car name",
                            list="ea-shared-names",
                            value=shared_val,
                            on:change=move |ev| {
                                let v = input_value(&ev);
                                crate::update(
                                    model,
                                    crate::Msg::EntryAppMsg(Msg::SetShared { entry_no, shared: v }),
                                );
                            },
                        )
                    }
                    div(class="control") {
                        div(class="field is-grouped") {
                            div(class="control") {
                                button(
                                    class="button is-small",
                                    title="Move up in running order",
                                    on:click=move |_| {
                                        crate::update(
                                            model,
                                            crate::Msg::EntryAppMsg(Msg::Move { entry_no, up: true }),
                                        )
                                    },
                                ) { i(class="fa fa-arrow-up") }
                            }
                            div(class="control") {
                                button(
                                    class="button is-small",
                                    title="Move down in running order",
                                    on:click=move |_| {
                                        crate::update(
                                            model,
                                            crate::Msg::EntryAppMsg(Msg::Move { entry_no, up: false }),
                                        )
                                    },
                                ) { i(class="fa fa-arrow-down") }
                            }
                        }
                    }
                }
            }
        } else {
            view! {}
        }
    };

    let admin_actions: View = {
        if admin {
            let entry_no_w = entry_no;
            if published {
                view! {
                    button(
                        class="button is-small is-warning",
                        title="Withdraw entry (no deletion after publish)",
                        on:click=move |_| {
                            crate::update(
                                model,
                                crate::Msg::EntryAppMsg(Msg::SetStatus {
                                    entry_no: entry_no_w,
                                    status: EntryStatus::Withdrawn,
                                }),
                            )
                        },
                    ) { "Withdraw" }
                }
            } else {
                view! {
                    button(
                        class="delete is-danger",
                        title="Remove entry",
                        on:click=move |_| {
                            crate::update(
                                model,
                                crate::Msg::EntryAppMsg(Msg::DeleteEntry(entry_no_w)),
                            )
                        },
                    )
                }
            }
        } else if mine && status != EntryStatus::Withdrawn {
            let entry_no_w = entry_no;
            view! {
                button(
                    class="button is-small is-warning",
                    on:click=move |_| {
                        crate::update(
                            model,
                            crate::Msg::EntryAppMsg(Msg::WithdrawOwn(entry_no_w)),
                        )
                    },
                ) { "Withdraw" }
            }
        } else {
            view! {}
        }
    };

    view! {
        li(class=if mine { "has-text-primary" } else { "" }) {
            div(class="field is-grouped is-grouped-multiline is-vcentered") {
                div(class="control") { (number_tag) }
                div(class="control") {
                    span(style="margin: 0 10px") { (name) }
                }
            }
            (mine_tag)
            (prefers_hint)
            (shared_badge)
            div(class="field is-grouped is-grouped-multiline") {
                div(class="control") { (status_selector) }
                (class_checks)
            }
            (admin_fields)
            (admin_actions)
        }
    }
}

fn view_enter_form(model: crate::Model) -> View {
    let em = model.screens.entry_app;
    view! {
        (move || {
            if em.admin.get() {
                return view! {};
            }
            if !model.entry_app.event.with(|e| e.entries_enabled) {
                return view! {
                    div(class="box") {
                        h2(class="title is-5") { "Enter the event" }
                        p(class="help") { "In-app entries are closed for this event." }
                    }
                };
            }
            if em.show_form.get() {
                view! {
                    div(class="box") {
                        h2(class="title is-5") { "Enter the event" }
                        div(class="field") {
                            label(class="label") { "Name" }
                            div(class="control") {
                                input(
                                    class="input",
                                    placeholder="Your name",
                                    bind:value=em.name,
                                    on:keydown=move |ev: web_sys::KeyboardEvent| {
                                        if ev.key_code() == 13 {
                                            crate::update(model, crate::Msg::EntryAppMsg(Msg::SubmitEntry));
                                        }
                                    },
                                )
                            }
                        }
                        div(class="field") {
                            label(class="label") { "Preferred car number" }
                            div(class="control") {
                                input(
                                    class="input",
                                    placeholder="Optional \u{2014} your usual number, e.g. 007",
                                    bind:value=em.preferred,
                                    on:keydown=move |ev: web_sys::KeyboardEvent| {
                                        if ev.key_code() == 13 {
                                            crate::update(model, crate::Msg::EntryAppMsg(Msg::SubmitEntry));
                                        }
                                    },
                                )
                            }
                            p(class="help") { "A preference only \u{2014} the timekeeper assigns the final number." }
                        }
                        div(class="field") {
                            label(class="label") { "Vehicle" }
                            div(class="control") {
                                input(
                                    class="input",
                                    placeholder="Optional \u{2014} make/model/notes",
                                    bind:value=em.vehicle,
                                    on:keydown=move |ev: web_sys::KeyboardEvent| {
                                        if ev.key_code() == 13 {
                                            crate::update(model, crate::Msg::EntryAppMsg(Msg::SubmitEntry));
                                        }
                                    },
                                )
                            }
                        }
                        div(class="field") {
                            label(class="label") { "Sharing a car?" }
                            div(class="control") {
                                input(
                                    class="input",
                                    placeholder="Optional \u{2014} shared-car name (rego, owner, description)",
                                    list="ea-shared-names",
                                    bind:value=em.shared,
                                    on:keydown=move |ev: web_sys::KeyboardEvent| {
                                        if ev.key_code() == 13 {
                                            crate::update(model, crate::Msg::EntryAppMsg(Msg::SubmitEntry));
                                        }
                                    },
                                )
                            }
                            p(class="help") { "Drivers sharing a car put the same name here so officials can run you early." }
                        }
                        div(class="field is-grouped") {
                            div(class="control") {
                                button(
                                    class="button is-primary",
                                    on:click=move |_| crate::update(model, crate::Msg::EntryAppMsg(Msg::SubmitEntry)),
                                ) { "Submit entry" }
                            }
                            div(class="control") {
                                button(
                                    class="button is-light",
                                    on:click=move |_| crate::update(model, crate::Msg::EntryAppMsg(Msg::CancelForm)),
                                ) { "Cancel" }
                            }
                        }
                        (view_feedback(model))
                    }
                }
            } else {
                view! {
                    div(class="field") {
                        div(class="control") {
                            button(
                                class="button is-primary",
                                on:click=move |_| crate::update(model, crate::Msg::EntryAppMsg(Msg::ShowForm)),
                            ) {
                                span(class="icon is-small") { i(class="fa fa-pen-to-square") }
                                span { "Enter now" }
                            }
                        }
                    }
                }
            }
        })
    }
}

fn view_feedback(model: crate::Model) -> View {
    let msg = model.screens.entry_app.feedback.get_clone();
    if msg.is_empty() {
        view! {}
    } else {
        view! { p(class="help is-danger") { (msg) } }
    }
}

fn view_batch_controls(model: crate::Model) -> View {
    let em = model.screens.entry_app;
    view! {
        (move || {
            if !em.admin.get() {
                return view! {};
            }
            let staged = em.staged.with(|v| v.len());
            let feedback = em.feedback.get_clone();
            let feedback_display: View = if feedback.is_empty() {
                view! {}
            } else {
                let fb = feedback;
                view! {
                    div(class="control") {
                        p(class="help is-danger") { (fb) }
                    }
                }
            };
            view! {
                div(class="field is-grouped") {
                    div(class="control") {
                        button(
                            class="button is-primary",
                            on:click=move |_| crate::update(model, crate::Msg::EntryAppMsg(Msg::SaveBatch)),
                        ) {
                            span(class="icon is-small") { i(class="fa fa-floppy-disk") }
                            span { "Save batch" }
                            (if staged > 0 {
                                view! { span(class="tag is-light") { (staged) } }
                            } else {
                                view! {}
                            })
                        }
                    }
                    div(class="control") {
                        button(
                            class="button is-link",
                            title="Fill suggested numbers + order for every unassigned entry",
                            on:click=move |_| crate::update(model, crate::Msg::EntryAppMsg(Msg::AssignNumbers)),
                        ) { "Assign numbers" }
                    }
                    div(class="control") {
                        button(
                            class="button is-light",
                            on:click=move |_| crate::update(model, crate::Msg::EntryAppMsg(Msg::DiscardBatch)),
                        ) { "Discard" }
                    }
                    (feedback_display)
                }
                p(class="help") { "Close-entries step: confirm details, assign numbers, set the running order, then Save." }
            }
        })
    }
}

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
