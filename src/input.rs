use serde::{Deserialize, Serialize};
use sycamore::prelude::*;

/// Text input state, held as signals so two-way `bind:value` works.
#[derive(Clone, Copy)]
pub struct InputModel {
    pub key: Signal<String>,
    pub input: Signal<String>,
    pub feedback: Signal<String>,
}

pub fn init() -> InputModel {
    InputModel {
        key: create_signal(String::new()),
        input: create_signal(String::new()),
        feedback: create_signal(String::new()),
    }
}

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum InputMsg {
    DoThing,
    CancelEdit,
}

/// A keyboard-friendly single line input.
/// Enter dispatches `DoThing`, Escape dispatches `CancelEdit`.
/// The text is two-way bound to `model.input`.
pub fn input_box(
    model: InputModel,
    placeholder: &'static str,
    dispatch: impl Fn(InputMsg) + 'static,
) -> View {
    view! {
        div {
            div { (move || model.feedback.get_clone()) }
            input(
                class="input",
                placeholder=placeholder,
                bind:value=model.input,
                on:keydown=move |ev: web_sys::KeyboardEvent| {
                    match ev.key_code() {
                        13 => dispatch(InputMsg::DoThing),
                        27 => dispatch(InputMsg::CancelEdit),
                        _ => {}
                    }
                },
            )
        }
    }
}

pub fn input_clear(model: InputModel) {
    model.key.set(String::new());
    model.input.set(String::new());
    model.feedback.set(String::new());
}

pub fn input_value(ev: &web_sys::Event) -> String {
    use wasm_bindgen::JsCast;
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|e| e.value())
        .unwrap_or_default()
}

pub fn select_value(ev: &web_sys::Event) -> String {
    use wasm_bindgen::JsCast;
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlSelectElement>().ok())
        .map(|e| e.value())
        .unwrap_or_default()
}

#[allow(dead_code)]
pub fn input_feedback(model: InputModel, msg: &str) {
    model.feedback.set(msg.to_string());
}
