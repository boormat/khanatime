// Stage edit view.
// List of times... generally in order of entry.
// + big view of current last one
// + text field.
// strikeout old entries
// Sort button. #, edit order, result
// use crate::Msg;
use seed::{prelude::*, *};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub enum StageMsg {
    StageDataEntry(String), //this is the tricky one?
    Bump,
    Command,
    CancelEdit,
}

pub fn update(msg: StageMsg, model: &mut StageModel) {
    match msg {
        StageMsg::StageDataEntry(value) => {
            model.cmd = value; // typey typey
        }
        StageMsg::Bump => {
            log!("bump");
            model.cmd = model.cmd.clone() + ".";
        }
        StageMsg::Command => {
            log!("cmd:", model.cmd);
            model.cmd.clear();
        }
        StageMsg::CancelEdit => {
            model.cmd.clear();
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
struct ScoreData {
    // keys
    stage: i8,
    car: String,

    // date
    time: f32, // as entered.. maybe an enum? of codes and time? pritable, so time plus penalties etc.
    code: Code, // enter code and score (just in case WD gets changed later)
    flags: i8,
    garage: bool,

    // edit info
    official: Official,
    signed: String, // sign of official
    ts: u64,        // datetime of the data entry
    ignore: bool,   // mark if edit/replaced or when official knows was a bad time.
    primary: bool,  // if official/time was the primary timer.
}

#[derive(Default, Serialize, Deserialize)]
enum Code {
    #[default]
    DNS,
    WD,
    FTS,
    DNF,
}

#[derive(Default, Serialize, Deserialize)]
struct Official {
    official: String, //name
    pubkey: String,   // officials ring Ed25519
}

#[derive(Default)]
pub struct StageModel {
    // edit box, list of times
    #[allow(dead_code)]
    scores: Vec<ScoreData>,
    #[allow(dead_code)]
    cmd: String,
    stage: i8,
    event: String,
}

pub fn view(model: &StageModel) -> Node<StageMsg> {
    div! {
        h1![format!("{} Stage {}", model.event, model.stage)],
        // sort buttons.
        // results list... here
        input_box_wrap(&model.cmd),
        p!(model.cmd.to_string()),
    }
}

fn input_box_wrap(val: &String) -> Node<StageMsg> {
    div![
        C!["pannel-block"],
        p![
            C!["control has-icons-left"],
            input_box(val),
            span![C!["icon is-left"], i![C!["fas fa-car"]]]
        ],
    ]
}

fn input_box(val: &String) -> Node<StageMsg> {
    // copy here to avoid bogus unused warnings
    const ENTER_KEY: u32 = 13;
    const ESC_KEY: u32 = 27;
    // empty![]
    input![
        C!["input"],
        attrs! {
            At::Value => val;
            At::AutoFocus => true.as_at_value();
            At::Placeholder => "enter times. stage to change stage";
        },
        keyboard_ev(Ev::KeyDown, |keyboard_event| {
            match keyboard_event.key_code() {
                ENTER_KEY => Some(StageMsg::Command),
                ESC_KEY => Some(StageMsg::CancelEdit),
                _ => None,
            }
        }),
        input_ev(Ev::Input, StageMsg::StageDataEntry),
    ]
}
