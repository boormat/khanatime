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

/// Batch-edit confirmation modal: a diff list with Send / Keep editing /
/// Discard.  Rendered (as Bulma `.modal.is-active`) while `confirm` is `Some`.
pub fn view_confirm_modal(
    confirm: Signal<Option<Vec<String>>>,
    send_label: &'static str,
    send: impl Fn() + 'static,
    keep: impl Fn() + 'static,
    discard: impl Fn() + 'static,
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
                    view! {
                        div(class="modal is-active") {
                            div(class="modal-background")
                            div(class="modal-card") {
                                header(class="modal-card-head") {
                                    p(class="modal-card-title") { "Confirm changes" }
                                }
                                section(class="modal-card-body") {
                                    ul {
                                        (lines.iter().map(|l| {
                                            let text = l.clone();
                                            view! { li { (text) } }
                                        }).collect::<Vec<View>>())
                                    }
                                }
                                footer(class="modal-card-foot") {
                                    button(class="button is-primary", on:click=move |_| send()) {
                                        (send_label)
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
