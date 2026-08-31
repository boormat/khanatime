use std::collections::HashMap;

use sycamore::prelude::*;

/// UI state for the accounts page.
#[derive(Clone, Copy)]
pub struct Model {
    pub refresh: Signal<u8>,
    pub show_create: Signal<bool>,
    pub show_add_hs: Signal<bool>,
    pub show_contact: Signal<bool>,
    pub show_qr: Signal<Option<QrTarget>>,
    pub feedback: Signal<String>,
    /// Homeserver awaiting a Forget confirmation ("are you sure" modal).
    pub forget_target: Signal<Option<String>>,
    /// Send Hello modal state: homeserver + user_id of selected account.
    pub hello_target: Signal<Option<(String, String)>>,
    // Unified sign-in / create section (used by modal)
    pub create_hs: Signal<String>,
    pub create_type: Signal<u8>,
    pub create_event: Signal<String>,
    pub create_user: Signal<String>,
    pub create_pass: Signal<String>,
    pub create_desc: Signal<String>,
    // Add homeserver modal
    pub add_hs_url: Signal<String>,
    pub add_hs_name: Signal<String>,
    pub add_hs_element: Signal<String>,
    // Add contact modal
    pub contact_user: Signal<String>,
    pub contact_name: Signal<String>,
    pub contact_desc: Signal<String>,
    pub contact_phone: Signal<String>,
    /// Connectivity status per homeserver URL (keyed by the homeserver's `url`).
    pub hs_status: Signal<HashMap<String, ConnStatus>>,
    /// Monotonic token used to debounce the add-homeserver URL probe.
    pub hs_check_seq: Signal<u32>,
}

#[derive(Clone, PartialEq, Eq)]
pub enum QrTarget {
    /// Share account credentials (includes password). `true` = personal (warn).
    Account { homeserver: String, personal: bool },
    /// Share as contact (no credentials).
    Contact(String),
}

/// Reachability state of a Matrix homeserver, shown as a status chip.
#[derive(Clone, PartialEq)]
pub enum ConnStatus {
    /// Not yet checked (or no homeserver selected).
    Unknown,
    /// A probe is in flight.
    Checking,
    /// Server answered `/_matrix/client/versions`.
    Reachable,
    /// Network-level failure (server down / unreachable). Holds the message.
    Unreachable(String),
}

impl Model {
    pub fn new() -> Self {
        Self {
            refresh: create_signal(0u8),
            show_create: create_signal(false),
            show_add_hs: create_signal(false),
            show_contact: create_signal(false),
            show_qr: create_signal(None),
            feedback: create_signal(String::new()),
            forget_target: create_signal(None),
            hello_target: create_signal(None),
            create_hs: create_signal(String::new()),
            create_type: create_signal(0u8),
            create_event: create_signal(String::new()),
            create_user: create_signal(String::new()),
            create_pass: create_signal(String::new()),
            create_desc: create_signal(String::new()),
            add_hs_url: create_signal(String::new()),
            add_hs_name: create_signal(String::new()),
            add_hs_element: create_signal(String::new()),
            contact_user: create_signal(String::new()),
            contact_name: create_signal(String::new()),
            contact_desc: create_signal(String::new()),
            contact_phone: create_signal(String::new()),
            hs_status: create_signal(HashMap::new()),
            hs_check_seq: create_signal(0u32),
        }
    }
}

pub fn init() -> Model {
    Model::new()
}

/// Render a small connectivity status chip for a homeserver.
#[cfg(target_arch = "wasm32")]
pub fn view_hs_status(status: ConnStatus) -> View {
    match status {
        ConnStatus::Reachable => view! {
            span(class="tag is-success is-light is-small") { "online" }
        },
        ConnStatus::Checking => view! {
            span(class="tag is-warning is-light is-small") { "checking…" }
        },
        ConnStatus::Unreachable(msg) => view! {
            span(class="tag is-danger is-light is-small", title=msg) { "unreachable" }
        },
        ConnStatus::Unknown => view! {},
    }
}

pub fn view(model: crate::Model) -> View {
    let sm = model.screens.accounts;
    view! {
        div(class="section") {
            h1(class="title is-4") { "Accounts" }
            (view_homeservers(model))
            (view_signing_key(model))
            (view_contacts(model))
            (view_action_buttons(model))
            (if sm.show_create.get() { view_create_modal(model) } else { view! {} })
            (if sm.show_add_hs.get() { view_add_hs_modal(model) } else { view! {} })
            (if sm.show_contact.get() { view_contact_modal(model) } else { view! {} })
            (match sm.show_qr.get_clone() {
                Some(t) => view_qr_modal(model, t),
                None => view! {},
            })
            (view_forget_modal(model))
            (view_hello_modal(model))
            (if !sm.feedback.get_clone().is_empty() {
                let msg = sm.feedback.get_clone();
                view! {
                    div(class="notification is-info is-light") {
                        button(class="delete", on:click=move |_| sm.feedback.set(String::new()))
                        (msg)
                    }
                }
            } else { view! {} })
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn view_homeservers(model: crate::Model) -> View {
    let sm = model.screens.accounts;
    let _ = sm.refresh.get();
    let homeservers = crate::services::matrix::load_homeservers();
    let accounts = crate::services::matrix::load_accounts();
    let mut boxes: Vec<View> = Vec::new();
    for hs in homeservers {
        let hs_url = hs.url.clone();
        let hs_accounts: Vec<crate::services::matrix::Account> = accounts
            .iter()
            .filter(|a| a.homeserver == hs_url)
            .cloned()
            .collect();
        let label = crate::page::home::hs_host_port(&hs.url);
        let account_count = hs_accounts.len();
        let hs_url_remove = hs_url.clone();

        // Account rows
        let mut account_rows: Vec<View> = Vec::new();
        for a in hs_accounts {
            let type_label = match a.account_type {
                crate::services::matrix::AccountType::Personal => "Personal",
                crate::services::matrix::AccountType::Shared => "Shared",
            };
            let desc_text = if a.description.is_empty() {
                type_label.to_string()
            } else {
                format!("{} · {}", a.description, type_label)
            };
            let active = a.active;
            let is_personal = a.account_type == crate::services::matrix::AccountType::Personal;
            let a_hs = a.homeserver.clone();
            let a_hs_forget = a.homeserver.clone();
            let a_hs3 = a.homeserver.clone();
            let a_user3 = a.user_id.clone();
            account_rows.push(view! {
                div(class="notification is-light") {
                    div(class="is-flex is-align-items-center is-flex-wrap-wrap", style="gap: 0.5rem;") {
                        div {
                            p { (a.user_id.clone()) }
                            p(class="is-size-7 has-text-grey") { (desc_text) }
                        }
                        div(class="is-flex is-align-items-center is-flex-wrap-wrap", style="gap: 0.35rem;") {
                            (if active {
                                view! { span(class="tag is-success is-small ml-2") { "active" } }
                            } else { view! {} })
                            (if active {
                                view! {
                                    button(
                                        class="button is-small is-warning",
                                        on:click=move |_| {
                                            crate::update(model, crate::Msg::Conn(crate::sync::Msg::Logout));
                                            sm.refresh.update(|v| v.wrapping_add(1));
                                        },
                                    ) {
                                        span(class="icon is-small") { i(class="fa fa-right-from-bracket") }
                                        span { "Logout" }
                                    }
                                }
                            } else {
                                view! {
                                    button(
                                        class="button is-small is-link",
                                        on:click=move |_| {
                                            crate::update(model, crate::Msg::Conn(crate::sync::Msg::Relogin(a_hs.clone())));
                                            sm.refresh.update(|v| v.wrapping_add(1));
                                        },
                                    ) {
                                        span(class="icon is-small") { i(class="fa fa-right-to-bracket") }
                                        span { "Login" }
                                    }
                                }
                            })
                            button(
                                class="button is-small is-danger is-rounded",
                                title="Forget this account",
                                on:click=move |_| {
                                    sm.forget_target.set(Some(a_hs_forget.clone()));
                                },
                            ) {
                                span(class="icon is-small") { i(class="fa fa-xmark") }
                            }
                            button(
                                class="button is-small is-info is-outlined",
                                title="Share account credentials via QR",
                                on:click=move |_| {
                                    sm.show_qr.set(Some(QrTarget::Account {
                                        homeserver: a_hs3.clone(),
                                        personal: is_personal,
                                    }));
                                },
                            ) {
                                span(class="icon is-small") { i(class="fa fa-lock") }
                            }
                            button(
                                class="button is-small is-light",
                                title="Share as contact (no password)",
                                on:click=move |_| {
                                    sm.show_qr.set(Some(QrTarget::Contact(a_user3.clone())));
                                },
                            ) {
                                span(class="icon is-small") { i(class="fa fa-address-card") }
                            }
                        }
                    }
                }
            });
        }
        let body = if account_rows.is_empty() {
            view! { p(class="has-text-grey") { "No accounts." } }
        } else {
            view! { (account_rows) }
        };
        boxes.push(view! {
            div(class="box") {
                div(class="is-flex is-justify-content-space-between is-align-items-center") {
                    div(class="is-flex is-align-items-center", style="gap: 0.5rem;") {
                        h2(class="title is-5 mb-0") { (label) }
                        (move || {
                            let s = sm
                                .hs_status
                                .get_clone()
                                .get(&hs.url)
                                .cloned()
                                .unwrap_or(ConnStatus::Unknown);
                            view_hs_status(s)
                        })
                    }
                    (if account_count == 0 {
                        view! {
                            button(
                                class="button is-small is-danger is-outlined",
                                title="Remove homeserver",
                                on:click=move |_| {
                                    crate::services::matrix::remove_homeserver(&hs_url_remove);
                                    sm.refresh.update(|v| v.wrapping_add(1));
                                },
                            ) {
                                span(class="icon is-small") { i(class="fa fa-xmark") }
                            }
                        }
                    } else {
                        view! {
                            span(
                                class="tag is-light is-small",
                                title=format!("Remove all {} accounts before removing the homeserver", account_count),
                            ) {
                                span(class="icon is-small") { i(class="fa fa-xmark") }
                            }
                        }
                    })
                }
                (body)
            }
        });
    }
    let heading = view! { h2(class="title is-5") { "Homeservers" } };
    if boxes.is_empty() {
        view! { (heading) p(class="has-text-grey") { "No homeservers configured." } }
    } else {
        view! { (heading) (boxes) }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn view_homeservers(_model: crate::Model) -> View {
    view! {}
}

// ---------------------------------------------------------------------------
// Signing key section
// ---------------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
fn view_signing_key(model: crate::Model) -> View {
    let sm = model.screens.accounts;
    let _ = sm.refresh.get();
    // Generate an in-memory key if storage is blocked rather than panicking.
    let keys = crate::signing::DeviceKeys::load_or_generate("default", "device");
    let fp = keys.fingerprint().expect("fingerprint failed");
    let pub_key_b64 = keys.ed25519_public_key.clone();
    let registry = crate::signing::SigningKeyRegistry::load();

    let key_view = view! {
        div(class="is-flex is-align-items-center is-flex-wrap-wrap", style="gap: 0.5rem;") {
            span(class="tag is-info is-medium") { (fp) }
            button(
                class="button is-small is-link is-outlined",
                title="Copy public key to clipboard",
                on:click=move |_| {
                    let nav = web_sys::window().unwrap().navigator().clipboard();
                    let _ = nav.write_text(&pub_key_b64);
                },
            ) {
                span(class="icon is-small") { i(class="fa fa-copy") }
            }
        }
    };

    // Build trust registry sub-view
    let reg_view = if registry.all().is_empty() {
        view! {}
    } else {
        let mut reg_items: Vec<View> = Vec::new();
        for rec in registry.all() {
            let key_fp = crate::signing::DeviceKeys::from_public_key(
                String::new(),
                String::new(),
                rec.public_key.clone(),
            )
            .fingerprint()
            .unwrap_or_else(|_| "?".into());
            let status_class = match rec.status {
                crate::signing::KeyTrustStatus::Verified => "is-success",
                crate::signing::KeyTrustStatus::Unverified => "is-warning",
                crate::signing::KeyTrustStatus::Rejected => "is-danger",
            };
            let status_label = match rec.status {
                crate::signing::KeyTrustStatus::Verified => "Verified",
                crate::signing::KeyTrustStatus::Unverified => "Unverified",
                crate::signing::KeyTrustStatus::Rejected => "Rejected",
            };
            let uid_text = rec.user_id.clone().unwrap_or_else(|| "unknown".into());
            let linked = rec.contact_id.clone().unwrap_or_default();
            let linked_text = if linked.is_empty() {
                view! {}
            } else {
                view! { span(class="is-size-7 has-text-grey ml-2") { (format!("linked to {linked}")) } }
            };
            reg_items.push(view! {
                div(class="is-flex is-align-items-center is-flex-wrap-wrap py-1", style="gap: 0.25rem;") {
                    span(class="tag is-small") { (key_fp) }
                    span(class="tag is-small") { (uid_text) }
                    span(class={format!("tag is-small {status_class}")}) { (status_label) }
                    (linked_text)
                }
            });
        }
        view! {
            h3(class="title is-6 mt-4") { "Trust Registry" }
            (reg_items)
        }
    };

    view! {
        div(class="box") {
            h2(class="title is-5") { "Signing Key" }
            (key_view)
            (reg_view)
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn view_signing_key(_model: crate::Model) -> View {
    view! {}
}

#[cfg(target_arch = "wasm32")]
fn view_contacts(model: crate::Model) -> View {
    let sm = model.screens.accounts;
    let _ = sm.refresh.get();
    let contacts = crate::services::matrix::load_contacts();
    let mut items: Vec<View> = Vec::new();
    for c in contacts {
        let label = if c.name.is_empty() {
            c.description.clone()
        } else if c.description.is_empty() {
            c.name.clone()
        } else {
            format!("{} · {}", c.name, c.description)
        };
        let phone_text = match &c.phone {
            Some(p) => format!(" · {}", p),
            None => String::new(),
        };
        let key_text = c.signing_key.as_ref().map(|k| {
            crate::signing::DeviceKeys::from_public_key(String::new(), String::new(), k.clone())
                .fingerprint()
                .unwrap_or_else(|_| "?".into())
        });
        let key_view = if let Some(kfp) = key_text {
            view! { p(class="is-size-7 has-text-info") { "key: " (kfp) } }
        } else {
            view! {}
        };
        let c_uid = c.user_id.clone();
        let c_uid2 = c.user_id.clone();
        items.push(view! {
            div(class="notification is-light") {
                div(class="is-flex is-align-items-center is-flex-wrap-wrap", style="gap: 0.5rem;") {
                    div {
                        p { (c.user_id) }
                        p(class="is-size-7 has-text-grey") { (label) (phone_text) }
                        (key_view)
                    }
                    div(class="buttons") {
                        button(
                            class="button is-info is-outlined",
                            title="Share contact via QR",
                            on:click=move |_| {
                                sm.show_qr.set(Some(QrTarget::Contact(c_uid.clone())));
                            },
                        ) {
                            span(class="icon is-small") { i(class="fa fa-qrcode") }
                        }
                        button(
                            class="button is-danger is-outlined",
                            title="Remove contact",
                            on:click=move |_| {
                                crate::services::matrix::remove_contact(&c_uid2);
                                sm.refresh.set(sm.refresh.get() + 1);
                            },
                        ) {
                            span(class="icon is-small") { i(class="fa fa-trash") }
                        }
                    }
                }
            }
        });
    }
    let heading = view! { h2(class="title is-5") { "Contacts" } };
    if items.is_empty() {
        view! { (heading) p(class="has-text-grey") { "No contacts yet." } }
    } else {
        view! { (heading) (items) }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn view_contacts(_model: crate::Model) -> View {
    view! {}
}

/// Action buttons at the bottom of the accounts page.
#[cfg(target_arch = "wasm32")]
fn view_action_buttons(model: crate::Model) -> View {
    let sm = model.screens.accounts;
    view! {
        div(class="buttons mt-4") {
            button(class="button is-link", on:click=move |_| sm.show_create.set(true)) {
                span(class="icon") { i(class="fa fa-right-to-bracket") }
                span { "Sign in or create" }
            }
            button(class="button is-light", on:click=move |_| sm.show_add_hs.set(true)) {
                span(class="icon") { i(class="fa fa-server") }
                span { "Add homeserver" }
            }
            button(class="button is-light", on:click=move |_| sm.show_contact.set(true)) {
                span(class="icon") { i(class="fa fa-user-plus") }
                span { "Add contact" }
            }
            button(class="button is-light", on:click=move |_| {
                crate::update(model, crate::Msg::Show(crate::Screen::Qr));
            }) {
                span(class="icon") { i(class="fa fa-qrcode") }
                span { "Scan QR" }
            }
            (move || {
                let has_room = model.sync.room.with(|r| r.is_some());
                let has_id = model.sync.identity.with(|u| !u.is_empty());
                view! {
                    button(
                        class="button is-light",
                        disabled=!has_room || !has_id,
                        on:click=move |_| {
                            let hs = crate::services::matrix::active_hs().unwrap_or_default();
                            let uid = model.sync.identity.get_clone();
                            sm.hello_target.set(Some((hs, uid)));
                        },
                    ) {
                        span(class="icon") { i(class="fa fa-bullhorn") }
                        span { "Send Hello" }
                    }
                }
            })
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn view_action_buttons(_model: crate::Model) -> View {
    view! {}
}

// ---------------------------------------------------------------------------
// Sign in / Create modal
// ---------------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
fn view_create_modal(model: crate::Model) -> View {
    let sm = model.screens.accounts;
    let homeservers = crate::services::matrix::load_homeservers();
    let hs_opts: Vec<View> = homeservers
        .iter()
        .map(|hs| {
            let label = crate::page::home::hs_host_port(&hs.url);
            let url = hs.url.clone();
            view! { option(value=url) { (label) } }
        })
        .collect();
    let selected_hs = sm.create_hs.get_clone();
    let hs_config = homeservers.iter().find(|h| h.url == selected_hs);
    let is_sso = hs_config
        .map(|h| h.reg == crate::event::RegistrationMode::Sso)
        .unwrap_or(false);
    let account_type = sm.create_type.get();
    let is_shared = account_type == 1;

    // Event dropdown options (only needed when Shared is selected, but load eagerly for simplicity)
    let event_opts: Vec<View> = {
        let mut ids: Vec<String> = crate::event::list_events().into_iter().collect();
        ids.retain(|id| id != crate::event::DEMO_EVENT_ID);
        ids.sort();
        ids.into_iter()
            .map(|id| {
                let e = crate::event::load_event(&id);
                let label = if e.name.is_empty() {
                    id.clone()
                } else {
                    format!("{} ({})", e.name, e.year)
                };
                view! { option(value=id) { (label) } }
            })
            .collect()
    };

    let hs_empty = hs_opts.is_empty();
    let event_empty = event_opts.is_empty();

    // Right-column form content
    let show_pass = create_signal(false);
    let form = if selected_hs.is_empty() {
        view! {
            p(class="has-text-grey") { "Select a homeserver to continue." }
        }
    } else {
        let mut sections: Vec<View> = Vec::new();

        // SSO button (personal only, SSO homeservers)
        if is_sso && !is_shared {
            let hs_for_sso = selected_hs.clone();
            sections.push(view! {
                div(class="field") {
                    button(
                        class="button is-link",
                        on:click=move |_| {
                            sm.show_create.set(false);
                            crate::update(model, crate::Msg::Conn(crate::sync::Msg::SsoLoginFor(hs_for_sso.clone())));
                        },
                    ) {
                        span(class="icon") { i(class="fa fa-id-badge") }
                        span { "Sign in with SSO" }
                    }
                }
            });
        }

        // Event dropdown (Shared only)
        if is_shared {
            sections.push(view! {
                div(class="field") {
                    label(class="label") { "Event" }
                    div(class="is-flex", style="gap: 0.5rem;") {
                        div(class="control is-flex-grow-1") {
                            div(class="select is-fullwidth") {
                                select(bind:value=sm.create_event, disabled=event_empty) {
                                    option(value="") { "Select event…" }
                                    (event_opts)
                                }
                            }
                        }
                        button(
                            class="button is-info is-outlined",
                            title="Auto-fill username, password, and description from the selected event",
                            disabled=sm.create_event.get_clone().is_empty(),
                            on:click=move |_| {
                                let eid = sm.create_event.get_clone();
                                if eid.is_empty() { return; }
                                let e = crate::event::load_event(&eid);
                                let slug = crate::event::slugify(&e.name);
                                sm.create_user.set(format!("shared_{slug}"));
                                sm.create_pass.set(crate::ids::gen_short_id());
                                sm.create_desc.set(e.name.clone());
                            },
                        ) {
                            span(class="icon is-small") { i(class="fa fa-wand-magic-sparkles") }
                        }
                    }
                    p(class="help") { "Pick an event to auto-fill username, password, and description." }
                }
            });
        }

        // Username + Password
        sections.push(view! {
            div(class="field") {
                label(class="label") { "Username" }
                div(class="control") {
                    input(class="input", placeholder="e.g. alice", bind:value=sm.create_user)
                }
            }
            div(class="field") {
                label(class="label") { "Password" }
                div(class="control has-icons-right") {
                    input(class="input",
                        r#type=if show_pass.get() { "text" } else { "password" },
                        placeholder="Enter or generate a password",
                        bind:value=sm.create_pass,
                    )
                    span(class="icon is-small is-right is-clickable", on:click=move |_| show_pass.set(!show_pass.get())) {
                        i(class=if show_pass.get() { "fa fa-eye-slash" } else { "fa fa-eye" })
                    }
                }
            }
        });

        // Description (Shared only)
        if is_shared {
            sections.push(view! {
                div(class="field") {
                    label(class="label") { "Description" }
                    div(class="control") {
                        input(class="input", placeholder="e.g. Timing crew", bind:value=sm.create_desc)
                    }
                }
            });
        }

        view! { (sections) }
    };

    view! {
        div(class="modal is-active") {
            div(class="modal-background", on:click=move |_| sm.show_create.set(false))
            div(class="modal-card") {
                header(class="modal-card-head") {
                    p(class="modal-card-title") { "Sign in or create" }
                    button(class="delete", on:click=move |_| sm.show_create.set(false))
                }
                section(class="modal-card-body") {
                    div(class="columns is-mobile") {
                        div(class="column is-4") {
                            div(class="field") {
                                label(class="label") { "Homeserver" }
                                div(class="control") {
                                    div(class="select is-fullwidth") {
                                        select(bind:value=sm.create_hs, disabled=hs_empty) {
                                            option(value="") { "Select…" }
                                            (hs_opts)
                                        }
                                    }
                                }
                                p(class="help") {
                                    (move || view_hs_status(
                                        sm.hs_status
                                            .get_clone()
                                            .get(&sm.create_hs.get_clone())
                                            .cloned()
                                            .unwrap_or(ConnStatus::Unknown),
                                    ))
                                }
                            }
                            div(class="field") {
                                label(class="label") { "Account type" }
                                div(class="control") {
                                    label(class="radio") {
                                        input(r#type="radio", name="ct", checked=move || sm.create_type.get() == 0,
                                            on:input=move |_| {
                                                sm.create_type.set(0);
                                                // Field-clear handled by reactive effect in setup_effects
                                            })
                                        " Personal"
                                    }
                                    label(class="radio") {
                                        input(r#type="radio", name="ct", checked=move || sm.create_type.get() == 1,
                                            on:input=move |_| {
                                                sm.create_type.set(1);
                                                // Field-clear handled by reactive effect in setup_effects
                                            })
                                        " Shared"
                                    }
                                }
                            }
                        }
                        div(class="column") {
                            (form)
                        }
                    }
                }
                footer(class="modal-card-foot") {
                    div(class="buttons") {
                        button(
                            class="button is-link",
                            disabled=sm.create_hs.get_clone().is_empty() || sm.create_user.get_clone().trim().is_empty() || sm.create_pass.get_clone().trim().is_empty(),
                            on:click=move |_| {
                                let hs = sm.create_hs.get_clone();
                                let username = sm.create_user.get_clone();
                                let password = sm.create_pass.get_clone();
                                let desc = sm.create_desc.get_clone();
                                let acc_type = sm.create_type.get();
                                sm.show_create.set(false);
                                sm.feedback.set(format!("Creating @{}…", username));
                                crate::update(model, crate::Msg::CreateAccount { hs, username, password, description: desc, account_type: acc_type });
                            },
                        ) {
                            span(class="icon") { i(class="fa fa-plus") }
                            span { "Create account" }
                        }
                        button(
                            class="button is-link is-outlined",
                            disabled=sm.create_hs.get_clone().is_empty() || sm.create_user.get_clone().trim().is_empty(),
                            on:click=move |_| {
                                let hs = sm.create_hs.get_clone();
                                let username = sm.create_user.get_clone();
                                sm.show_create.set(false);
                                crate::update(model, crate::Msg::Conn(crate::sync::Msg::AddHomeserver { hs, username }));
                            },
                        ) {
                            span(class="icon") { i(class="fa fa-right-to-bracket") }
                            span { "Login with password" }
                        }
                    }
                    div(class="ml-auto") {
                        button(class="button", on:click=move |_| sm.show_create.set(false)) { "Close" }
                    }
                }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn view_create_modal(_model: crate::Model) -> View {
    view! {}
}

// ---------------------------------------------------------------------------
// Send Hello modal
// ---------------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
fn view_hello_modal(model: crate::Model) -> View {
    let sm = model.screens.accounts;
    if sm.hello_target.with(|t| t.is_none()) {
        return view! {};
    }
    let uid = model.sync.identity.get_clone();
    let hs = crate::services::matrix::active_hs().unwrap_or_default();
    let hs_label = crate::page::home::hs_host_port(&hs);

    view! {
        div(class="modal is-active") {
            div(class="modal-background", on:click=move |_| sm.hello_target.set(None))
            div(class="modal-card") {
                header(class="modal-card-head") {
                    p(class="modal-card-title") { "Send Hello" }
                    button(class="delete", on:click=move |_| sm.hello_target.set(None))
                }
                section(class="modal-card-body") {
                    p { "Post a signed hello to the current event's room." }
                    p(class="mt-2") {
                        "This associates your signing key with "
                        span(class="has-text-weight-semibold") { (uid) }
                        " on "
                        span(class="has-text-weight-semibold") { (hs_label) }
                        "."
                    }
                    p(class="help mt-2") {
                        "Others in the room can verify your device key from this message."
                    }
                }
                footer(class="modal-card-foot") {
                    button(
                        class="button is-link",
                        on:click=move |_| {
                            sm.hello_target.set(None);
                            crate::update(model, crate::Msg::SendHello);
                        },
                    ) { "Send" }
                    button(class="button", on:click=move |_| sm.hello_target.set(None)) { "Cancel" }
                }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn view_hello_modal(_model: crate::Model) -> View {
    view! {}
}

#[cfg(target_arch = "wasm32")]
fn view_add_hs_modal(model: crate::Model) -> View {
    let sm = model.screens.accounts;
    view! {
        div(class="modal is-active") {
            div(class="modal-background", on:click=move |_| sm.show_add_hs.set(false))
            div(class="modal-card") {
                header(class="modal-card-head") {
                    p(class="modal-card-title") { "Add homeserver" }
                    button(class="delete", on:click=move |_| sm.show_add_hs.set(false))
                }
                section(class="modal-card-body") {
                    div(class="buttons mb-4") {
                        button(
                            class="button is-link is-small",
                            on:click=move |_| {
                                sm.add_hs_url.set("https://matrix.org".to_string());
                                sm.add_hs_name.set("matrix.org".to_string());
                            },
                        ) {
                            span(class="icon is-small") { i(class="fa fa-bolt") }
                            span { "matrix.org" }
                        }
                    }
                    div(class="field") {
                        label(class="label") { "URL" }
                        div(class="control") {
                            input(class="input", placeholder="https://matrix.org", bind:value=sm.add_hs_url)
                        }
                        p(class="help") {
                            (move || view_hs_status(
                                sm.hs_status
                                    .get_clone()
                                    .get(&sm.add_hs_url.get_clone())
                                    .cloned()
                                    .unwrap_or(ConnStatus::Unknown),
                            ))
                        }
                    }
                    div(class="field") {
                        label(class="label") { "Name" }
                        div(class="control") {
                            input(class="input", placeholder="e.g. matrix.org", bind:value=sm.add_hs_name)
                        }
                    }
                    div(class="field") {
                        label(class="label") { "Element Web URL (optional)" }
                        div(class="control") {
                            input(class="input", placeholder="e.g. https://app.element.io", bind:value=sm.add_hs_element)
                        }
                        p(class="help") { "Link for opening rooms in Element. Defaults to app.element.io for matrix.org." }
                    }
                }
                footer(class="modal-card-foot") {
                    button(class="button is-link", on:click=move |_| {
                        let url = sm.add_hs_url.get_clone();
                        if url.is_empty() { sm.feedback.set("Enter a URL.".into()); return; }
                        let name = sm.add_hs_name.get_clone();
                        let element = sm.add_hs_element.get_clone();
                        let reg = if crate::event::is_matrix_org_homeserver(&url) {
                            crate::event::RegistrationMode::Sso
                        } else {
                            crate::event::RegistrationMode::Open
                        };
                        let element_link = if element.is_empty() {
                            crate::event::element_link_default(&url)
                        } else {
                            element
                        };
                        let hs = crate::services::matrix::HomeserverConfig {
                            url: url.clone(),
                            name: if name.is_empty() { crate::page::home::hs_host_port(&url) } else { name },
                            description: String::new(),
                            reg,
                            element_link,
                        };
                        crate::services::matrix::save_homeserver(&hs);
                        sm.show_add_hs.set(false);
                        sm.refresh.set(sm.refresh.get() + 1);
                    }) { "Add" }
                    button(class="button", on:click=move |_| sm.show_add_hs.set(false)) { "Cancel" }
                }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn view_add_hs_modal(_model: crate::Model) -> View {
    view! {}
}

#[cfg(target_arch = "wasm32")]
fn view_contact_modal(model: crate::Model) -> View {
    let sm = model.screens.accounts;
    view! {
        div(class="modal is-active") {
            div(class="modal-background", on:click=move |_| sm.show_contact.set(false))
            div(class="modal-card") {
                header(class="modal-card-head") {
                    p(class="modal-card-title") { "Add contact" }
                    button(class="delete", on:click=move |_| sm.show_contact.set(false))
                }
                section(class="modal-card-body") {
                    div(class="field") {
                        label(class="label") { "Matrix user ID" }
                        div(class="control") {
                            input(class="input", placeholder="@user:matrix.org", bind:value=sm.contact_user)
                        }
                    }
                    div(class="field") {
                        label(class="label") { "Name" }
                        div(class="control") {
                            input(class="input", placeholder="Bob Smith", bind:value=sm.contact_name)
                        }
                    }
                    div(class="field") {
                        label(class="label") { "Description" }
                        div(class="control") {
                            input(class="input", placeholder="NDC Club President", bind:value=sm.contact_desc)
                        }
                    }
                    div(class="field") {
                        label(class="label") { "Phone (optional)" }
                        div(class="control") {
                            input(class="input", r#type="tel", placeholder="0412 345 678", bind:value=sm.contact_phone)
                        }
                    }
                }
                footer(class="modal-card-foot") {
                    button(class="button is-link", on:click=move |_| {
                        let uid = sm.contact_user.get_clone();
                        if uid.is_empty() { sm.feedback.set("Enter a Matrix user ID.".into()); return; }
                        let contact = crate::services::matrix::Contact {
                            user_id: uid,
                            name: sm.contact_name.get_clone(),
                            description: sm.contact_desc.get_clone(),
                            phone: { let p = sm.contact_phone.get_clone(); if p.is_empty() { None } else { Some(p) } },
                            signing_key: None,
                        };
                        crate::services::matrix::save_contact(&contact);
                        sm.show_contact.set(false);
                        sm.refresh.set(sm.refresh.get() + 1);
                    }) { "Add" }
                    button(class="button", on:click=move |_| sm.show_contact.set(false)) { "Cancel" }
                }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn view_contact_modal(_model: crate::Model) -> View {
    view! {}
}

#[cfg(target_arch = "wasm32")]
fn view_qr_modal(model: crate::Model, target: QrTarget) -> View {
    let sm = model.screens.accounts;
    let (title, payload, warning) = match &target {
        QrTarget::Account {
            homeserver: hs,
            personal,
        } => {
            let accounts = crate::services::matrix::load_accounts();
            match accounts.iter().find(|a| a.homeserver == *hs && a.active) {
                Some(a) => {
                    let pass = match &a.kind {
                        crate::services::matrix::StoredAuth::Matrix { password, .. } => {
                            password.clone()
                        }
                        crate::services::matrix::StoredAuth::OAuth { .. } => String::new(),
                    };
                    let app_base = web_sys::window()
                        .and_then(|w| {
                            let origin = w.location().origin().ok()?;
                            let path = w.location().pathname().ok()?;
                            Some(format!("{origin}{path}"))
                        })
                        .unwrap_or_default();
                    let data = url::form_urlencoded::byte_serialize(
                        format!(
                            "type=account&homeserver={}&user_id={}&password={}",
                            a.homeserver, a.user_id, pass,
                        )
                        .as_bytes(),
                    )
                    .collect::<String>();
                    let url = format!("{app_base}?{data}");
                    let warn = if *personal {
                        Some("This QR contains your personal account credentials including password. Only share with trusted people in person.".to_string())
                    } else {
                        None
                    };
                    (format!("Account — {}", a.user_id), url, warn)
                }
                None => ("No active account".into(), String::new(), None),
            }
        }
        QrTarget::Contact(uid) => {
            // First check contacts, then check accounts (share as contact, no password).
            let contacts = crate::services::matrix::load_contacts();
            let app_base = web_sys::window()
                .and_then(|w| {
                    let origin = w.location().origin().ok()?;
                    let path = w.location().pathname().ok()?;
                    Some(format!("{origin}{path}"))
                })
                .unwrap_or_default();
            if let Some(c) = contacts.iter().find(|c| c.user_id == *uid) {
                let phone_param = c
                    .phone
                    .as_deref()
                    .map(|p| format!("&phone={p}"))
                    .unwrap_or_default();
                let qs = format!(
                    "type=contact&user_id={}&name={}&description={}{phone_param}",
                    c.user_id, c.name, c.description,
                );
                let data = url::form_urlencoded::byte_serialize(qs.as_bytes()).collect::<String>();
                let url = format!("{app_base}?{data}");
                (format!("Contact — {}", c.user_id), url, None)
            } else {
                // Not in contacts — check accounts for sharing as contact.
                let accounts = crate::services::matrix::load_accounts();
                if let Some(a) = accounts.iter().find(|a| a.user_id == *uid) {
                    let qs = format!(
                        "type=contact&user_id={}&name={}&description={}",
                        a.user_id, a.user_id, a.description,
                    );
                    let data =
                        url::form_urlencoded::byte_serialize(qs.as_bytes()).collect::<String>();
                    let url = format!("{app_base}?{data}");
                    (format!("Contact — {}", a.user_id), url, None)
                } else {
                    ("Contact not found".into(), String::new(), None)
                }
            }
        }
    };
    let svg = if payload.is_empty() {
        String::new()
    } else {
        crate::services::qr::qr_svg(&payload, 320).unwrap_or_default()
    };
    let svg_empty = svg.is_empty();
    let payload_empty = payload.is_empty();
    view! {
        div(class="modal is-active") {
            div(class="modal-background", on:click=move |_| sm.show_qr.set(None))
            div(class="modal-card") {
                header(class="modal-card-head") {
                    p(class="modal-card-title") { (title) }
                    button(class="delete", on:click=move |_| sm.show_qr.set(None))
                }
                section(class="modal-card-body has-text-centered") {
                    (match warning {
                        Some(ref msg) => {
                            let m = msg.clone();
                            view! { div(class="notification is-danger is-light mb-4 has-text-left") {
                                p(class="has-text-weight-semibold") { "Warning" }
                                p { (m) }
                            }}
                        }
                        None => view! {},
                    })
                    (if svg_empty {
                        view! { p { "No data to encode." } }
                    } else {
                        view! { div(dangerously_set_inner_html=svg) {} }
                    })
                    (if !payload_empty {
                        let display = if payload.len() > 80 {
                            format!("{}…", &payload[..80])
                        } else {
                            payload.clone()
                        };
                        let copy_data = payload.clone();
                        view! {
                            div(class="mt-4") {
                                p(class="has-text-weight-semibold is-size-7 mb-1") { "QR payload:" }
                                pre(class="has-text-left is-size-7 has-background-light p-2") {
                                    code { (display) }
                                }
                            }
                            button(
                                class="button is-small is-link is-outlined",
                                on:click=move |_| {
                                    if let Some(window) = web_sys::window() {
                                        let nav = window.navigator();
                                        let _ = nav.clipboard().write_text(&copy_data);
                                    }
                                    sm.feedback.set("Copied to clipboard.".into());
                                },
                            ) {
                                span(class="icon is-small") { i(class="fa fa-copy") }
                                span { "Copy" }
                            }
                        }
                    } else { view! {} })
                }
                footer(class="modal-card-foot") {
                    button(class="button", on:click=move |_| sm.show_qr.set(None)) { "Close" }
                }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn view_qr_modal(_model: crate::Model, _target: QrTarget) -> View {
    view! {}
}

// ---------------------------------------------------------------------------
// Forget confirmation modal
// ---------------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
fn view_forget_modal(model: crate::Model) -> View {
    let sm = model.screens.accounts;
    let Some(hs) = sm.forget_target.get_clone() else {
        return view! {};
    };
    let hs_display = hs.clone();
    let hs_confirm = hs.clone();
    view! {
        div(class="modal is-active") {
            div(class="modal-background", on:click=move |_| sm.forget_target.set(None))
            div(class="modal-card") {
                header(class="modal-card-head") {
                    p(class="modal-card-title") { "Forget this account?" }
                    button(class="delete", on:click=move |_| sm.forget_target.set(None))
                }
                section(class="modal-card-body") {
                    p { "Remove the stored session for:" }
                    p(class="has-text-weight-medium") { (hs_display) }
                    p(class="help") {
                        "Forgetting the active account also signs it out server-side."
                    }
                }
                footer(class="modal-card-foot") {
                    button(
                        class="button is-danger",
                        on:click=move |_| {
                            sm.forget_target.set(None);
                            crate::update(
                                model,
                                crate::Msg::Conn(crate::sync::Msg::Forget(hs_confirm.clone())),
                            );
                            sm.refresh.update(|v| v.wrapping_add(1));
                        },
                    ) { "Forget" }
                    button(class="button", on:click=move |_| sm.forget_target.set(None)) {
                        "Cancel"
                    }
                }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn view_forget_modal(_model: crate::Model) -> View {
    view! {}
}
