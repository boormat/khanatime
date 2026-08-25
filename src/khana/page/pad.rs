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
