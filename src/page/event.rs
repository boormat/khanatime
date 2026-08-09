use crate::event::Entry;
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
}

#[derive(Clone, Copy)]
pub struct Model {
    pub class: InputModel,
    pub entrant: InputModel,
}

pub fn init() -> Model {
    Model {
        class: crate::input::init(),
        entrant: crate::input::init(),
    }
}

fn save_event(model: crate::Model) {
    crate::event::save_event(&model.event.get_clone());
}

pub fn update(model: crate::Model, msg: Msg) {
    // TODO Use a result to update the feedback?
    match msg {
        Msg::ClassInput(InputMsg::CancelEdit) => {
            input_clear(model.event_model.class);
        }
        Msg::EntryInput(InputMsg::CancelEdit) => {
            input_clear(model.event_model.entrant);
        }
        Msg::EntryInput(InputMsg::DoThing) => {
            let input = model.event_model.entrant.input.get_clone();
            if let Some((car, name)) = parse_car_and(&input[..]) {
                let mut ok = false;
                model.event.update(|e| ok = e.add_entry(car, name));
                if ok {
                    save_event(model);
                    input_clear(model.event_model.entrant);
                } else {
                    input_feedback(model.event_model.entrant, "Duplicate Entry.");
                }
            } else {
                input_feedback(
                    model.event_model.entrant,
                    "Can't parse Entry. Car#<space>Name",
                );
            }
        }
        Msg::EditClass(class) => {
            model.event_model.class.input.set(class.clone());
            model
                .event_model
                .class
                .feedback
                .set(format!("Editing class {class}"));
        }

        Msg::ClassInput(InputMsg::DoThing) => {
            // new or rename... if key not null?
            let key = model.event_model.class.key.get_clone();
            let input = model.event_model.class.input.get_clone();
            if key.is_empty() {
                model.event.update(|e| e.add_class(&input));
            } else {
                // can't remove without removing drivers first?
                model.event.update(|e| {
                    e.rename_class(&key, &input);
                });
                save_event(model);
            }
            input_clear(model.event_model.class);
        }

        Msg::DeleteClass(class) => {
            khanatime::log!("delete {}", class);
            // can't remove without removing drivers first?
            let mut removed = false;
            model.event.update(|e| removed = e.remove_class(&class));
            if removed {
                save_event(model);
            }
        }
        Msg::ToggleClass { car, class } => {
            khanatime::log!("toggle {} {}", car, class);
            model.event.update(|e| {
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
    }
}

pub fn view(model: crate::Model) -> View {
    view! {
        div {
            h1 {
                (move || {
                    format!(
                        "Event: {} Stages:{}",
                        model.event.with(|e| e.name.clone()),
                        model.event.with(|e| e.stages_count)
                    )
                })
            }
            // sort buttons.
            // results list... here
            (move || view_class_list(model))
            (input_box(
                model.event_model.class,
                "New Class?",
                move |msg| crate::update(model, crate::Msg::EventMsg(Msg::ClassInput(msg))),
            ))
            (move || view_entrant_list(model))
            (input_box(
                model.event_model.entrant,
                "New Entrant?",
                move |msg| crate::update(model, crate::Msg::EventMsg(Msg::EntryInput(msg))),
            ))
        }
    }
}

fn view_class_list(model: crate::Model) -> View {
    let classes = model.event.with(|event| event.classes.clone());
    let items = classes
        .iter()
        .map(|class| {
            let class = class.clone();
            let class_disp = class.clone();
            let class_del = class.clone();
            view! {
                li {
                    span(class="tag is-medium") {
                        i(
                            class="fa fa-pen-to-square",
                            on:click=move |_| {
                                crate::update(model, crate::Msg::EventMsg(Msg::EditClass(class.clone())))
                            },
                        )
                        (class_disp)
                        button(
                            class="delete is-danger",
                            on:click=move |_| {
                                crate::update(model, crate::Msg::EventMsg(Msg::DeleteClass(class_del.clone())))
                            },
                        )
                    }
                }
            }
        })
        .collect::<Vec<View>>();
    view! { ul(class="todo-list") { (items) } }
}

fn view_entrant_list(model: crate::Model) -> View {
    let (entries, classes) = model
        .event
        .with(|event| (event.entries.clone(), event.classes.clone()));
    let items = entries
        .iter()
        .map(|entry| view_entry(model, entry, &classes))
        .collect::<Vec<View>>();
    view! {
        div {
            header { h1 { "Entrants" } }
            ul { (items) }
        }
    }
}

fn view_entry(model: crate::Model, entry: &Entry, classes: &Vec<String>) -> View {
    let car = entry.car.clone();
    let name = entry.name.clone();
    let entry_classes = entry.classes.clone();
    let car_disp = car.clone();

    let mut class_checks: Vec<View> = vec![];
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

    view! {
        li {
            span(class="tag is-black") {
                i(class="fa fa-car", style="width: 20px")
                (car_disp)
            }
            span(style="width: 80px; margin: 10px") { (name) }
            (class_checks)
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
