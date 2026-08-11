mod app;
mod event;
mod input;
mod page;
mod services;
mod timing_event;
mod view;

pub use app::{ConnState, Model, Msg, Screen};

use sycamore::prelude::*;
use sycamore::render;

pub fn update(model: Model, msg: Msg) {
    app::update(model, msg);
}

fn main() {
    std::panic::set_hook(Box::new(|info| {
        let js_stack = js_sys::Reflect::get(&js_sys::Error::new("panic"), &"stack".into())
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_else(|| "no stack".to_string());
        let msg = format!("PANIC: {info}\nJS STACK:\n{js_stack}");
        khanatime::web_log(&msg);
        if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
            if let Some(body) = doc.body() {
                body.set_inner_html(&format!("<pre>{}</pre>", msg.replace('<', "&lt;")));
            }
        }
    }));
    render(move || {
        let model = Model::init();
        app::setup_effects(model);
        let start = if warm_start() {
            Screen::Results
        } else {
            Screen::Home
        };
        app::show(model, start);
        #[cfg(target_arch = "wasm32")]
        page::sync::resume_on_load(model);
        app::view(model)
    });
}

/// True when we already have a persisted Matrix session and a session event:
/// most users just want to look at the standings, so land straight on Results.
fn warm_start() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        let session = crate::services::matrix::load_session().is_some();
        let event = crate::event::session_event_name();
        let has_event = !event.is_empty();
        return session && has_event;
    }
    #[cfg(not(target_arch = "wasm32"))]
    false
}
