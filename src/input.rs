use seed::{prelude::*, *};
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct InputModel {
    pub key: String,
    pub input: String,
    pub feedback: String,
}

#[derive(Serialize, Deserialize, Clone)]
// #[derive(Clone)]
pub enum InputMsg {
    DoThing,
    DataEntry(String),
    CancelEdit,
}

pub fn input_box<Msg: std::clone::Clone + 'static>(
    model: &InputModel,
    placeholder: &str,
    base_msg: fn(InputMsg) -> Msg,
) -> Node<Msg> {
    const ENTER_KEY: u32 = 13;
    const ESC_KEY: u32 = 27;
    // so enums which take parameters are actually functions
    let do_thing = base_msg(InputMsg::DoThing);
    let cancel_edit: Msg = base_msg(InputMsg::CancelEdit);
    let data_entry = move |x| base_msg(InputMsg::DataEntry(x));
    div![
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
                    ENTER_KEY => Some(do_thing),
                    ESC_KEY => Some(cancel_edit),
                    _ => None,
                }
            }),
            input_ev(Ev::Input, data_entry),
        ],
    ]
}

pub fn input_clear(model: &mut InputModel) {
    model.key.clear();
    model.input.clear();
    model.feedback.clear();
}

pub fn input_update(model: &mut InputModel, msg: String) {
    model.input = msg;
    model.feedback.clear();
}

pub fn input_feedback(model: &mut InputModel, msg: &str) {
    model.feedback = msg.to_string();
}
