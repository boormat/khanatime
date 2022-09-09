// helpers for rendering things to Html/Nodes
use crate::event::*;
use seed::prelude::*;
use seed::{nodes, *};

pub fn ktime<Msg>(time: &KTime) -> Vec<Node<Msg>> {
    // nodes![
    let text = match time {
        KTime::Time(t) => return nodes!(show_ktimetime(t)),
        KTime::NOSHO => "DNS",
        KTime::WD => "WD",
        KTime::FTS => "FTS",
        KTime::DNF => "DNF",
    };
    nodes![div!(C!["tag is-black"], text)]
}

pub fn show_ktimetime<Msg>(time: &KTimeTime) -> Node<Msg> {
    // nodes![
    let f = i![C!["fa fa-flag"]];
    let g = i![C!["fa fa-warehouse"]];
    let fl = vec![f; time.flags as usize];
    let gl = vec![g; time.garage as usize];

    let ts = format!("{:.1}", time.time_ds as f32 / 10.0);
    let t = span!(ts);
    div!(nodes![t, gl, fl])
}

#[allow(dead_code)]
pub fn show_ktimetime_labels<Msg>(time: &KTimeTime) -> Node<Msg> {
    // use labels
    let g = show_garage(time.garage);
    let f = show_flag(time.flags);
    let ts = format!("{:.1}", time.time_ds as f32 / 10.0);
    let t = span!(C!["tag is-rounded "], ts);
    div!(nodes![t, f, g])
}

pub fn show_flag<Msg>(flags: u8) -> Node<Msg> {
    match flags {
        0 => empty(),
        1 => span!(C!["tag is-rounded is-info"], "F"),
        _ => span!(C!["tag is-rounded is-info"], format!("{}F", flags)),
    }
}
pub fn show_garage<Msg>(garage: bool) -> Node<Msg> {
    match garage {
        false => empty(),
        true => span!(C!["tag is-rounded is-info"], "G"),
    }
}

pub fn car_number<Msg>(car: &String) -> Node<Msg> {
    span! {
        C!["label label-default"],
        car
    }
}
