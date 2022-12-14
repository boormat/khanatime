use crate::event::Entry;
use crate::input::InputModel;
use crate::input::*;

// Event edit view.
// List of Classes. = derived from users?
// List of Entrants.
use lazy_regex::regex;
use seed::{prelude::*, *};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub enum Msg {
    // classes
    EditClass(String), // borkish
    DeleteClass(String),
    ClassInput(InputMsg),
    // entry stuff
    EntryInput(InputMsg),
    ToggleClass { car: String, class: String },
}

pub struct Model {
    class: InputModel,
    entrant: InputModel,
}

pub fn init() -> Model {
    let model = Model {
        class: Default::default(),
        entrant: Default::default(),
    };
    model
}

fn save_event(model: &crate::Model) {
    crate::event::save_event(&model.event);
}

pub fn update(msg: Msg, model: &mut crate::Model, _orders: &mut impl Orders<crate::Msg>) {
    // TODO Use a result to update the feedback?
    match msg {
        Msg::ClassInput(InputMsg::DataEntry(value)) => {
            input_update(&mut model.event_model.class, value); // typey typey
        }
        Msg::ClassInput(InputMsg::CancelEdit) => {
            input_clear(&mut model.event_model.class);
        }
        Msg::EntryInput(InputMsg::DataEntry(value)) => {
            input_update(&mut model.event_model.entrant, value); // typey typey
        }
        Msg::EntryInput(InputMsg::CancelEdit) => {
            input_clear(&mut model.event_model.entrant);
        }
        Msg::EntryInput(InputMsg::DoThing) => {
            if let Some((car, name)) = parse_car_and(&model.event_model.entrant.input[..]) {
                let ok = model.event.add_entry(car, name);
                if ok {
                    save_event(model);
                    input_clear(&mut model.event_model.entrant);
                } else {
                    input_feedback(&mut model.event_model.entrant, "Duplicate Entry.");
                }
            } else {
                input_feedback(
                    &mut model.event_model.entrant,
                    "Can't parse Entry. Car#<space>Name",
                );
            }
        }
        Msg::EditClass(class) => {
            model.event_model.class.input = format!("{class}");
            model.event_model.class.feedback = format!("Editing class {class}");
        }

        Msg::ClassInput(InputMsg::DoThing) => {
            // new or rename... if key not null?
            let input = &model.event_model.class;
            if input.key.is_empty() {
                let new = &input.input;
                model.event.add_class(&new);
            } else {
                // can't remove without removing drivers first?
                let new = &input.input;
                let old = &input.key;
                if model.event.rename_class(&old, &new) {
                    save_event(model);
                }
            }
            input_clear(&mut model.event_model.class);
        }

        Msg::DeleteClass(class) => {
            log!("delete", class);
            // can't remove without removing drivers first?
            if model.event.remove_class(&class) {
                save_event(model);
            }
        }
        Msg::ToggleClass { car, class } => {
            log!("toggle", car, class);
            if let Some(entry) = model.event.entries.iter_mut().find(|e| e.car == car) {
                if entry.classes.contains(&class) {
                    entry.classes.retain(|x| x != &class);
                } else {
                    entry.classes.push(class);
                }
                save_event(model);
            }
        }
    }
}

pub fn view(model: &crate::Model) -> Node<Msg> {
    div! {
        h1![format!("Event: {} Stages:{}", model.event.name, model.event.stages_count)],
        // sort buttons.
        // results list... here
        view_class_list(&model),
        input_box(&model.event_model.class, "New Class?", Msg::ClassInput ),
        view_entrant_list(&model),
        input_box(&model.event_model.entrant, "New Entrant?",  Msg::EntryInput),
    }
}

fn view_class_list(model: &crate::Model) -> Node<Msg> {
    ul![
        C!["todo-list"],
        model.event.classes.iter().map(|class| {
            let class1 = class.to_string();
            let class2 = class.to_string();

            li![
                el_key(&class),
                span![
                    C!["tag is-medium"],
                    i![
                        C!["fa fa-pen-to-square"],
                        ev(Ev::Click, move |_| Msg::EditClass(class1)),
                    ],
                    &class,
                    button![
                        C!["delete is-danger"],
                        ev(Ev::Click, move |_| Msg::DeleteClass(class2))
                    ],
                ],
            ]
        })
    ]
}

fn view_entrant_list(model: &crate::Model) -> Vec<Node<Msg>> {
    nodes! {
        header![h1!["Entrants"]],
        ul![model
            .event
            .entries
            .iter()
            .map(|entry| view_entry(model, &entry))],
    }
}

fn view_entry(model: &crate::Model, entry: &Entry) -> Node<Msg> {
    li![
        span![
            C!["tag is-black"],
            i!(
                C!("fa fa-car"),
                style!(
                    St::Width => px(20)
                ),
            ),
            style!(
                St::Width => px(40)
            ),
            &entry.car
        ],
        span![
            style!(
                St::Width => px(80)
                St::Margin => px(10)
            ),
            &entry.name,
        ],
        model.event.classes.iter().map(|class| {
            let class_on = entry.classes.contains(class);
            let class1 = class.to_string();
            let class2 = class.to_string();
            let car1 = entry.car.clone();
            label![
                C!["checkbox"],
                input![
                    attrs! {
                        At::Type => "checkbox",
                        At::Checked => class_on.as_at_value()
                    },
                    ev(Ev::Change, move |_| Msg::ToggleClass {
                        car: car1,
                        class: class2
                    }),
                ],
                &class1,
            ]
        }),
    ]
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
