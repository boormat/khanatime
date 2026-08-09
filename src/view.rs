// helpers for rendering things to Html/Nodes
use crate::event::*;
use sycamore::prelude::*;

pub fn ktime(time: &KTime) -> View {
    match time {
        KTime::Time(t) => show_ktimetime(t),
        KTime::NOSHO => view! { div(class="tag is-black") { "DNS" } },
        KTime::WD => view! { div(class="tag is-black") { "WD" } },
        KTime::FTS => view! { div(class="tag is-black") { "FTS" } },
        KTime::DNF => view! { div(class="tag is-black") { "DNF" } },
    }
}

pub fn show_ktimetime(time: &KTimeTime) -> View {
    let ts = format!("{:.1}", time.time_ds as f32 / 10.0);
    let mut icons: Vec<View> = vec![];
    if time.garage {
        icons.push(view! { i(class="fa fa-warehouse") });
    }
    for _ in 0..time.flags {
        icons.push(view! { i(class="fa fa-flag") });
    }
    view! { div { span { (ts) } (icons) } }
}

pub fn car_number(car: String) -> View {
    view! { span(class="label label-default") { (car) } }
}
