use crate::event::KTime;
use crate::event::KTimeTime;
use crate::event::ScoreData;
use crate::input::input_box;
use crate::input::input_clear;
use crate::input::InputModel;
use crate::input::InputMsg;
use crate::view as show;
use crate::Model;

// Stage edit view.
// List of times... generally in order of entry.
// + big view of current last one
// + text field.
use lazy_regex::regex;
use serde::{Deserialize, Serialize};
use sycamore::prelude::*;

#[derive(Serialize, Deserialize, Clone)]
pub enum StageMsg {
    CmdInput(InputMsg),
}

#[derive(Clone, Copy)]
pub struct StageModel {
    pub cmd: InputModel,
    pub preview: Signal<Result<CmdParse, CmdError>>,
    pub stage: Signal<u8>,
}

// adds score from user entry in model
fn add_score(model: Model) {
    // hmmm probably should cope with error to avoid user funnies?
    let s = model.screens.stage.preview.with(|p| match p {
        Ok(CmdParse::Time(cmd)) => to_score(model.screens.stage.stage.get(), cmd),
        _ => panic!("add_score called without a parsed time command"),
    });
    model.khana.scores.update(|v| v.push(s));
}

fn to_score(stage: u8, cmd: &TimeCmd) -> ScoreData {
    ScoreData {
        stage,
        car: cmd.car.clone(),
        time: cmd.code.clone(),
    }
}

pub fn init() -> StageModel {
    StageModel {
        cmd: crate::input::init(),
        stage: create_signal(1),
        preview: create_signal(Err(CmdError::Nothing)),
    }
}

/// Send the entered time to the current event's pending outbox (the durable
/// record until it's flushed to the timing room).
fn broadcast_time(model: Model, car: &str, stage: u8, time: &KTime) {
    crate::page::enqueue_ktime(model, stage, car, time, None);
}

pub fn update(model: Model, msg: StageMsg) {
    match msg {
        StageMsg::CmdInput(InputMsg::CancelEdit) => {
            clear_cmd(model);
        }

        StageMsg::CmdInput(InputMsg::DoThing) => {
            let input = model.screens.stage.cmd.input.get_clone();
            let cmd = parse_command(&input);
            match cmd {
                Ok(CmdParse::Time(tc)) => {
                    khanatime::log!("time");
                    add_score(model);
                    broadcast_time(model, &tc.car, model.screens.stage.stage.get(), &tc.code);
                    crate::update(model, crate::Msg::Reload);

                    clear_cmd(model);
                }
                Ok(CmdParse::Stage { number }) => {
                    model.screens.stage.stage.set(number);
                    clear_cmd(model);
                }
                Ok(CmdParse::Event { event }) => {
                    crate::update(model, crate::Msg::SetEvent(event));
                    clear_cmd(model);
                }

                Err(_) => khanatime::log!("parse nope"),
            };
        }
    }
}

fn clear_cmd(model: Model) {
    model.screens.stage.preview.set(Err(CmdError::Nothing)); // hmm rubish OK
    input_clear(model.screens.stage.cmd);
}

pub fn view(model: Model) -> View {
    view! {
        div {
            h1(class="title is-4") { "Manual entry" }
            h2 {
                (move || {
                    format!(
                        "Event: {}  Stage:{}",
                        model.khana.event.with(|e| e.name.clone()),
                        model.screens.stage.stage.get()
                    )
                })
            }
            (move || view_list(model))
            (move || view_preview(model))
            (input_box_wrap(model))
            (move || {
                let test = model.screens.stage.stage.get();
                crate::page::view_timing_log(model, test)
            })
        }
    }
}

fn view_preview(model: Model) -> View {
    view! {
        div {
            (move || {
                model.screens.stage.preview.with(|p| match p {
                    Ok(CmdParse::Time(tc)) => {
                        let msg = format!("Confirm time {:?}?", tc);
                        view! { div { (msg) } }
                    }
                    Ok(CmdParse::Stage { number }) => {
                        let msg = format!("Edit stage {}?", number);
                        view! { div { (msg) } }
                    }
                    Ok(CmdParse::Event { event }) => {
                        let msg = format!("Open event {}?", event);
                        view! { div { (msg) } }
                    }
                    Err(CmdError::Nothing) => view! { div { "Nothing to see here :-)" } },
                    Err(CmdError::BadInput { value }) => {
                        let value = value.clone();
                        view! { div { (value) } }
                    }
                })
            })
        }
    }
}

fn view_list(model: Model) -> View {
    let mut v = vec![view_time_header()];
    model.khana.scores.with(|scores| {
        for a in scores.iter() {
            v.push(view_time(a));
        }
    });
    view! { table { (v) } }
}

fn view_time_header() -> View {
    view! {
        tr {
            th { "Stage" }
            th { "Car" }
            th { "Time" }
            th { "Flags" }
        }
    }
}
fn view_time(score: &ScoreData) -> View {
    let stage = score.stage.to_string();
    let car = show::car_number(score.car.clone());
    let time = show::ktime(&score.time);
    view! {
        tr {
            td { (stage) }
            td { (car) }
            td { (time) }
        }
    }
}

fn input_box_wrap(model: Model) -> View {
    let dispatch =
        move |msg: InputMsg| crate::update(model, crate::Msg::StageMsg(StageMsg::CmdInput(msg)));
    view! {
        div(class="pannel-block") {
            p(class="control has-icons-left") {
                (input_box(
                    model.screens.stage.cmd,
                    "enter times. stage to change stage",
                    dispatch,
                ))
                span(class="icon is-left") { i(class="fas fa-car") }
            }
        }
    }
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
pub fn parse_command(cmd: &str) -> Result<CmdParse, CmdError> {
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

    Ok(CmdParse::Time(TimeCmd {
        car: car.to_string(),
        code,
    }))
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
            if !numstr.is_empty() {
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

    if !s.trim().is_empty() {
        return Err(bad_input("Trailing text, expecting Flags/Garage"));
    }

    if garages > 1 {
        return Err(bad_input("Too many garage penalties"));
    }
    Ok((flags, garages == 1))
}

#[derive(PartialEq, Debug, Default)]
pub struct TimeCmd {
    car: String,
    code: KTime,
}

#[derive(PartialEq, Debug)]
pub enum CmdParse {
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
        assert!(parse_flags_garages(" F4FGG").is_err());
        assert!(parse_flags_garages(" 4FF0G sdfs").is_err());
        // let (code, cmd) = parse_time(cmd)?;
        // let (flags, garage) = parse_flags_garages(cmd)?;
    }

    #[test]
    fn parse_ccommands() {
        assert_eq!(parse_command("s 1"), Ok(CmdParse::Stage { number: 1 }));
        assert_eq!(parse_command("Stage 1"), Ok(CmdParse::Stage { number: 1 }));
        assert_eq!(parse_command("S 200"), Ok(CmdParse::Stage { number: 200 }));
        assert!(parse_command("t").is_err());
        assert!(parse_command("stagex 1").is_err());

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
        assert!(parse_command("et aa").is_err());

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
