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

// Browser-only coverage: these read `window.location` / `history`, unreachable
// from the native suite.  Each test seeds a query via `history.replaceState`
// and restores a clean query afterwards so runs don't leak into one another.
#[cfg(all(test, target_arch = "wasm32"))]
mod wasm_tests {
    use super::*;
    use wasm_bindgen_test::*;

    /// Push `query` (without leading `?`) into the current URL, return a guard
    /// that clears it on drop.
    struct QueryGuard;

    impl QueryGuard {
        fn set(q: &str) -> Self {
            if let Some(w) = web_sys::window() {
                let path = w.location().pathname().unwrap_or_default();
                if let Ok(h) = w.history() {
                    let _ = h.replace_state_with_url(
                        &wasm_bindgen::JsValue::NULL,
                        "",
                        Some(&format!("{path}?{q}")),
                    );
                }
            }
            QueryGuard
        }
    }

    impl Drop for QueryGuard {
        fn drop(&mut self) {
            if let Some(w) = web_sys::window() {
                let path = w.location().pathname().unwrap_or_default();
                if let Ok(h) = w.history() {
                    let _ = h.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&path));
                }
            }
        }
    }

    #[wasm_bindgen_test]
    fn clean_url_returns_no_invite() {
        let _g = QueryGuard::set("");
        assert!(from_location().is_none());
        assert!(from_location_account().is_none());
        assert!(from_location_contact().is_none());
    }

    #[wasm_bindgen_test]
    fn from_location_parses_full_invite() {
        let _g = QueryGuard::set(
            "homeserver=https://hs.local&event=ev1&sid=!space&tid=!timing&reg=open",
        );
        let inv = from_location().expect("invite present");
        assert_eq!(inv.homeserver, "https://hs.local");
        assert_eq!(inv.event, "ev1");
        assert_eq!(inv.sid, "!space");
        assert_eq!(inv.tid, "!timing");
        assert_eq!(inv.reg, crate::event::RegistrationMode::Open);
    }

    #[wasm_bindgen_test]
    fn from_location_ignores_partial_invite() {
        let _g = QueryGuard::set("homeserver=https://hs.local&event=ev1");
        // Missing sid/tid → guard rejects it.
        assert!(from_location().is_none());
    }

    #[wasm_bindgen_test]
    fn from_location_account_parses_credentials() {
        let _g = QueryGuard::set(
            "type=account&homeserver=https://hs.local&user_id=@bob:hs&password=secret123",
        );
        let (hs, uid, pass) = from_location_account().expect("account present");
        assert_eq!(hs, "https://hs.local");
        assert_eq!(uid, "@bob:hs");
        assert_eq!(pass, "secret123");
    }

    #[wasm_bindgen_test]
    fn from_location_account_requires_type_and_ids() {
        let _g = QueryGuard::set("homeserver=https://hs.local&user_id=@bob:hs");
        // No `type=account` → None.
        assert!(from_location_account().is_none());
    }

    #[wasm_bindgen_test]
    fn from_location_contact_parses_fields() {
        let _g = QueryGuard::set(
            "type=contact&user_id=@carol:hs&name=Carol&description=Timer&phone=0400123456",
        );
        let (uid, name, desc, phone) = from_location_contact().expect("contact present");
        assert_eq!(uid, "@carol:hs");
        assert_eq!(name, "Carol");
        assert_eq!(desc, "Timer");
        assert_eq!(phone.as_deref(), Some("0400123456"));
    }

    #[wasm_bindgen_test]
    fn from_location_contact_omits_empty_phone() {
        let _g = QueryGuard::set("type=contact&user_id=@carol:hs&phone=");
        let (_, _, _, phone) = from_location_contact().expect("contact present");
        assert!(phone.is_none());
    }
}
