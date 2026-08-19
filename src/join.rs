//! QR / URL join-link arrival.
//!
//! A join link is a URL like `{app-base}?homeserver=..&event=..&sid=..&tid=..&reg=..`
//! (see [`crate::event::Invite`]).  A phone scanning it loads the app with that
//! query; `from_location` reads it and the startup hook hands it to
//! `Msg::Join`, which connects (reusing a stored session or registering/SSO per
//! `reg`), adopts the event by room id and lands on Results.
//!
//! Parsing lives in [`crate::event::Invite::from_query`] (pure + testable);
//! this module only touches `window.location`.

use crate::event::Invite;

/// Read a join invite from `window.location`'s query string.  Accepts only a
/// self-contained invite (homeserver + event + space/timing room ids present),
/// so it never misfires on OAuth callbacks or unrelated query strings.
#[cfg(target_arch = "wasm32")]
pub fn from_location() -> Option<Invite> {
    let search = web_sys::window()?.location().search().ok()?;
    let q = search.strip_prefix('?').unwrap_or(&search);
    let inv = Invite::from_query(q)?;
    if inv.homeserver.is_empty() || inv.event.is_empty() || inv.sid.is_empty() || inv.tid.is_empty()
    {
        return None;
    }
    Some(inv)
}

/// Parse `type=account` from the URL query string, returning
/// `(homeserver, user_id, password)` if present.
#[cfg(target_arch = "wasm32")]
pub fn from_location_account() -> Option<(String, String, String)> {
    let search = web_sys::window()?.location().search().ok()?;
    let q = search.strip_prefix('?').unwrap_or(&search);
    let params: std::collections::HashMap<String, String> = q
        .split('&')
        .filter_map(|pair| {
            let mut kv = pair.splitn(2, '=');
            let k = kv.next()?.to_string();
            let v = kv.next()?.to_string();
            Some((k, v))
        })
        .collect();
    if params.get("type").map(String::as_str) != Some("account") {
        return None;
    }
    let homeserver = params.get("homeserver")?.clone();
    let user_id = params.get("user_id")?.clone();
    let password = params.get("password").cloned().unwrap_or_default();
    Some((homeserver, user_id, password))
}

/// Parse `type=contact` from the URL query string, returning
/// `(user_id, name, description, phone)` if present.
#[cfg(target_arch = "wasm32")]
pub fn from_location_contact() -> Option<(String, String, String, Option<String>)> {
    let search = web_sys::window()?.location().search().ok()?;
    let q = search.strip_prefix('?').unwrap_or(&search);
    let params: std::collections::HashMap<String, String> = q
        .split('&')
        .filter_map(|pair| {
            let mut kv = pair.splitn(2, '=');
            let k = kv.next()?.to_string();
            let v = kv.next()?.to_string();
            Some((k, v))
        })
        .collect();
    if params.get("type").map(String::as_str) != Some("contact") {
        return None;
    }
    let user_id = params.get("user_id")?.clone();
    let name = params.get("name").cloned().unwrap_or_default();
    let description = params.get("description").cloned().unwrap_or_default();
    let phone = params.get("phone").cloned().filter(|s| !s.is_empty());
    Some((user_id, name, description, phone))
}

/// Clear the join query from the URL so a refresh / navigation doesn't re-join.
#[cfg(target_arch = "wasm32")]
pub fn consume() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let path = window.location().pathname().unwrap_or_default();
    if let Ok(history) = window.history() {
        let _ = history.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&path));
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub fn from_location() -> Option<Invite> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub fn from_location_account() -> Option<(String, String, String)> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub fn from_location_contact() -> Option<(String, String, String, Option<String>)> {
    None
}
