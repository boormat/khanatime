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
    car_tag(&car)
}

/// Car number tag with fa-car icon — consistent across all views.
pub fn car_tag(car: &str) -> View {
    if car.is_empty() {
        view! { span(class="tag is-light kt-car-tag") { i(class="fa fa-car") { " ?" } } }
    } else {
        let c = car.to_string();
        view! { span(class="tag is-black kt-car-tag") { i(class="fa fa-car") { " " } (c) } }
    }
}

/// Class tag — consistent styling for class badges.
pub fn class_tag(class: &str) -> View {
    let c = class.to_string();
    view! { span(class="tag is-info is-light is-small") { (c) } }
}

/// Entrant summary line — car + name + classes (for lists).
pub fn entrant_summary(car: &str, name: &str, classes: &[String]) -> View {
    let tags: Vec<View> = classes.iter().map(|c| class_tag(c)).collect();
    let n = name.to_string();
    let car_view = car_tag(car);
    view! {
        span(class="kt-entrant-line") {
            (car_view)
            span { (n) }
            (tags)
        }
    }
}

/// Entrant detail line — car + name + vehicle · shared (for full view).
pub fn entrant_detail(car: &str, name: &str, vehicle: &str, shared: &str) -> View {
    let mut info = Vec::new();
    if !vehicle.is_empty() {
        info.push(vehicle.to_string());
    }
    if !shared.is_empty() {
        info.push(format!("Shared: {}", shared));
    }
    let n = name.to_string();
    let info_text = info.join(" \u{00b7} ");
    let car_view = car_tag(car);
    let has_info = !info_text.is_empty();
    view! {
        span(class="kt-entrant-line") {
            (car_view)
            span { (n) }
            (if has_info {
                let t = info_text.clone();
                view! { span(class="has-text-grey is-size-7") { (t) } }
            } else {
                view! {}
            })
        }
    }
}

/// Batch-edit confirmation modal: a diff list with Send / Keep editing /
/// Discard.  Rendered (as Bulma `.modal.is-active`) while `confirm` is `Some`.
/// `send_label` is re-evaluated reactively; `warning` (if non-empty) is shown
/// above the diff list.
pub fn view_confirm_modal(
    confirm: Signal<Option<Vec<String>>>,
    send_label: impl Fn() -> String + 'static,
    send: impl Fn() + 'static,
    keep: impl Fn() + 'static,
    discard: impl Fn() + 'static,
    warning: Signal<String>,
) -> View {
    use std::rc::Rc;
    let send = Rc::new(send);
    let keep = Rc::new(keep);
    let discard = Rc::new(discard);
    view! {
        (move || {
            match confirm.get_clone() {
                None => view! {},
                Some(lines) => {
                    let send = send.clone();
                    let keep = keep.clone();
                    let discard = discard.clone();
                    let label = send_label();
                    let warning = warning.get_clone();
                    let warning_view = if warning.is_empty() {
                        view! {}
                    } else {
                        view! { p(class="help is-warning") { (warning) } }
                    };
                    view! {
                        div(class="modal is-active") {
                            div(class="modal-background")
                            div(class="modal-card") {
                                header(class="modal-card-head") {
                                    p(class="modal-card-title") { "Confirm changes" }
                                }
                                section(class="modal-card-body") {
                                    (warning_view)
                                    ul {
                                        (lines.iter().map(|l| {
                                            let text = l.clone();
                                            view! { li { (text) } }
                                        }).collect::<Vec<View>>())
                                    }
                                }
                                footer(class="modal-card-foot") {
                                    button(class="button is-primary", on:click=move |_| send()) {
                                        (label)
                                    }
                                    button(class="button", on:click=move |_| keep()) {
                                        "Keep editing"
                                    }
                                    button(class="button is-danger is-outlined", on:click=move |_| discard()) {
                                        "Discard"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        })
    }
}
