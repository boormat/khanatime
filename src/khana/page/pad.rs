use std::collections::HashMap;

use sycamore::prelude::*;

/// Quick-pick chips for registered cars, tap to fill `digits`.
/// Includes an "Unknown" chip at the end for cars not yet identified.
pub fn car_chips(entries: Vec<crate::event::Entry>, digits: Signal<String>) -> View {
    let mut chips: Vec<View> = entries
        .iter()
        .map(|e| {
            let car = e.car.clone();
            let name = e.name.clone();
            let car_set = car.clone();
            view! {
                button(
                    class="button is-small is-light",
                    on:click=move |_| digits.set(car_set.clone()),
                ) {
                    span(class="kt-car-tag has-text-weight-semibold") { (car) }
                    span(class="ml-2") { (name) }
                }
            }
        })
        .collect();
    chips.push(view! {
        button(
            class="button is-small is-warning",
            on:click=move |_| digits.set("?".to_string()),
        ) {
            span(class="kt-car-tag has-text-weight-semibold") { "?" }
            span(class="ml-2") { "Unknown" }
        }
    });
    view! { div(class="field is-grouped is-grouped-multiline") { (chips) } }
}

/// Car chips grouped by runs remaining (most remaining first), sorted by car
/// number within each group (unknown "?" at end).  Each group is labelled and
/// includes a `(Nr)` badge on each chip showing remaining runs.
pub fn car_chips_with_runs(
    entries: Vec<crate::event::Entry>,
    digits: Signal<String>,
    runs_remaining: &HashMap<String, u8>,
    unknown_remaining: u8,
) -> View {
    use crate::event::cmp_car_number;

    // Build (car, name, remaining) for each entry, plus the unknown chip.
    struct CarInfo {
        car: String,
        name: String,
        remaining: u8,
    }
    let mut cars: Vec<CarInfo> = entries
        .iter()
        .filter(|e| !e.car.is_empty())
        .map(|e| CarInfo {
            car: e.car.clone(),
            name: e.name.clone(),
            remaining: *runs_remaining.get(&e.car).unwrap_or(&0),
        })
        .collect();
    cars.sort_by(|a, b| cmp_car_number(&a.car, &b.car));

    // Group by remaining count.
    let mut groups: HashMap<u8, Vec<&CarInfo>> = HashMap::new();
    for c in &cars {
        groups.entry(c.remaining).or_default().push(c);
    }
    // Sort groups descending by remaining count.
    let mut group_keys: Vec<u8> = groups.keys().copied().collect();
    group_keys.sort_by(|a, b| b.cmp(a));

    let mut rows: Vec<View> = Vec::new();
    for remaining in group_keys {
        let car_list = groups.remove(&remaining).unwrap();
        let label_text = format!("{remaining} run{}", if remaining != 1 { "s" } else { "" });
        let chips: Vec<View> = car_list
            .iter()
            .map(|ci| {
                let car = ci.car.clone();
                let name = ci.name.clone();
                let car_set = car.clone();
                let badge = format!("({remaining}r)");
                view! {
                    button(
                        class="button is-small is-light",
                        on:click=move |_| digits.set(car_set.clone()),
                    ) {
                        span(class="kt-car-tag has-text-weight-semibold") { (car) }
                        span(class="ml-2") { (name) }
                        span(class="tag is-small ml-1 is-link is-light") { (badge) }
                    }
                }
            })
            .collect();
        rows.push(view! {
            p(class="help has-text-weight-semibold mb-1") { (label_text) }
            div(class="field is-grouped is-grouped-multiline") { (chips) }
        });
    }

    // Unknown chip in its own row at the bottom.
    let u_badge = format!("({unknown_remaining}r)");
    rows.push(view! {
        p(class="help has-text-weight-semibold mb-1") { "Unknown" }
        div(class="field is-grouped is-grouped-multiline") {
            button(
                class="button is-small is-warning",
                on:click=move |_| digits.set("?".to_string()),
            ) {
                span(class="kt-car-tag has-text-weight-semibold") { "?" }
                span(class="ml-2") { "Unknown" }
                span(class="tag is-small ml-1 is-light") { (u_badge) }
            }
        }
    });

    view! { div { (rows) } }
}

/// Number chips for picking a test (1..=count).
pub fn test_chips(count: u8, current: Signal<u8>) -> View {
    let chips: Vec<View> = (1..=count)
        .map(|t| {
            let active = current.get() == t;
            view! {
                button(
                    class=format!(
                        "button is-small {}",
                        if active { "is-primary" } else { "is-light" }
                    ),
                    on:click=move |_| current.set(t),
                ) {
                    (format!("Test {t}"))
                }
            }
        })
        .collect();
    view! { div(class="field is-grouped is-grouped-multiline") { (chips) } }
}
