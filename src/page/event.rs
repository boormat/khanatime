// Stage edit view.
// List of times... generally in order of entry.
// + big view of current last one
// + text field.
// strikeout old entries
// Sort button. #, edit order, result
use crate::Msg;
use seed::{prelude::*, *};
use serde::{Deserialize, Serialize};

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
    // official: String, //name
    // pubkey: String,   // officials ring Ed25519
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
    // Time(f32), // seconds
}

#[derive(Default, Serialize, Deserialize)]
struct Official {
    official: String, //name
    pubkey: String,   // officials ring Ed25519
}

#[derive(Default)]
pub struct StageModel {
    // edit box, list of times
    scores: Vec<ScoreData>,
    cmd: String,
    stage: i8,
    event: String,
}

pub fn view(model: &StageModel) -> Node<Msg> {
    // let var`
    div![
        h1![format!("{} Stage {}", model.event, model.stage)],
        // sort buttons.
        // results list... here

        // input![
        //         C!["input is-large"],
        //         attrs! {
        //             At::Placeholder => "New Event Name?"; // this changes
        //             At::AutoFocus => true.as_at_value();
        //             At::Value => model.cmd;
        //         },
        //         keyboard_ev(Ev::KeyDown, |keyboard_event| {
        //             match keyboard_event.key_code() {
        //                 ENTER_KEY => Some(Msg::Cre nateEvent),
        //                 ESC_KEY => Some(Msg::CancelEdit),
        //                 _ => None,
        //             }
        //         }),
        //         input_ev(Ev::Input, Msg::DataEntry),
        //     ],
        raw!(
            r#"<div class="panel-block">
        <p class="control has-icons-left">
          <input class="input" type="text" placeholder="what stage?" autofocus>
          <span class="icon is-left">
            <i class="fas fa-car" aria-hidden="true"></i>
          </span>
        </p>
      </div>"#
        ),
    ]
}
// }
//     section![
//         C!["hero", "is-medium", "ml-6"],
//         div![
//             C!["hero-body"],
//             h1![C!["title", "is-size-1"], "Khana Time Tracker",],
//             a![
//                 // attrs! {At::Href => "https://seed-rs.org/"},
//                 h2![C!["subtitle", "is-size-3"], "seed-rs.org"]
//             ],
//             a![
//                 C!["button", "is-primary", "mt-5", "is-size-5"],
//                 // attrs! {At::Href => Urls::new(base_url).time_tracker()},
//                 strong!["List events here"],
//             ],
//         ]
//     ]
