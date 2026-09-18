mod app;
mod ids;
mod input;
mod join;
mod khana;
mod log;
mod page;
#[cfg(target_arch = "wasm32")]
mod qr_scan;
mod services;
pub mod signing;
mod sync;

pub use khanatime::APP_VERSION;

pub use app::{ConnState, Model, Msg, Screen};

// Re-export moved modules so existing `crate::event::*` and similar paths
// keep working during the transition period.
pub mod event {
    pub use crate::khana::event::*;
}
pub mod timing_event {
    pub use crate::khana::timing_event::*;
}
pub mod view {
    pub use crate::khana::view::*;
}
pub mod batch {
    pub use crate::khana::batch::*;
}
pub mod replay {
    pub use crate::khana::replay::*;
}

// WASM test harness: `cargo test --target wasm32-unknown-unknown` runs every
// `#[wasm_bindgen_test]` in a headless browser, so wasm-only paths
// (localStorage, DOM, matrix-sdk) are covered too.  The native suite
// (`cargo test`) covers pure logic; the two are complementary.  Declared once
// per test binary — `signing` is compiled into both lib and bin, hence the
// matching block in lib.rs.
#[cfg(all(test, target_arch = "wasm32"))]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

use sycamore::prelude::*;
use sycamore::render;

pub fn update(model: Model, msg: Msg) {
    app::update(model, msg);
}

fn main() {
    // Ensure the device signing keypair exists before any code tries to sign.
    #[cfg(target_arch = "wasm32")]
    {
        crate::signing::DeviceKeys::load_or_generate();
    }

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
            // A join link overrides the persisted session: consume the query,
            // show Home (conn status visible) and start the join.
            let handled = {
                #[cfg(target_arch = "wasm32")]
                {
                    if let Some(link) = join::from_location() {
                        join::consume();
                        app::show(model, Screen::Home);
                        crate::update(model, Msg::Join(link));
                        true
                    } else if let Some((hs, user, pass)) = join::from_location_account() {
                        join::consume();
                        crate::update(
                            model,
                            Msg::ImportAccount {
                                homeserver: hs,
                                user_id: user,
                                password: pass,
                            },
                        );
                        true
                    } else if let Some((uid, name, desc, phone)) = join::from_location_contact() {
                        join::consume();
                        crate::update(
                            model,
                            Msg::ImportContact {
                                user_id: uid,
                                name,
                                description: desc,
                                phone,
                            },
                        );
                        true
                    } else {
                        false
                    }
                }
                #[cfg(not(target_arch = "wasm32"))]
                {
                    false
                }
            };
            if !handled {
                app::show(model, initial_screen());
                #[cfg(target_arch = "wasm32")]
                sync::resume_on_load(model);
            }
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
        let has_event = !crate::khana::event::session_event_name().is_empty();
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

/// True when we already have a *currently active* Matrix session and a session
/// event: most users just want to look at the standings, so land straight on
/// Results.  A soft logout clears the active pointer, so a deactivated session
/// lands on Home (the sign-in / accounts screen) instead.
fn warm_start() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        let active = crate::services::matrix::active_hs().is_some();
        let event = crate::khana::event::session_event_name();
        let has_event = !event.is_empty();
        active && has_event
    }
    #[cfg(not(target_arch = "wasm32"))]
    false
}
