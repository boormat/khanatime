// Stage edit view.
// List of times... generally in order of entry.
// + big view of current last one
// + text field.
// strikeout old entries
// Sort button. #, edit order, result
// use crate::Msg;
// use parse_display::FromStr;
use seed::{prelude::*, *};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub enum StageMsg {
    StageDataEntry(String),
    Bump,
    Command,
    CancelEdit,
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

// probably time to do in 2 phases, one to pull out as strings,
// especially for F and G flags.
#[derive(parse_display::FromStr, PartialEq, Debug, Default)]
#[from_str(regex = r####"(?x)
        ^\s*(?P<car>[0-9]+)
        (?:\s+(?P<code>WD|NOSHO|FTS|DNF|[0-9]+[.][0-9]*))?
        (?:\s+(?P<flags>[0-9])F)?
        (?:\s+(?P<garage>[0-9])G)?
        "####)]
// #[derive(parse_display::FromStr, PartialEq, Debug, Default)]
// #[display("{car} {code} {flags} {garage}")]
#[from_str(default)]
struct TimeCmd {
    car: u8,
    // time: Option<f32>,#        (?:\s+(?P<time>))?
    code: Codes,
    flags: u8,
    garage: u8,
}

// #[derive(Copy, Clone, Default, Deserialize, PartialEq, Debug)]
#[derive(parse_display::FromStr, PartialEq, Debug, Default)]
#[display("{}")]
enum Codes {
    #[default]
    NOSHO,
    WD,
    FTS,
    DNF,
    #[display("{0}")]
    Time(f32),
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stage() {
        // assert_eq!("s 1".parse(), Ok(CmdParse::Stage { number: 1 }));
        // assert_eq!("Stage 1".parse(), Ok(CmdParse::Stage { number: 1 }));
        // assert_eq!("s 200".parse(), Ok(CmdParse::Stage { number: 200 }));
        // assert_eq!("t".parse::<CmdParse>().is_err(), true);
        // assert_eq!("stagex 1".parse::<CmdParse>().is_err(), true);

        // times
        assert_eq!(
            "1 10.1 1F 1G".parse(),
            Ok(TimeCmd {
                car: 1,
                code: Codes::Time(10.1),
                flags: 1,
                garage: 1,
            })
        );
        assert_eq!(
            "2 10.1 1F 1G".parse(),
            Ok(TimeCmd {
                car: 2,
                code: Codes::Time(10.1),
                flags: 1,
                garage: 1,
            })
        );

        assert_eq!(
            "3 WD 0F 0G".parse(),
            Ok(TimeCmd {
                car: 3,
                code: Codes::WD,
                flags: 0,
                garage: 0,
            })
        );
    }
}

// fn parse_time(cmd: &str) -> Option<TimeCmd> {
//     let re = regex!(
//         r####"(?x)
//         ^\s*(?P<car>[0-9]+)
//         (?:\s+(?P<time>[0-9]+[.][0-9]*))?
//         (?:\s+(?P<code>WD|NOSHO|FTS|DNF)?
//         (?:\s+(?P<flags>[0-9]F)?
//         (?:\s+(?P<garage>G)?
//         "####,
//     );

//     if !re.is_match(cmd) {
//         return None;
//     }
//     return None;

//     // Ok(Default::default())
// }

// #[cfg(test)]
// mod tests {
//     use super::*;
//     #[test]
//     fn stage() {
//         let a = TimeCmd {
//             car: 1,
//             flags: 4,
//             garage: true,
//             time: Some(2.3),
//             code: Some(Codes::WD),
//         };

//         assert_eq!(parse_time("s 1"), Some(a));
//         // assert_eq!("Stage 1".parse(), Ok(CmdParse::Stage { number: 1 }));
//         // assert_eq!("s 200".parse(), Ok(CmdParse::Stage { number: 200 }));
//         // assert_eq!("t".parse::<CmdParse>().is_err(), true);
//         // assert_eq!("stagex 1".parse::<CmdParse>().is_err(), true);
//     }
// }

// #[derive(parse_display::FromStr, PartialEq, Debug)]
// #[from_str(regex = "[sS](tage)? *(?P<number>[0-9]+)")]
// struct CmdStage {
//     number: u8,
// }
// #[derive(parse_display::Display, PartialEq, Debug)]
// //[#display("{0:?}")]
// WD,
// #[derive(parse_display::FromStr, PartialEq, Debug)]
// enum CmdParse {
//     #[from_str(regex = "[sS](tage)? *(?P<number>[0-9]+)")]
//     Stage { number: u8 },
//     #[from_str(regex = " *(?P<car>[0-9]+) +(?P<time>[0-9.]+)? (?P<flags>[0-9]+)F")]
//     Time { car: u8, time: f32, flags: u8 },
//     // #[from_str(regex = " (?P<toks>[0-9]+)F")]
//     // Tokens { car: u8, toks: Vec<String> },
//     #[from_str(regex = " *(?P<car>[0-9]+) +(?P<time>[^ ]+)? (?P<flags>[0-9]+)F")]
//     TimeCode { car: u8, time: TimeCode, flags: u8 },
//     #[from_str(regex = "TC2 *(?P<car>[0-9]+) +(?P<time>[^ ]+) (?P<flags>[0-9]+)F +")]
//     TimeCode2 { car: u8, time: TimeCode, flags: u8 },
// }

// #[derive(parse_display::FromStr, PartialEq, Debug)]
// enum T1 {
//     #[from_str(regex = "(?:(?P<car[0-9]+) |(?P<flags>[0-9]+)F)+")]
//     C1 { car: u8, flags: u8 },
// }

// #[cfg(test)]
// mod tests {
//     use super::*;
//     #[test]
//     fn stage() {
//         assert_eq!("s 1".parse(), Ok(CmdParse::Stage { number: 1 }));
//         assert_eq!("Stage 1".parse(), Ok(CmdParse::Stage { number: 1 }));
//         assert_eq!("s 200".parse(), Ok(CmdParse::Stage { number: 200 }));
//         assert_eq!("t".parse::<CmdParse>().is_err(), true);
//         assert_eq!("stagex 1".parse::<CmdParse>().is_err(), true);

//         // times
//         assert_eq!(
//             "2 10.1 1F".parse(),
//             Ok(CmdParse::Time {
//                 car: 2,
//                 time: 10.1,
//                 flags: 1
//             })
//         );

//         assert_eq!(
//             "22 1 22F".parse(),
//             Ok(CmdParse::Time {
//                 car: 22,
//                 time: 1.0,
//                 flags: 22
//             })
//         );
//         assert_eq!("WD".parse(), Ok(TimeCode::WD));
//         assert_eq!(
//             "22 WD 22F".parse(),
//             Ok(CmdParse::TimeCode {
//                 car: 22,
//                 time: TimeCode::WD,
//                 flags: 22
//             })
//         );
//         assert_eq!(
//             "TC2 22 WD 22F".parse(),
//             Ok(CmdParse::TimeCode2 {
//                 car: 22,
//                 time: TimeCode::WD,
//                 flags: 22
//             })
//         );
//     }
// }
