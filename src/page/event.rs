use crate::event::Entry;
use crate::event::EventInfo;
// use crate::event::ScoreData;

//  Event edit view.
// List of Classes. = derived from users?
// List of Entrants.
// Import timing results... later
use lazy_regex::regex;
use seed::{prelude::*, *};
use serde::{Deserialize, Serialize};

// use super::stage::parse_car;

#[derive(Serialize, Deserialize, Clone)]
pub enum Msg {
    DataEntry(String),
    CancelEdit,
    // classes
    EditClass(Option<String>),
    DeleteClass(String),
    UpdateClass(String),
    // entry stuff
    CreateEntry,
    ToggleClass { car: String, class: String },
}
pub struct Model {
    // new_entry: String,
    input: InputModel,
    event: EventInfo,
    input_class: InputModel,
    edit_class: String,
    edit_entry: String,
}

pub fn init() -> Model {
    let mut model = Model {
        input: Default::default(),
        event: Default::default(),
        edit_class: Default::default(),
        input_class: Default::default(),
        edit_entry: Default::default(),
    };
    load_ui(&mut model);
    load_event(&mut model);
    model
}

const EVENTPAGE_PREFIX: &str = "eventpage:";

fn load_event(model: &mut Model) {
    if !model.event.name.is_empty() {
        let key = format!("{}{}", EVENTPAGE_PREFIX, model.event.name);
        let s = LocalStorage::get(&key).unwrap_or_default();
        model.event = s;
    }
}

fn save_event(model: &Model) {
    let key = format!("{}{}", EVENTPAGE_PREFIX, model.event.name);
    LocalStorage::insert(&key, &model.event).expect("save data to LocalStorage");
    save_ui(model);
    log!("saving  event ", key);
}

fn load_ui(model: &mut Model) {
    if let Ok(event) = SessionStorage::get("event") {
        model.event.name = event;
    }
}

fn save_ui(model: &Model) {
    SessionStorage::insert("event", &model.event.name).expect("save data to SessionStorage");
}

pub fn update(msg: Msg, model: &mut Model) {
    // TODO Use a result to update the feedback?
    match msg {
        Msg::DataEntry(value) => {
            input_update(&mut model.input, value); // typey typey
        }
        Msg::CancelEdit => {
            input_clear(&mut model.input);
        }
        Msg::CreateEntry => {
            if let Some((car, name)) = parse_car_and(&model.input.input[..]) {
                let ok = model.event.add_entry(car, name);
                if ok {
                    save_event(model);
                    save_ui(model);
                    input_clear(&mut model.input);
                } else {
                    input_feedback(&mut model.input, "Duplicate Entry.");
                }
            } else {
                input_feedback(&mut model.input, "Can't parse Entry. Car#<space>Name");
            }
        }
        Msg::EditClass(None) => model.edit_class.clear(),
        Msg::EditClass(Some(class)) => {
            model.edit_class = class.clone();
            model.input_class.input = class;
        }

        Msg::UpdateClass(class) => {
            log!("rename", class);
            let new = &model.input.input;
            // can't remove without removing drivers first?
            if model.event.rename_class(&class, &new) {
                save_event(model);
                save_ui(model);
            }
            input_clear(&mut model.input);
        }
        Msg::DeleteClass(class) => {
            log!("delete", class);
            // can't remove without removing drivers first?
            if model.event.remove_class(&class) {
                save_event(model);
                save_ui(model);
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
                save_ui(model);
            }
        }
    }
}

pub fn view(model: &Model) -> Node<Msg> {
    div! {
        h1![format!("Event: {} Stages:{}", model.event.name, model.event.stages_count)],
        // sort buttons.
        // results list... here
        view_class_list(&model),
        view_entrant_head(&model),
        view_entrant_list(&model),
        // input_box_wrap(&model.cmd),
        // p!(model.cmd.to_string()),
    }
}

fn view_class_list(model: &Model) -> Node<Msg> {
    ul![
        C!["todo-list"],
        model.event.classes.iter().map(|class| {
            let class1 = class.to_string();
            let class2 = class.to_string();
            // let id = todo.id;
            let edit = class == &model.edit_class;

            li![
                // C![IF!(todo.completed => "completed"), IF!(is_selected => "editing")],
                el_key(&class),
                div![
                    C!["view"],
                    // input![C!["toggle"],
                    //     attrs!{At::Type => "checkbox", At::Checked => todo.completed.as_at_value()},
                    //     ev(Ev::Change, move |_| Msg::ToggleTodo(id)),
                    // ],
                    // label![&class, ev(Ev::Click, move |_| Msg::EditClass(Some(class1))),],
                    IF!( edit => {
                        input_box(&model.input, "edit class", Msg::UpdateClass(class.clone()))
                    }),
                    IF!( !edit => {
                        label![&class, ev(Ev::Click, move |_| Msg::EditClass(Some(class1))),]
                    }),
                    button![
                        C!["destroy"],
                        "Delete",
                        ev(Ev::Click, move |_| Msg::DeleteClass(class2))
                    ],
                ],
                // IF!(is_selected => {
                //     let selected_todo = selected_todo.unwrap();
                //     input![C!["edit"],
                //         el_ref(&selected_todo.input_element),
                //         attrs!{At::Value => selected_todo.title},
                //         input_ev(Ev::Input, Msg::SelectedTodoTitleChanged),
                //         keyboard_ev(Ev::KeyDown, |keyboard_event| {
                //             Some(match keyboard_event.key().as_str() {
                //                 ESCAPE_KEY => Msg::SelectTodo(None),
                //                 ENTER_KEY => Msg::SaveSelectedTodo,
                //                 _ => return None
                //             })
                //         }),
                //         ev(Ev::Blur, |_| Msg::SaveSelectedTodo),
                //     ]
                // }),
            ]
        })
    ]
}
fn view_entrant_head(model: &Model) -> Node<Msg> {
    let cmd = Msg::CreateEntry;
    header![
        h1!["Entrants"],
        input_box(&model.input, "New Entrant?", cmd)
    ]
}

fn view_entrant_list(model: &Model) -> Node<Msg> {
    ul![model
        .event
        .entries
        .iter()
        .map(|entry| view_entry(model, &entry))]
}

fn view_time_header() -> Node<Msg> {
    tr![th![""], th!["Car"], th!["Time"], th!["Flags"],]
}

fn view_entry(model: &Model, entry: &Entry) -> Node<Msg> {
    li![
        label![&entry.name],
        model.event.classes.iter().map(|class| {
            let class_on = entry.classes.contains(class);
            let class = class.to_string();
            let class2 = class.to_string();
            let class3 = class.to_string();
            let car = entry.car.clone();
            let car2 = entry.car.clone();
            div![
                input![
                    C!["toggle"],
                    attrs! {
                        At::Type => "checkbox",
                        At::Checked => class_on.as_at_value()
                    },
                    ev(Ev::Change, move |_| Msg::ToggleClass { car, class: class2 }),
                ],
                label![
                    &class,
                    ev(Ev::Click, move |_| Msg::ToggleClass {
                        car: car2,
                        class: class3
                    })
                ],
            ]
        }),
    ]
}

fn view_car_number(car: &String) -> Node<Msg> {
    span! {
        C!["label label-default"],
        car
    }
}

// fn view_stage_links(model: &Model) -> Node<Msg> {
// div![match &model.preview {
//     Some(CmdParse::Time(tc)) => {
//         raw!("POSSIBLE time")
//     }
//     Some(CmdParse:: { number }) => {
//         raw!("POSIBLE stage")
//     }
//     Some(CmdParse::Event { event }) => {
//         raw!("POSIBLE event")
//     }
//     None => raw!(""),
// },]
// }

// fn input_box_wrap(val: &String) -> Node<Msg> {
//     div![
//         C!["pannel-block"],
//         p![
//             C!["control has-icons-left"],
//             input_box(val),
//             span![C!["icon is-left"], i![C!["fas fa-car"]]]
//         ],
//     ]
// }

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

#[derive(Default)]
pub struct InputModel {
    pub input: String,
    pub feedback: String,
}

// fn editable_box(model: &InputModel, placeholder: &str, cmd: Msg, edit: bool) -> Vec<Node<Msg>> {
//     if edit {
//         input_box(model, placeholder, cmd)
//     } else {
//         label![&class, ev(Ev::Click, move |_| Msg::EditClass(Some(class1))),]
//     }
// }

fn input_box(model: &InputModel, placeholder: &str, cmd: Msg) -> Vec<Node<Msg>> {
    const ENTER_KEY: u32 = 13;
    const ESC_KEY: u32 = 27;
    nodes![
        div![&model.feedback],
        input![
            C!["input"],
            attrs! {
                At::Value => model.input;
                At::AutoFocus => true.as_at_value();
                At::Placeholder => placeholder;
            },
            keyboard_ev(Ev::KeyDown, |keyboard_event| {
                match keyboard_event.key_code() {
                    ENTER_KEY => Some(cmd),
                    ESC_KEY => Some(Msg::CancelEdit),
                    _ => None,
                }
            }),
            input_ev(Ev::Input, Msg::DataEntry),
        ],
    ]
}

fn input_clear(model: &mut InputModel) {
    model.input.clear();
    model.feedback.clear();
}

fn input_update(model: &mut InputModel, msg: String) {
    model.input = msg;
    model.feedback.clear();
}

fn input_feedback(model: &mut InputModel, msg: &str) {
    model.feedback = msg.to_string();
}
