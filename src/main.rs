mod app;
mod batch;
mod event;
mod ids;
mod input;
mod log;
mod page;
#[cfg(target_arch = "wasm32")]
mod qr_scan;
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
        #[cfg(target_arch = "wasm32")]
        let sso_callback = oauth_callback().is_some();
        #[cfg(not(target_arch = "wasm32"))]
        let sso_callback = false;
        if !sso_callback {
            app::show(model, initial_screen());
            #[cfg(target_arch = "wasm32")]
            sync::resume_on_load(model);
        } else {
            // OAuth/SSO callback tab: it posted the result to the initiating
            // tab over BroadcastChannel and is closing itself.
            app::show(model, Screen::Home);
        }
        app::view(model)
    });
}

/// Handle an OAuth/SSO callback in the sign-in tab.  The homeserver redirected
/// here with `?code=…&state=…` (or `?error=…&state=…`); post the full callback
/// URL to the initiating tab over a BroadcastChannel named by `state`, then
/// close this tab.  Returns `Some(())` when the URL was an OAuth callback.
#[cfg(target_arch = "wasm32")]
fn oauth_callback() -> Option<()> {
    let window = web_sys::window()?;
    let location = window.location();
    let href = location.href().ok()?;
    let url = url::Url::parse(&href).ok()?;
    let query: std::collections::HashMap<String, String> = url.query_pairs().into_owned().collect();
    let state = query.get("state")?;
    let is_callback = query.contains_key("code") || query.contains_key("error");
    if !is_callback {
        return None;
    }
    let channel = web_sys::BroadcastChannel::new(state).ok()?;
    let _ = channel.post_message(&wasm_bindgen::JsValue::from_str(&href));
    channel.close();
    let _ = window.close();
    Some(())
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
