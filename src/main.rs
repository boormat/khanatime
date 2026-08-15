mod app;
mod batch;
mod event;
mod input;
mod log;
mod page;
mod replay;
mod services;
mod sync;
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
        app::show(model, initial_screen());
        #[cfg(target_arch = "wasm32")]
        sync::resume_on_load(model);
        app::view(model)
    });
}

/// Screen to land on after a reload: the one named by the URL hash (trunk's
/// livereload force-reloads when the dev server restarts, and a plain refresh
/// should keep your place too).  Falls back to the warm-start default when the
/// URL has nothing usable.
fn initial_screen() -> Screen {
    #[cfg(target_arch = "wasm32")]
    if let Some(screen) = app::screen_from_url() {
        let has_event = !crate::event::session_event_name().is_empty();
        if !screen.needs_event() || has_event {
            return screen;
        }
    }
    if warm_start() {
        Screen::Results
    } else {
        Screen::Home
    }
}

/// True when we already have a persisted Matrix session and a session event:
/// most users just want to look at the standings, so land straight on Results.
fn warm_start() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        let session = crate::services::matrix::load_session().is_some();
        let event = crate::event::session_event_name();
        let has_event = !event.is_empty();
        session && has_event
    }
    #[cfg(not(target_arch = "wasm32"))]
    false
}
