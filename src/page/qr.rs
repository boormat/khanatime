use sycamore::prelude::*;

/// Minimal percent-decode for query-string values.
fn urldecode(s: &str) -> Option<String> {
    let s = s.replace('+', " ");
    let mut out = Vec::with_capacity(s.len());
    let mut chars = s.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let hi = hex_val(chars.next()?)?;
            let lo = hex_val(chars.next()?)?;
            out.push(hi * 16 + lo);
        } else {
            out.push(b);
        }
    }
    String::from_utf8(out).ok()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Parsed QR URL ready for confirmation.
#[derive(Clone)]
pub enum ParsedQr {
    Account {
        homeserver: String,
        user_id: String,
        password: String,
    },
    Contact {
        user_id: String,
        name: String,
        description: String,
        phone: Option<String>,
    },
    Invite(crate::event::Invite),
}

#[derive(Clone, Copy)]
pub struct Model {
    pub url_input: Signal<String>,
    pub pending: Signal<Option<ParsedQr>>,
    pub error: Signal<String>,
}

impl Model {
    pub fn new() -> Self {
        Self {
            url_input: create_signal(String::new()),
            pending: create_signal(None),
            error: create_signal(String::new()),
        }
    }
}

pub fn init() -> Model {
    Model::new()
}

/// Try to parse a QR URL string into a [`ParsedQr`].
pub fn parse_qr_url(text: &str) -> Option<ParsedQr> {
    let query_str = text.split('?').nth(1).unwrap_or(text);
    let params: std::collections::HashMap<String, String> = query_str
        .split('&')
        .filter_map(|pair| {
            let mut kv = pair.splitn(2, '=');
            let k = urldecode(kv.next()?)?;
            let v = urldecode(kv.next().unwrap_or(""))?;
            Some((k, v))
        })
        .collect();
    if let Some(ty) = params.get("type").map(String::as_str) {
        match ty {
            "account" => {
                let homeserver = params.get("homeserver")?.clone();
                let user_id = params.get("user_id")?.clone();
                let password = params.get("password").cloned().unwrap_or_default();
                return Some(ParsedQr::Account {
                    homeserver,
                    user_id,
                    password,
                });
            }
            "contact" => {
                let user_id = params.get("user_id")?.clone();
                let name = params.get("name").cloned().unwrap_or_default();
                let description = params.get("description").cloned().unwrap_or_default();
                let phone = params.get("phone").cloned().filter(|s| !s.is_empty());
                return Some(ParsedQr::Contact {
                    user_id,
                    name,
                    description,
                    phone,
                });
            }
            _ => {}
        }
    }
    let inv = crate::event::Invite::from_url(text).filter(|inv| {
        !inv.homeserver.is_empty()
            && !inv.event.is_empty()
            && !inv.sid.is_empty()
            && !inv.tid.is_empty()
    })?;
    Some(ParsedQr::Invite(inv))
}

pub fn view(model: crate::Model) -> View {
    let sm = model.screens.qr;
    // Watch for camera scan results in preview mode.
    let scan_preview = model.sync.scan_preview;
    create_effect(move || {
        if let Some(text) = scan_preview.get_clone() {
            scan_preview.set(None);
            match parse_qr_url(&text) {
                Some(parsed) => sm.pending.set(Some(parsed)),
                None => sm
                    .error
                    .set("Scanned text is not a recognised QR URL.".into()),
            }
        }
    });
    view! {
        div(class="section") {
            h1(class="title is-4") { "QR Import" }
            p(class="subtitle is-6 has-text-grey") {
                "Paste a QR URL from clipboard, type one in, or scan with the camera."
            }
            div(class="field has-addons") {
                div(class="control is-expanded") {
                    input(
                        class="input",
                        placeholder="Paste or type a QR URL…",
                        bind:value=sm.url_input,
                    )
                }
                div(class="control") {
                    button(class="button is-link", on:click=move |_| {
                        let url = sm.url_input.get_clone();
                        if url.trim().is_empty() {
                            sm.error.set("Enter a QR URL first.".into());
                            return;
                        }
                        match parse_qr_url(&url) {
                            Some(parsed) => {
                                sm.error.set(String::new());
                                sm.pending.set(Some(parsed));
                            }
                            None => {
                                sm.error.set("That doesn't look like a valid QR URL.".into());
                            }
                        }
                    }) { "Import" }
                }
            }
            div(class="field is-grouped mt-2") {
                div(class="control") {
                    button(class="button is-light", on:click=move |_| {
                        sm.error.set(String::new());
                        wasm_bindgen_futures::spawn_local(async move {
                            if let Some(window) = web_sys::window() {
                                let clip = window.navigator().clipboard();
                                match wasm_bindgen_futures::JsFuture::from(clip.read_text()).await {
                                    Ok(val) => {
                                        if let Some(text) = val.as_string() {
                                            sm.url_input.set(text);
                                        }
                                    }
                                    Err(_) => {
                                        sm.error.set("Could not read clipboard.".into());
                                    }
                                }
                            }
                        });
                    }) {
                        span(class="icon") { i(class="fa fa-paste") }
                        span { "Paste from clipboard" }
                    }
                }
                div(class="control") {
                    button(class="button is-light", on:click=move |_| {
                        sm.error.set(String::new());
                        crate::update(model, crate::Msg::ScanStartPreview);
                    }) {
                        span(class="icon") { i(class="fa fa-camera") }
                        span { "Scan QR" }
                    }
                }
            }
            (if !sm.error.get_clone().is_empty() {
                let msg = sm.error.get_clone();
                view! { div(class="notification is-danger is-light mt-4") { (msg) } }
            } else { view! {} })
            (match sm.pending.get_clone() {
                Some(parsed) => view_confirm_modal(model, parsed),
                None => view! {},
            })
            (crate::khana::helpers::view_handoff(model))
        }
    }
}

fn view_confirm_modal(model: crate::Model, parsed: ParsedQr) -> View {
    let sm = model.screens.qr;
    let (title, summary) = match &parsed {
        ParsedQr::Account {
            homeserver,
            user_id,
            ..
        } => (
            "Import Account".to_string(),
            format!("Save account {user_id} on {homeserver}?"),
        ),
        ParsedQr::Contact { user_id, name, .. } => {
            let label = if name.is_empty() {
                user_id.clone()
            } else {
                format!("{name} ({user_id})")
            };
            (
                "Import Contact".to_string(),
                format!("Save contact {label}?"),
            )
        }
        ParsedQr::Invite(inv) => (
            "Join Event".to_string(),
            format!(
                "Join event on {}?",
                crate::page::home::hs_host_port(&inv.homeserver)
            ),
        ),
    };
    view! {
        div(class="modal is-active") {
            div(class="modal-background", on:click=move |_| sm.pending.set(None))
            div(class="modal-card") {
                header(class="modal-card-head") {
                    p(class="modal-card-title") { (title) }
                    button(class="delete", on:click=move |_| sm.pending.set(None))
                }
                section(class="modal-card-body") {
                    p { (summary) }
                }
                footer(class="modal-card-foot") {
                    button(class="button is-link", on:click=move |_| {
                        if let Some(p) = sm.pending.take() {
                            sm.url_input.set(String::new());
                            match p {
                                ParsedQr::Account { homeserver, user_id, password } => {
                                    crate::update(model, crate::Msg::ImportAccount {
                                        homeserver, user_id, password,
                                    });
                                }
                                ParsedQr::Contact { user_id, name, description, phone } => {
                                    crate::update(model, crate::Msg::ImportContact {
                                        user_id, name, description, phone,
                                    });
                                }
                                ParsedQr::Invite(inv) => {
                                    crate::update(model, crate::Msg::Join(inv));
                                }
                            }
                        }
                    }) { "Confirm" }
                    button(class="button", on:click=move |_| sm.pending.set(None)) { "Cancel" }
                }
            }
        }
    }
}
