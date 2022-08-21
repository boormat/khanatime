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
    Command,
    CancelEdit,
}
pub struct StageModel {
    scores: Vec<ScoreData>,
    cmd: String,
    preview: Option<CmdParse>,
    stage: u8,
    event: String,
}

#[derive(Default, Serialize, Deserialize, Debug)]
struct ScoreData {
    // keys For moment only accept int car numbers? 00 0B 24TBC
    stage: u8,
    car: String,
    time: KTime,
    flags: u8,
    garage: u8,
}

// adds score from user entry in model
fn add_score(model: &mut StageModel) {
    // hmmm probably should cope with error to avoid user funnies?
    let s = match &model.preview {
        Some(CmdParse::Time(cmd)) => to_score(model.stage, &cmd),
        _ => panic!(),
    };
    // to_score(model.stage, model.preview.unwrap())
    // todo invalidate existing score if replacing it... (should be in preview too!)
    model.scores.push(s);
}

fn to_score(stage: u8, cmd: &TimeCmd) -> ScoreData {
    ScoreData {
        stage,
        car: cmd.car.clone(),
        time: cmd.code.clone(),
        flags: cmd.flags,
        garage: cmd.garage,
    }
}

// #[derive(Copy, Clone, Default, Deserialize, PartialEq, Debug)]
#[derive(
    parse_display::FromStr,
    parse_display::Display,
    PartialEq,
    Debug,
    Serialize,
    Deserialize,
    Default,
    Clone,
)]
#[display("{}")]
enum KTime {
    #[default]
    NOSHO,
    WD,
    FTS,
    DNF,
    #[display("{0}")]
    Time(f32),
}
#[derive(Default, Serialize, Deserialize)]
struct Official {
    official: String, //name
    pubkey: String,   // officials ring Ed25519
}

pub fn init() -> StageModel {
    let mut model = StageModel {
        scores: Default::default(),
        cmd: Default::default(),
        stage: 1,
        event: "today.Khana".to_string(),
        preview: None,
    };
    load_ui(&mut model);
    load_event(&mut model);
    model
}

const STAGEPAGE_PREFIX: &str = "stagepage:";
// const STAGEPAGE_PREFIX: &str = "eventstagepage:";

fn load_event(model: &mut StageModel) {
    if !model.event.is_empty() {
        let key = format!("{}{}", STAGEPAGE_PREFIX, model.event);
        let s = LocalStorage::get(&key).unwrap_or_default();
        model.scores = s;
    }
}

fn save_event(model: &StageModel) {
    let key = format!("{}{}", STAGEPAGE_PREFIX, model.event);
    LocalStorage::insert(&key, &model.scores).expect("save data to LocalStorage");
    save_ui(model);
    log!("saving  event ", key);
}

fn load_ui(model: &mut StageModel) {
    if let Ok(event) = SessionStorage::get("event") {
        model.event = event;
    }
}

fn save_ui(model: &StageModel) {
    SessionStorage::insert("event", &model.event).expect("save data to SessionStorage");
}

// /// list of known events in storage.  String is storage key, is the event name
// /// if it fails .. empty is fine
// fn list_events() -> HashSet<String> {
//     let len = LocalStorage::len().unwrap_or_default();
//     let mut out: HashSet<String> = Default::default();
// }
pub fn update(msg: StageMsg, model: &mut StageModel) {
    match msg {
        StageMsg::StageDataEntry(value) => {
            model.cmd = value; // typey typey

            // Show preview of what is about to happen on enter/save
            model.preview = match parse_command(&model.cmd) {
                Ok(cmd) => Some(cmd),
                Err(_) => None,
            };
        }
        StageMsg::Command => {
            log!("cmd:", model.cmd);
            match &model.preview {
                Some(CmdParse::Time(_tc)) => {
                    log!("time");
                    add_score(model);
                    save_event(model);

                    clear_cmd(model);
                }
                Some(CmdParse::Stage { number }) => {
                    model.stage = *number;
                    clear_cmd(model);
                }
                Some(CmdParse::Event { event }) => {
                    model.event = event.clone();
                    save_ui(model);
                    load_event(model);
                    clear_cmd(model);
                }
                None => log!("parse nope"),
            };
        }
        StageMsg::CancelEdit => {
            clear_cmd(model);
        }
    }
}

fn clear_cmd(model: &mut StageModel) {
    model.preview = None;
    model.cmd.clear();
}

pub fn view(model: &StageModel) -> Node<StageMsg> {
    div! {
        h1![format!("Event: {} Stage:{}", model.event, model.stage)],
        // sort buttons.
        // results list... here
        view_list(&model),
        view_preview(&model),
        input_box_wrap(&model.cmd),
        // p!(model.cmd.to_string()),
    }
}

fn view_preview(model: &StageModel) -> Node<StageMsg> {
    // let val = match &model.preview {
    match &model.preview {
        Some(CmdParse::Time(tc)) => {
            log!(tc);
            // None
            // raw!("POSSIBLE time")
        }
        Some(CmdParse::Stage { number }) => {
            // raw!("POSIBLE stage")
            log!(number);
            // None
        }
        Some(CmdParse::Event { event }) => {
            log!(event);
            // None
        }
        None => {}
    };

    // div![if!( Some(val) => val)]
    seed::empty()
}

fn view_list(model: &StageModel) -> Node<StageMsg> {
    let mut v = vec![view_time_header()];
    for a in model.scores.iter() {
        v.push(view_time(&a));
    }
    table![v]
}

fn view_time_header() -> Node<StageMsg> {
    tr![th!["Stage"], th!["Car"], th!["Time"], th!["Flags"],]
}
fn view_time(score: &ScoreData) -> Node<StageMsg> {
    // log!(score.car);
    // log!(score);
    tr![
        td![score.stage.to_string()],
        td![view_car_number(&score.car)],
        td![view_time_score(&score.time)],
        td![
            IF! {score.flags > 0 => format!("{} Flags", score.flags)},
            IF! {score.garage > 0 => "Garage"},
        ],
    ]
}

fn view_time_score(time: &KTime) -> Node<StageMsg> {
    // tr![th!["Stage"], th!["Car"], th!["Time"], th!["Flags"],]
    log!(time.to_string());
    div!(time.to_string())
    // div!("Yolo".to_string())
}
// }
// tr![
//     td!{a!{i! C!["fa-solid fa-wrench"]]></i> ] }
//     <td><a><i className="icon-wrench edit"></i></a>}
//     <td><EntrantLabel car={this.props.car} name={this.props.name} /></td>
//     <td>{this.props.time}</td>
//     <td>{this.props.flags}F</td>
//     ]
// <span  key={car} className="label label-default"
// onClick={this.props.onClick.bind(null,car)}>
//     {car} {name}
// </span>

fn view_car_number(car: &String) -> Node<StageMsg> {
    span! {
        C!["label label-default"],
        car
    }
    //     {car} {name}
    // </span>
}

// nodes![
//     //
//     "yolo",
//     "yolo2",
//     // model.scores.iter()
//     // .map(|&score| => {
//     //     score
//     // }
//     // .collect::<_>()
//     // // model.scores.iter().collect( |score| =>
//     // )
// ]
// div![for score in model.scores.iter() {
//     // x.
//     raw!("yolo"),
//     //p!(raw!(format!("{}", score)))
// },]
// empty!

// fn view_stage_links(model: &Model) -> Node<Msg> {
// div![match &model.preview {
//     Some(CmdParse::Time(tc)) => {
//         raw!("POSSIBLE time")
//     }
//     Some(CmdParse::Stage { number }) => {
//         raw!("POSIBLE stage")
//     }
//     Some(CmdParse::Event { event }) => {
//         raw!("POSIBLE event")
//     }
//     None => raw!(""),
// },]
// }

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

/// Parse a string into a Command enum
/// Hide whichever matching is selected to parse
/// probably needs to start returning user feedback on errors?
fn parse_command(cmd: &String) -> Result<CmdParse, parse_display::ParseError> {
    // TODO insert trying the nice variations/easy to type eg 1 65.1, 2 WD
    cmd.parse::<CmdParse>()
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
    car: String,
    code: KTime,
    flags: u8,
    garage: u8,
}

// #[derive(parse_display::Display, PartialEq, Debug)]
#[derive(parse_display::FromStr, PartialEq, Debug)]
#[display("{0}")]
enum CmdParse {
    #[from_str(regex = "[sS](tage)? *(?P<number>[0-9]+)")]
    Stage {
        number: u8,
    },
    #[from_str(regex = "[eE](vent)? +(?P<event>.+) *$")]
    Event {
        event: String,
    },
    Time(TimeCmd),
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stage() {
        assert_eq!("s 1".parse(), Ok(CmdParse::Stage { number: 1 }));
        assert_eq!("Stage 1".parse(), Ok(CmdParse::Stage { number: 1 }));
        assert_eq!("S 200".parse(), Ok(CmdParse::Stage { number: 200 }));
        assert_eq!("t".parse::<CmdParse>().is_err(), true);
        assert_eq!("stagex 1".parse::<CmdParse>().is_err(), true);

        // times
        assert_eq!(
            "1 10.1 1F 1G".parse(),
            Ok(CmdParse::Time(TimeCmd {
                car: 1.to_string(),
                code: KTime::Time(10.1),
                flags: 1,
                garage: 1,
            }))
        );
        assert_eq!(
            "2 10.1 1F 1G".parse(),
            Ok(TimeCmd {
                car: 2.to_string(),
                code: KTime::Time(10.1),
                flags: 1,
                garage: 1,
            })
        );

        assert_eq!(
            "3 WD 0F 0G".parse(),
            Ok(TimeCmd {
                car: 3.to_string(),
                code: KTime::WD,
                flags: 0,
                garage: 0,
            })
        );
    }
}
