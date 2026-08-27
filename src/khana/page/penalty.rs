use sycamore::prelude::*;

use crate::event::{KTime, KTimeTime};

// Finish penalty entry: status chips + flag counter (+5s each) + garage toggle,
// with a live net-time preview. Shared by Finish screen and stopwatch.

#[derive(Clone, Copy)]
pub struct PenaltyModel {
    pub flags: Signal<u8>,
    pub garage: Signal<bool>,
    pub status: Signal<String>, // "clean" | "dnf" | "fts" | "wd"
}

pub fn init() -> PenaltyModel {
    PenaltyModel {
        flags: create_signal(0),
        garage: create_signal(false),
        status: create_signal("clean".to_string()),
    }
}

pub fn clear(model: PenaltyModel) {
    model.flags.set(0);
    model.garage.set(false);
    model.status.set("clean".to_string());
}

/// Elapsed + flag/garage penalties, deciseconds.
pub fn net_ds(time_ds: u16, flags: u8, garage: bool) -> u32 {
    (time_ds as u32) + (50 * (flags as u32 + garage as u32))
}

/// The [KTime] the penalty panel represents for the scores model.
pub fn to_ktime(model: PenaltyModel, time_ds: u16) -> KTime {
    match model.status.get_clone().as_str() {
        "dnf" => KTime::DNF,
        "fts" => KTime::FTS,
        "wd" => KTime::WD,
        _ => KTime::Time(KTimeTime {
            time_ds,
            flags: model.flags.get(),
            garage: model.garage.get(),
        }),
    }
}

pub const STATUS_CHIPS: [(&str, &str, &str); 3] = [
    ("dnf", "DNF", "is-danger"),
    ("fts", "FTS", "is-danger"),
    ("wd", "WD", "is-danger"),
];

pub fn view(app: crate::Model, p: PenaltyModel, time_ds: u16) -> View {
    view! {
        div(class="box") {
            h3(class="title is-6") {
                "Penalties"
                a(
                    class="is-pulled-right",
                    on:click=move |_| crate::update(app, crate::Msg::Show(crate::Screen::KhanaRules)),
                ) {
                    "What counts? "
                    i(class="fa fa-circle-question")
                }
            }
            div(class="field is-grouped is-grouped-multiline") {
                (move || {
                    let cur = p.status.get_clone();
                    let views: Vec<View> = STATUS_CHIPS
                        .iter()
                        .map(|(val, label, cls)| {
                            let val = *val;
                            let label = *label;
                            let cls = *cls;
                            let active = cur == val;
                            view! {
                                button(
                                    class=format!(
                                        "button is-small {}",
                                        if active { cls } else { "is-light" }
                                    ),
                                    on:click=move |_| {
                                        if active {
                                            p.status.set("clean".to_string());
                                        } else {
                                            p.status.set(val.to_string());
                                        }
                                    },
                                ) {
                                    (label)
                                }
                            }
                        })
                        .collect();
                    let views: View = views.into();
                    views
                })
            }
            div(class="field is-grouped") {
                div(class="control") {
                    button(
                        class="button",
                        disabled=move || p.flags.get() == 0,
                        on:click=move |_| p.flags.update(|f| *f = f.saturating_sub(1)),
                    ) { "−" }
                }
                div(class="control is-expanded") {
                    div(class="has-text-centered") {
                        (move || {
                            format!("{} flag(s)  +5s each", p.flags.get())
                        })
                    }
                }
                div(class="control") {
                    button(
                        class="button",
                        disabled=move || p.flags.get() >= 9,
                        on:click=move |_| p.flags.update(|f| *f += 1),
                    ) { "+" }
                }
            }
            div(class="field is-grouped is-grouped-centered") {
                (move || {
                    let on = p.garage.get();
                    view! {
                        button(
                            class=format!("button is-small {}", if on { "is-warning" } else { "is-light" }),
                            on:click=move |_| p.garage.set(!p.garage.get()),
                        ) {
                            span(class="icon is-small") { i(class="fa fa-warehouse") }
                            span { " Garage (+5s)" }
                        }
                    }
                })
            }
            div(class="field") {
                (move || {
                    let status = p.status.get_clone();
                    if status == "dnf" || status == "fts" || status == "wd" {
                        let upper = status.to_uppercase();
                        view! {
                            div(class="notification is-warning is-light has-text-centered") {
                                ("Result: ") (upper)
                            }
                        }
                    } else {
                        let raw = format!("{:.1}", time_ds as f32 / 10.0);
                        let net = format!("{:.1}", net_ds(time_ds, p.flags.get(), p.garage.get()) as f32 / 10.0);
                        let penalized = net_ds(time_ds, p.flags.get(), p.garage.get()) > time_ds as u32;
                        view! {
                            div(class=if penalized {
                                "notification is-warning is-light has-text-centered"
                            } else {
                                "notification is-success is-light has-text-centered"
                            }) {
                                ("Elapsed ") (raw) (" s → ") ("Net ") (net) (" s")
                            }
                        }
                    }
                })
            }
        }
    }
}
