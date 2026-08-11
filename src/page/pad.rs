use sycamore::prelude::*;

/// On-screen number keypad bound to `digits` (capped at `cap` digits).
/// A big, touch-friendly 3x4 grid: 1-9, then backspace / 0 / clear.
pub fn keypad(digits: Signal<String>, cap: usize) -> View {
    let push = move |d: &'static str| {
        digits.update(|s| {
            if s.chars().count() < cap {
                s.push_str(d);
            }
        });
    };
    let back = move |_| {
        digits.update(|s| {
            s.pop();
        })
    };
    let clear = move |_| digits.set(String::new());

    let rows: Vec<Vec<&str>> = vec![
        vec!["1", "2", "3"],
        vec!["4", "5", "6"],
        vec!["7", "8", "9"],
    ];
    let grid: Vec<View> = rows
        .iter()
        .map(|row| {
            let btns: Vec<View> = row
                .iter()
                .map(|d| {
                    let d = *d;
                    view! {
                        div(class="control is-expanded") {
                            button(class="button is-large", on:click=move |_| push(d)) { (d) }
                        }
                    }
                })
                .collect();
            view! { div(class="field is-grouped") { (btns) } }
        })
        .collect();

    view! {
        div {
            div(class="field") {
                div(class="control") {
                    (move || view! {
                        input(
                            class="input has-text-centered is-large",
                            value=digits.get_clone(),
                            disabled=true,
                        )
                    })
                }
            }
            (grid)
            div(class="field is-grouped") {
                div(class="control") {
                    button(class="button is-large", on:click=back) { i(class="fa fa-delete-left") }
                }
                div(class="control is-expanded") {
                    button(class="button is-large", on:click=move |_| push("0")) { "0" }
                }
                div(class="control") {
                    button(class="button is-large", on:click=clear) { "CLR" }
                }
            }
        }
    }
}

/// Quick-pick chips for registered cars, tap to fill `digits`.
pub fn car_chips(entries: Vec<crate::event::Entry>, digits: Signal<String>) -> View {
    if entries.is_empty() {
        return view! { p(class="help") { "No entries yet — add cars on the Event page." } };
    }
    let chips: Vec<View> = entries
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
                    span(class="has-text-weight-semibold") { ("#") (car) }
                    span(class="ml-2") { (name) }
                }
            }
        })
        .collect();
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
