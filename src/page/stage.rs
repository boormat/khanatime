use crate::event::KTime;
use crate::event::KTimeTime;
use crate::event::ScoreData;

// Stage edit view.
// List of times... generally in order of entry.
// + big view of current last one
// + text field.
// strikeout old entries
// Sort button. #, edit order, result
// use crate::Msg;
// use parse_display::FromStr;
use lazy_regex::regex;
// use parse_display::ParseError;
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
    // preview: Option<CmdParse>,
    preview: Result<CmdParse, CmdError>,
    stage: u8,
    event: String,
}

// adds score from user entry in model
fn add_score(model: &mut StageModel) {
    // hmmm probably should cope with error to avoid user funnies?
    let s = match &model.preview {
        Ok(CmdParse::Time(cmd)) => to_score(model.stage, &cmd),
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
    }
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
        preview: Err(CmdError::Nothing), // hmm rubish OK
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
            model.preview = parse_command(&model.cmd);
            // model.preview  = parse_command(&model.cmd) {
            //     model.preview = cmd;
            //     // todo add more feedback for
            // };
        }
        StageMsg::Command => {
            log!("cmd:", model.cmd);
            match &model.preview {
                Ok(CmdParse::Time(_tc)) => {
                    log!("time");
                    add_score(model);
                    save_event(model);

                    clear_cmd(model);
                }
                Ok(CmdParse::Stage { number }) => {
                    model.stage = *number;
                    clear_cmd(model);
                }
                Ok(CmdParse::Event { event }) => {
                    model.event = event.clone();
                    save_ui(model);
                    load_event(model);
                    clear_cmd(model);
                }
                Err(_) => log!("parse nope"),
            };
        }
        StageMsg::CancelEdit => {
            clear_cmd(model);
        }
    }
}

fn clear_cmd(model: &mut StageModel) {
    model.preview = Err(CmdError::Nothing); // hmm rubish OK
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
        Ok(CmdParse::Time(tc)) => {
            return div![format!("Confirm time {:?}?", tc)];
        }
        Ok(CmdParse::Stage { number }) => {
            return div![format!("Edit stage {}?", number)];
        }
        Ok(CmdParse::Event { event }) => {
            return div![format!("Open event {}?", event)];
        }
        Err(CmdError::Nothing) => {
            return div![":-)"];
        }
        Err(CmdError::BadInput { value }) => {
            return div![value];
        }
    }
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
        // td![
        //     IF! {score.flags > 0 => format!("{} Flags", score.flags)},
        //     IF! {score.garage > 0 => "Garage"},
        // ],
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

// Result Error class for UI feedback
#[derive(thiserror::Error, Debug, Eq, PartialEq)]
enum CmdError {
    #[error("Invalid {value}")]
    BadInput { value: String },
    #[error("Ignoring")]
    Nothing,
}

/// Parse a string into a Command enum
/// Hide whichever matching is selected to parse
/// probably needs to start returning user feedback on errors?
fn parse_command(cmd: &str) -> Result<CmdParse, CmdError> {
    // TODO insert trying the nice variations/easy to type eg 1 65.1, 2 WD
    // recar = Regex::new(r"^\d+").unwrap();
    // let m = recar.find(cmd.trim_start());
    // if
    // if let Ok(res) = cmd.parse::<CmdParse>() {
    //     return Ok(res);

    match parse_stage_cmd(cmd) {
        Err(CmdError::Nothing) => {}
        // xx => return xx,
        Ok(scmd) => return Ok(scmd),
        Err(x) => return Err(x),
    }

    match parse_event_cmd(cmd) {
        Err(CmdError::Nothing) => {}
        // xx => return xx,
        Ok(scmd) => return Ok(scmd),
        Err(x) => return Err(x),
    }

    // #[from_str(regex = "[sS](tage)? *(?P<number>[0-9]+)")]
    // Stage { number: u8 },
    // TODO use hand rolled parsed to just use simple enum instead of structs
    // #[from_str(regex = "[eE](vent)? +(?P<event>.+) *$")]
    // Event { event: String },

    let (car, cmd) = parse_car(cmd)?;
    let (timestr, cmd) = parse_time_str(cmd)?;
    let (flags, garage) = parse_flags_garages(cmd)?;
    let code: KTime = match &timestr.to_ascii_uppercase()[..] {
        "WD" => KTime::WD,
        "NOSHO" => KTime::NOSHO,
        "FTS" => KTime::FTS,
        "DNF" => KTime::WD,
        &_ => match timestr.parse::<f32>() {
            Ok(time) => {
                let ktt = KTimeTime {
                    time,
                    flags,
                    garage,
                };
                KTime::Time(ktt)
            }
            Err(_) => return Err(bad_input("Could not Parse Time")),
        },
    };

    return Ok(CmdParse::Time(TimeCmd {
        car: car.to_string(),
        code,
    }));
}

fn parse_stage_cmd(cmd: &str) -> Result<CmdParse, CmdError> {
    // let re = regex!(r"^\d+");
    let re1 = regex!("^[sS](tage)? +");
    let s = cmd.trim_start();
    let extra = match re1.find(s) {
        None => return Err(CmdError::Nothing),
        Some(m) => &s[m.end()..],
    };

    // todo anyhow context ? syntax for nicerness
    let re2 = regex!("^[0-9]+ *$");
    let s = extra.trim_start();
    let extra = match re2.find(s) {
        None => return Err(bad_input("No stage #number")),
        Some(m) => m.as_str(),
    };

    match extra.parse::<u8>() {
        Ok(s) => Ok(CmdParse::Stage { number: s }),
        Err(_) => Err(bad_input("Bad stage #number")),
    }
}

fn parse_event_cmd(cmd: &str) -> Result<CmdParse, CmdError> {
    // let re = regex!(r"^\d+");
    // "[eE](vent)? +(?P<event>.+) *$")]
    let re1 = regex!("^[eE](vent)? +");
    let s = cmd.trim_start();
    let extra = match re1.find(s) {
        None => return Err(CmdError::Nothing),
        Some(m) => &s[m.end()..],
    };

    // todo anyhow context ? syntax for nicerness
    let re2 = regex!("^.+ *$");
    let s = extra.trim_start();
    let extra = match re2.find(s) {
        None => return Err(bad_input("No event name")),
        Some(m) => m.as_str(),
    };

    Ok(CmdParse::Event {
        event: extra.into(),
    })
}

fn bad_input(msg: &str) -> CmdError {
    CmdError::BadInput { value: msg.into() }
}

// find the car# at, return the rest as second field
// if there is no car, its empty
fn parse_car(cmd: &str) -> Result<(&str, &str), CmdError> {
    let re = regex!(r"^\d+");
    let s = cmd.trim_start();
    match re.find(s) {
        // None => (&s[0..0], &s[0..]),
        // if car.len() == 0 {
        None => Err(bad_input("No car #number")),
        Some(m) => Ok((&s[0..m.end()], &s[m.end()..])),
    }
}

// fn parse_time(cmd: &str) -> Result<KTime, CmdError> {
//     Some(m) => match m.as_str().to_uppercase().parse::<KTime>() {
//         Err(_) => Err(CmdError("Invalid time or code".to_string())),
//         Ok(t) => {
//             let rest = &s[m.end()..];
//             Ok((t, rest))
//         }
//     },

// }

// find the timecode at start, return the rest as second field
// We are not checking for a valid code, so outer layer can give user
// feedback
fn parse_time_str(cmd: &str) -> Result<(String, &str), CmdError> {
    // let re = regex!(r"^(:WD|NOSHO|FTS|DNF|[0-9]+[.]?[0-9]*)");
    let re = regex!(r"^([A-Za-z]+|[0-9]+[.]?[0-9]*)");
    let s = cmd.trim_start();
    match re.find(s) {
        None => Err(bad_input("Invalid time or unexpected code")),
        Some(m) => {
            let rest = &s[m.end()..];
            let s = m.as_str().to_uppercase();
            Ok((s, rest))
        }
    }
}

// count garages and flags.
// Only 1 garage allowed.  G|g|1g|0g
// TODO make sure notices extra stuff
fn parse_flags_garages(cmd: &str) -> Result<(u8, bool), CmdError> {
    // let re = regex!(r"^(:WD|NOSHO|FTS|DNF|[0-9]+[.]?[0-9]*)");
    let re = regex!(r"^ *([0-9]*)([fFgG])");

    let mut flags: u8 = 0;
    let mut garages: u8 = 0;

    let mut s: &str = cmd.trim_start();
    while let Some(caps) = re.captures(s) {
        let mut tags = 1; //default
                          // None => Err(CmdError("Invalid Flag or Garage String".to_string())),
        if let Some(numm) = caps.get(1) {
            let numstr = numm.as_str();
            if numstr.len() > 0 {
                match numstr.parse() {
                    Ok(v) => {
                        tags = v;
                    }
                    Err(_) => {
                        return Err(bad_input("Invalid Flag or Garage Count"));
                    }
                }
            }
        }

        match caps.get(2).unwrap().as_str() {
            "f" => flags += tags,
            "F" => flags += tags,
            "g" => garages += tags,
            "G" => garages += tags,
            _ => panic!(),
        }

        s = &s[caps.get(0).unwrap().as_str().len()..]; // move along
    }

    if s.trim().len() > 0 {
        return Err(bad_input("Trailing text, expecting Flags/Garage"));
    }

    if garages > 1 {
        return Err(bad_input("Too many garage penalties"));
    }
    Ok((flags, garages == 1))
}
//             match u8::from_str(numstr) {
//                 None =>
//                 Ok(v) => {
//                     count = v;
//                 }
//                         Err => {
//                             return Err(CmdError("Invalid Flag or Garage Count".to_string()));
//                         }
//                     }
//                 }
//             }
//         }

//         println!("Movie: {:?}, Released: {:?}", &caps["title"], &caps["year"]);
//     }
//     let s = cmd.trim_start();
//     match re.find(s) {
//         None => Err(CmdError("Invalid Flag or Garage String".to_string())),
//         Some(num, "g") => match m.as_str().parse::<KTime>() {
//             Err() => Err(CmdError("Invalid time or code".to_string())),
//             Ok(KTime(t)) => {
//                 let rest = &s[m.end()..];
//                 Ok((t, rest))
//             }
//         },
//     }
// }

// probably time to do in 2 phases, one to pull out as strings,
// especially for F and G flags.
#[derive(PartialEq, Debug, Default)]
// #[from_str(regex = r####"(?x)
//         ^\s*(?P<car>[0-9]+)
//         (?:\s+(?P<code>WD|NOSHO|FTS|DNF|[0-9]+[.][0-9]*))?
//         (?:\s+(?P<flags>[0-9])F)?
//         (?:\s+(?P<garage>[0-9])G)?
//         "####)]
// #[display("{0}")]
// #[derive(parse_display::FromStr, PartialEq, Debug, Default)]
// #[display("{car} {code} {flags} {garage}")]
// #[from_str(default)]
struct TimeCmd {
    car: String,
    code: KTime,
}

// #[derive(parse_display::Display, PartialEq, Debug)]
// #[derive(parse_display::FromStr, PartialEq, Debug)]
#[derive(PartialEq, Debug)]
// #[display("{0}")]
enum CmdParse {
    // #[from_str(regex = "[sS](tage)? *(?P<number>[0-9]+)")]
    Stage { number: u8 },
    // TODO use hand rolled parsed to just use simple enum instead of structs
    // #[from_str(regex = "[eE](vent)? +(?P<event>.+) *$")]
    Event { event: String },
    Time(TimeCmd),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        assert_eq!(parse_car("1"), Ok(("1", "")));
        assert_eq!(parse_car(" 11 "), Ok(("11", " ")));
        assert_eq!(parse_car(" 22 3.3 FF "), Ok(("22", " 3.3 FF ")));

        assert_eq!(parse_time_str("WD"), Ok(("WD".to_string(), "")));
        assert_eq!(parse_time_str("wD"), Ok(("WD".to_string(), "")));
        assert_eq!(parse_time_str(" 1.23 XX"), Ok(("1.23".to_string(), " XX")));
        assert_eq!(
            parse_time_str(" NOSHO 1212"),
            Ok(("NOSHO".to_string(), " 1212"))
        );
        // assert_eq!(parse_time("WD"), Ok((KTime::WD, "")));
        // assert_eq!(parse_time("wD"), Ok((KTime::WD, "")));
        // assert_eq!(parse_time(" 1.23 XX"), Ok((KTime::Time(1.23), " XX")));
        // assert_eq!(parse_time(" NOSHO 1212"), Ok((KTime::NOSHO, " 1212")));

        assert_eq!(parse_flags_garages(" 1F1G"), Ok((1, true)));
        assert_eq!(parse_flags_garages(" FFG "), Ok((2, true)));
        assert_eq!(parse_flags_garages(" F4F0G"), Ok((5, false)));
        assert_eq!(parse_flags_garages(" F 4F GF 4F"), Ok((10, true)));
        assert_eq!(parse_flags_garages(" F4FGG").is_err(), true);
        assert_eq!(parse_flags_garages(" 4FF0G sdfs").is_err(), true);
        // let (code, cmd) = parse_time(cmd)?;
        // let (flags, garage) = parse_flags_garages(cmd)?;
    }

    #[test]
    fn parse_ccommands() {
        assert_eq!(parse_command("s 1"), Ok(CmdParse::Stage { number: 1 }));
        assert_eq!(parse_command("Stage 1"), Ok(CmdParse::Stage { number: 1 }));
        assert_eq!(parse_command("S 200"), Ok(CmdParse::Stage { number: 200 }));
        assert_eq!(parse_command("t").is_err(), true);
        assert_eq!(parse_command("stagex 1").is_err(), true);

        assert_eq!(
            parse_command("e a"),
            Ok(CmdParse::Event { event: "a".into() })
        );
        assert_eq!(
            parse_command("event abc"),
            Ok(CmdParse::Event {
                event: "abc".into()
            })
        );
        assert_eq!(parse_command("et aa").is_err(), true);

        // times
        assert_eq!(
            parse_command("1 10.1 1F 1G"),
            Ok(CmdParse::Time(TimeCmd {
                car: 1.to_string(),
                code: KTime::Time(KTimeTime {
                    time: 10.1,
                    flags: 1,
                    garage: true,
                }),
            }))
        );
        assert_eq!(
            parse_command("2 WD"),
            Ok(CmdParse::Time(TimeCmd {
                car: 2.to_string(),
                code: KTime::WD,
            }))
        );
    }
}
