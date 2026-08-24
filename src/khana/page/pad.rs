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
                    class="button is-light",
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
            class="button is-warning",
            on:click=move |_| digits.set("?".to_string()),
        ) {
            span(class="kt-car-tag has-text-weight-semibold") { "?" }
            span(class="ml-2") { "Unknown" }
        }
    });
    view! { div(class="field is-grouped is-grouped-multiline") { (chips) } }
}

/// Compact car-number-only chips grouped by runs remaining, sorted descending.
/// Tag badge separators between groups; TBA chip at the very end.
pub fn car_chips_compact(
    entries: Vec<crate::event::Entry>,
    digits: Signal<String>,
    runs_remaining: &HashMap<String, u8>,
) -> View {
    use crate::event::cmp_car_number;

    struct CarInfo {
        car: String,
        remaining: u8,
    }
    let mut cars: Vec<CarInfo> = entries
        .iter()
        .filter(|e| !e.car.is_empty())
        .map(|e| CarInfo {
            car: e.car.clone(),
            remaining: *runs_remaining.get(&e.car).unwrap_or(&0),
        })
        .collect();
    cars.sort_by(|a, b| {
        b.remaining
            .cmp(&a.remaining)
            .then_with(|| cmp_car_number(&a.car, &b.car))
    });

    // Group by remaining count.
    let mut groups: HashMap<u8, Vec<&CarInfo>> = HashMap::new();
    for c in &cars {
        groups.entry(c.remaining).or_default().push(c);
    }
    let mut group_keys: Vec<u8> = groups.keys().copied().collect();
    group_keys.sort_by(|a, b| b.cmp(a));

    let mut rows: Vec<View> = Vec::new();
    for remaining in group_keys {
        let car_list = groups.remove(&remaining).unwrap();
        let badge_label = format!("{remaining}r");
        let car_strings: Vec<String> = car_list.iter().map(|ci| ci.car.clone()).collect();
        rows.push(view! {
            div(class="field is-grouped is-grouped-multiline is-align-items-center mb-1") {
                span(class="tag is-small is-link is-light kt-runs-separator") { (badge_label) }
                (car_strings.iter().map(|car| {
                    let car_set = car.clone();
                    let car_display = car.clone();
                    view! {
                        button(
                            class="button is-light is-small",
                            on:click=move |_| digits.set(car_set.clone()),
                        ) {
                            span(class="kt-car-tag has-text-weight-semibold") { (car_display) }
                        }
                    }
                }).collect::<Vec<View>>())
            }
        });
    }

    // TBA chip at the very end.
    rows.push(view! {
        div(class="field is-grouped is-grouped-multiline is-align-items-center mb-1") {
            span(class="tag is-small is-light kt-runs-separator") { "TBA" }
            button(
                class="button is-warning is-small",
                on:click=move |_| digits.set("?".to_string()),
            ) {
                span(class="kt-car-tag has-text-weight-semibold") { "?" }
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
                        "button {}",
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
