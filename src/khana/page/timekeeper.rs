use crate::event::KTime;
use crate::event::KTimeTime;
use crate::input::input_box;
use crate::input::input_clear;
use crate::input::InputModel;
use crate::input::InputMsg;
use crate::view as show;

use lazy_regex::regex;
use serde::{Deserialize, Serialize};
use sycamore::prelude::*;

#[derive(Serialize, Deserialize, Clone)]
pub enum TimekeeperMsg {
    CmdInput(InputMsg),
    /// Start editing an existing observation (pre-fills the manual entry box).
    StartEdit(String),
    /// Void an existing observation by uid.
    Void(String),
}

#[derive(Clone, Copy)]
pub struct TimekeeperModel {
    pub cmd: InputModel,
    pub preview: Signal<Result<CmdParse, CmdError>>,
    pub stage: Signal<u8>,
    /// uid of the observation being edited, if any.
    pub editing_uid: Signal<Option<String>>,
}

pub fn init() -> TimekeeperModel {
    TimekeeperModel {
        cmd: crate::input::init(),
        stage: create_signal(1),
        preview: create_signal(Err(CmdError::Nothing)),
        editing_uid: create_signal(None),
    }
}

pub fn update(model: crate::Model, msg: TimekeeperMsg) {
    match msg {
        TimekeeperMsg::CmdInput(InputMsg::CancelEdit) => {
            model.screens.timekeeper.editing_uid.set(None);
            clear_cmd(model);
        }

        TimekeeperMsg::CmdInput(InputMsg::DoThing) => {
            let input = model.screens.timekeeper.cmd.input.get_clone();
            let cmd = parse_command(&input);
            match cmd {
                Ok(CmdParse::Time(tc)) => {
                    let test = model.screens.timekeeper.stage.get();
                    match model.screens.timekeeper.editing_uid.get_clone() {
                        Some(target_uid) => {
                            crate::khana::helpers::enqueue_amend(
                                model,
                                &target_uid,
                                test,
                                &tc.car,
                                &tc.code,
                                None,
                            );
                            model.screens.timekeeper.editing_uid.set(None);
                        }
                        None => {
                            crate::khana::helpers::enqueue_ktime(
                                model, test, &tc.car, &tc.code, None,
                            );
                        }
                    }
                    crate::update(model, crate::Msg::Reload);
                    clear_cmd(model);
                }
                Ok(CmdParse::Stage { number }) => {
                    model.screens.timekeeper.stage.set(number);
                    clear_cmd(model);
                }
                Ok(CmdParse::Event { event }) => {
                    crate::update(model, crate::Msg::SetEvent(event));
                    clear_cmd(model);
                }

                Err(_) => khanatime::log!("parse nope"),
            };
        }

        TimekeeperMsg::StartEdit(uid) => {
            let runs = model
                .khana
                .runs
                .with(|runs| runs.iter().find(|r| r.uid == uid).cloned());
            if let Some(run) = runs {
                if let Some(time_ds) = run.time_ds {
                    let time_s = time_ds as f32 / 10.0;
                    let flags = run.flags.unwrap_or(0);
                    let garage_char = if matches!(run.status.as_deref(), Some("garage")) {
                        "G"
                    } else {
                        ""
                    };
                    let input = format!("{} {} {}{}", run.car, time_s, flags, garage_char);
                    model.screens.timekeeper.cmd.input.set(input);
                    model.screens.timekeeper.editing_uid.set(Some(uid));
                }
            }
        }

        TimekeeperMsg::Void(uid) => {
            let test = model.screens.timekeeper.stage.get();
            let car = model.khana.runs.with(|runs| {
                runs.iter()
                    .find(|r| r.uid == uid)
                    .map(|r| r.car.clone())
                    .unwrap_or_default()
            });
            crate::khana::helpers::enqueue_void(model, &uid, test, &car);
        }
    }
}

fn clear_cmd(model: crate::Model) {
    model.screens.timekeeper.preview.set(Err(CmdError::Nothing)); // hmm rubish OK
    input_clear(model.screens.timekeeper.cmd);
}

pub fn view(model: crate::Model) -> View {
    view! {
        div {
            h1(class="title is-4") { "Timekeeper" }
            h2 {
                (move || {
                    format!(
                        "Event: {}  Stage:{}",
                        model.khana.event.with(|e| e.name.clone()),
                        model.screens.timekeeper.stage.get()
                    )
                })
            }
            (input_box_wrap(model))
            (move || view_preview(model))
            (move || view_timing_observations(model))
            (move || view_compact_results(model))
        }
    }
}

fn view_preview(model: crate::Model) -> View {
    let editing = model.screens.timekeeper.editing_uid.get_clone().is_some();
    view! {
        div(class="box") {
            h3(class="title is-6") {
                (if editing { "Editing observation" } else { "New time entry" })
            }
            (move || {
                model.screens.timekeeper.preview.with(|p| match p {
                    Ok(CmdParse::Time(tc)) => {
                        let msg = format!("Confirm time {:?}?", tc);
                        view! { div { (msg) } }
                    }
                    Ok(CmdParse::Stage { number }) => {
                        let msg = format!("Switch to stage {}?", number);
                        view! { div { (msg) } }
                    }
                    Ok(CmdParse::Event { event }) => {
                        let msg = format!("Open event {}?", event);
                        view! { div { (msg) } }
                    }
                    Err(CmdError::Nothing) => view! { div(class="has-text-grey") {
                        (if editing { "Modify the time above, Enter to confirm, Escape to cancel" } else { "Enter car + time (e.g. 12 10.1 1F)" })
                    } },
                    Err(CmdError::BadInput { value }) => {
                        let value = value.clone();
                        view! { div(class="has-text-danger") { (value) } }
                    }
                })
            })
        }
    }
}

/// Timing observations for the current stage, with Void and Edit buttons.
fn view_timing_observations(model: crate::Model) -> View {
    use crate::event::{RunRecord, RUN_FINISH, RUN_START, RUN_STOP};
    view! {
        div(class="box") {
            h3(class="title is-6") { "Observations" }
            (move || {
                let _now = model.tick.get(); // subscribe to tick for live "Xs ago"
                let now = js_sys::Date::now() as i64;
                let test = model.screens.timekeeper.stage.get();
                let runs: Vec<RunRecord> = model.khana.runs.with(|runs| {
                    runs.iter()
                        .filter(|r| r.test == test && !r.voided)
                        .filter(|r| r.r#type == RUN_START || r.r#type == RUN_FINISH || r.r#type == RUN_STOP)
                        .cloned()
                        .collect()
                });
                let mut runs = runs;
                runs.sort_by_key(|r| std::cmp::Reverse(r.ts));
                if runs.is_empty() {
                    return view! { p(class="help") { "No timing observations yet." } };
                }
                let views: Vec<View> = runs
                    .iter()
                    .map(|r| {
                        let uid = r.uid.clone();
                        let uid_edit = uid.clone();
                        let uid_void = uid.clone();
                        let (icon_char, icon_class) = if r.r#type == RUN_START {
                            ("\u{25B6}", "has-text-success")
                        } else if r.r#type == RUN_STOP {
                            ("\u{23F9}", "has-text-danger")
                        } else {
                            ("\u{25A0}", "")
                        };
                        let car_text = format!(" #{}", r.car);
                        let ts = super::super::helpers::fmt_ts(r.ts, now);
                        let time_text = match r.time_ds {
                            Some(ds) => format!("{:.1}", ds as f32 / 10.0),
                            None => {
                                let status = r.status.as_deref().unwrap_or("—");
                                status.to_string()
                            }
                        };
                        let official_view: View = match &r.official_id {
                            Some(o) if !o.is_empty() => {
                                let text = format!("by {}", o);
                                view! { span(class="has-text-grey-light ml-2") { (text) } }
                            }
                            _ => view! {},
                        };
                        let comment_view: View = match &r.comment {
                            Some(c) if !c.is_empty() => {
                                let text = format!("\"{}\"", c);
                                view! { span(class="has-text-grey ml-2 is-size-7") { (text) } }
                            }
                            _ => view! {},
                        };
                        view! {
                            div(class="level is-mobile") {
                                div(class="level-left") {
                                    span(class=icon_class) { (icon_char) }
                                    span(class="has-text-weight-semibold") { (car_text) }
                                    span(class="has-text-grey ml-2") { (ts) }
                                    span(class="ml-2") { (time_text) }
                                    (official_view)
                                    (comment_view)
                                }
                                div(class="level-right") {
                                    span(class="buttons are-small") {
                                        button(
                                            class="button is-small is-link is-light",
                                            on:click=move |_| crate::update(model, crate::Msg::TimekeeperMsg(TimekeeperMsg::StartEdit(uid_edit.clone()))),
                                        ) {
                                            span(class="icon is-small") { i(class="fa fa-pen") }
                                        }
                                        button(
                                            class="button is-small is-danger is-light",
                                            on:click=move |_| crate::update(model, crate::Msg::TimekeeperMsg(TimekeeperMsg::Void(uid_void.clone()))),
                                        ) {
                                            span(class="icon is-small") { i(class="fa fa-xmark") }
                                        }
                                    }
                                }
                            }
                        }
                    })
                    .collect();
                view! { (views) }
            })
        }
    }
}

/// Compact results table for the current stage.
fn view_compact_results(model: crate::Model) -> View {
    view! {
        div(class="box") {
            h3(class="title is-6") { "Results" }
            (move || {
                let test = model.screens.timekeeper.stage.get();
                let event = model.khana.event.get_clone();
                let runs = model.khana.runs.get_clone();
                let rv = crate::event::create_outright_view(&event, &runs);
                let mut rows: Vec<(&String, &crate::event::ResultRow)> = rv.rows.iter().collect();
                rows.sort_by(|a, b| crate::khana::page::results::cmp_car(a.1, b.1));
                if rows.is_empty() {
                    return view! { p(class="help") { "No results yet." } };
                }
                let header = view! {
                    tr {
                        th { "#" }
                        th { "Driver" }
                        th { "Time" }
                        th { "Pos" }
                    }
                };
                let row_views: Vec<View> = rows
                    .iter()
                    .enumerate()
                    .map(|(i, (_car, rr))| {
                        let car = show::car_tag(&rr.entry.car);
                        let name = rr.entry.name.clone();
                        let col = rr.columns.get(test as usize - 1);
                        let (time_text, pos_text) = match col {
                            Some(Some(rs)) => {
                                let time = show::ktime(&rs.runs.first().map(|r| r.time.clone()).unwrap_or(crate::event::KTime::NOSHO));
                                let pos = rs.stage_pos.as_ref().map(|p| p.pos.to_string()).unwrap_or_else(|| "\u{2014}".into());
                                (time, pos)
                            }
                            _ => ("\u{2014}".into(), "\u{2014}".into()),
                        };
                        view! {
                            tr {
                                td { (i + 1) }
                                td { (car) }
                                td { (name) }
                                td { (time_text) }
                                td { (pos_text) }
                            }
                        }
                    })
                    .collect();
                view! {
                    div(class="table-container") {
                        table(class="table is-narrow is-fullwidth") {
                            (header)
                            (row_views)
                        }
                    }
                }
            })
        }
    }
}

fn input_box_wrap(model: crate::Model) -> View {
    let dispatch = move |msg: InputMsg| {
        crate::update(
            model,
            crate::Msg::TimekeeperMsg(TimekeeperMsg::CmdInput(msg)),
        )
    };
    view! {
        div(class="pannel-block") {
            p(class="control has-icons-left") {
                (input_box(
                    model.screens.timekeeper.cmd,
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
