use crate::event::KTime;
use crate::event::KTimeTime;
use crate::event::ScoreData;
use crate::input::input_box;
use crate::input::input_clear;
use crate::input::input_update;
use crate::input::InputModel;
use crate::input::InputMsg;
use crate::view as show;
use crate::Model;

// Stage edit view.
// List of times... generally in order of entry.
// + big view of current last one
// + text field.
use lazy_regex::regex;
use seed::{prelude::*, *};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub enum StageMsg {
    CmdInput(InputMsg),
}
pub struct StageModel {
    cmd: InputModel,
    preview: Result<CmdParse, CmdError>,
    stage: u8,
}

// adds score from user entry in model
fn add_score(model: &mut Model) {
    // hmmm probably should cope with error to avoid user funnies?
    let s = match &model.stage_model.preview {
        Ok(CmdParse::Time(cmd)) => to_score(model.stage_model.stage, &cmd),
        _ => panic!(),
    };
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
    let model = StageModel {
        cmd: Default::default(),
        stage: 1,
        preview: Err(CmdError::Nothing), // hmm rubish OK
    };
    model
}

fn save_times(model: &Model) {
    crate::event::save_times(&model.event.name, &model.scores.as_ref());
}

pub fn update(msg: StageMsg, model: &mut Model, orders: &mut impl Orders<crate::Msg>) {
    match msg {
        StageMsg::CmdInput(InputMsg::DataEntry(value)) => {
            // typey typey
            input_update(&mut model.stage_model.cmd, value);
            // Show preview of what is about to happen on enter/save
            let cmd = parse_command(&model.stage_model.cmd.input);
            if let Ok(CmdParse::Time(_tc)) = cmd {
                // if tc.car in model.event.unw
                // todo! check is in event...
            }
            model.stage_model.preview = parse_command(&model.stage_model.cmd.input);
        }
        StageMsg::CmdInput(InputMsg::CancelEdit) => {
            input_clear(&mut model.stage_model.cmd);
            clear_cmd(&mut model.stage_model);
        }

        StageMsg::CmdInput(InputMsg::DoThing) => {
            let cmd = parse_command(&model.stage_model.cmd.input);
            match cmd {
                Ok(CmdParse::Time(_tc)) => {
                    log!("time");
                    add_score(model);
                    save_times(model);
                    orders.send_msg(crate::Msg::Reload);

                    clear_cmd(&mut model.stage_model);
                }
                Ok(CmdParse::Stage { number }) => {
                    model.stage_model.stage = number;
                    clear_cmd(&mut model.stage_model);
                }
                Ok(CmdParse::Event { event }) => {
                    orders.send_msg(crate::Msg::SetEvent(event));
                    clear_cmd(&mut model.stage_model);
                }

                Err(_) => log!("parse nope"),
            };
        }
    }
}

fn clear_cmd(model: &mut StageModel) {
    model.preview = Err(CmdError::Nothing); // hmm rubish OK
    input_clear(&mut model.cmd);
}

pub fn view(model: &Model) -> Node<StageMsg> {
    div! {
        h1![format!("Event: {} Stage:{}", model.event.name, model.stage_model.stage)],
        // sort buttons.
        // results list... here
        view_list(&model),
        view_preview(&model.stage_model),
        input_box_wrap(&model.stage_model.cmd),
    }
}

fn view_preview(model: &StageModel) -> Node<StageMsg> {
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
            return div!["Nothing to see here :-)"];
        }
        Err(CmdError::BadInput { value }) => {
            return div![value];
        }
    }
}

fn view_list(model: &Model) -> Node<StageMsg> {
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
    tr![
        td![score.stage.to_string()],
        td![show::car_number(&score.car)],
        td![show::ktime(&score.time)],
    ]
}

fn input_box_wrap(model: &InputModel) -> Node<StageMsg> {
    div![
        C!["pannel-block"],
        p![
            C!["control has-icons-left"],
            input_box(
                model,
                "enter times. stage to change stage",
                StageMsg::CmdInput
            ),
            span![C!["icon is-left"], i![C!["fas fa-car"]]]
        ],
    ]
}

// Result Error class for UI feedback
#[derive(thiserror::Error, Debug, Eq, PartialEq)]
pub enum CmdError {
    #[error("Invalid {value}")]
    BadInput { value: String },
    #[error("Ignoring")]
    Nothing,
}

/// Parse a string into a Command enum
/// Hide whichever matching is selected to parse
/// probably needs to start returning user feedback on errors?
fn parse_command(cmd: &str) -> Result<CmdParse, CmdError> {
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

    let (car, cmd) = parse_car(cmd)?;
    let (timestr, cmd) = parse_time_str(cmd)?;
    let (flags, garage) = parse_flags_garages(cmd)?;
    let code: KTime = match &timestr.to_ascii_uppercase()[..] {
        "WD" => KTime::WD,
        "NOSHO" => KTime::NOSHO,
        "FTS" => KTime::FTS,
        "DNF" => KTime::DNF,
        &_ => match timestr.parse::<f32>() {
            Ok(time) => {
                let ktt = KTimeTime {
                    time_ds: (10f32 * time) as u16,
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
pub fn parse_car(cmd: &str) -> Result<(&str, &str), CmdError> {
    let re = regex!(r"^\d+[A-Za-z]*");
    let s = cmd.trim_start();
    match re.find(s) {
        None => Err(bad_input("No car #number")),
        Some(m) => Ok((&s[0..m.end()], &s[m.end()..])),
    }
}

// find the timecode at start, return the rest as second field
// We are not checking for a valid code, so outer layer can give user
// feedback
fn parse_time_str(cmd: &str) -> Result<(String, &str), CmdError> {
    // let re = regex!(r"^(:WD|NOSHO|FTS|DNF|[0-9]+[.]?[0-9]*)");
    let re = regex!(r"^([A-Za-z]+|[0-9]+[.]?[0-9]*|[0-9]+:[0-9]+[.]?[0-9])");
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

#[derive(PartialEq, Debug, Default)]

struct TimeCmd {
    car: String,
    code: KTime,
}

#[derive(PartialEq, Debug)]
enum CmdParse {
    Stage { number: u8 },
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
        // assert_eq!(parse_time_str(" 1:1.23"), Ok(("61.23".to_string(), " XX")));
        assert_eq!(parse_time_str(" 1.23 XX"), Ok(("1.23".to_string(), " XX")));
        assert_eq!(
            parse_time_str(" NOSHO 1212"),
            Ok(("NOSHO".to_string(), " 1212"))
        );

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
                    time_ds: 101,
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
