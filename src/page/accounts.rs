use sycamore::prelude::*;

/// UI state for the accounts page.
#[derive(Clone, Copy)]
pub struct Model {
    pub refresh: Signal<u8>,
    pub show_create: Signal<bool>,
    pub show_login: Signal<bool>,
    pub show_add_hs: Signal<bool>,
    pub show_contact: Signal<bool>,
    pub show_qr: Signal<Option<QrTarget>>,
    pub feedback: Signal<String>,
    pub create_hs: Signal<String>,
    pub create_type: Signal<u8>,
    pub create_desc: Signal<String>,
    pub login_hs: Signal<String>,
    pub login_user: Signal<String>,
    pub login_pass: Signal<String>,
    pub add_hs_url: Signal<String>,
    pub add_hs_name: Signal<String>,
    pub add_hs_element: Signal<String>,
    pub contact_user: Signal<String>,
    pub contact_name: Signal<String>,
    pub contact_desc: Signal<String>,
    pub contact_phone: Signal<String>,
}

#[derive(Clone, PartialEq, Eq)]
pub enum QrTarget {
    /// Share account credentials (includes password). `true` = personal (warn).
    Account { homeserver: String, personal: bool },
    /// Share as contact (no credentials).
    Contact(String),
}

impl Model {
    pub fn new() -> Self {
        Self {
            refresh: create_signal(0u8),
            show_create: create_signal(false),
            show_login: create_signal(false),
            show_add_hs: create_signal(false),
            show_contact: create_signal(false),
            show_qr: create_signal(None),
            feedback: create_signal(String::new()),
            create_hs: create_signal(String::new()),
            create_type: create_signal(0u8),
            create_desc: create_signal(String::new()),
            login_hs: create_signal(String::new()),
            login_user: create_signal(String::new()),
            login_pass: create_signal(String::new()),
            add_hs_url: create_signal(String::new()),
            add_hs_name: create_signal(String::new()),
            add_hs_element: create_signal(String::new()),
            contact_user: create_signal(String::new()),
            contact_name: create_signal(String::new()),
            contact_desc: create_signal(String::new()),
            contact_phone: create_signal(String::new()),
        }
    }
}

pub fn init() -> Model {
    Model::new()
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
            (if sm.show_login.get() { view_login_modal(model) } else { view! {} })
            (if sm.show_add_hs.get() { view_add_hs_modal(model) } else { view! {} })
            (if sm.show_contact.get() { view_contact_modal(model) } else { view! {} })
            (match sm.show_qr.get_clone() {
                Some(t) => view_qr_modal(model, t),
                None => view! {},
            })
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
        let desc = if hs.description.is_empty() {
            hs.url.clone()
        } else {
            hs.description.clone()
        };
        let mut account_rows: Vec<View> = Vec::new();
        for a in &hs_accounts {
            let type_label = match a.account_type {
                crate::services::matrix::AccountType::Personal => "Personal",
                crate::services::matrix::AccountType::EventShared => "Event shared",
                crate::services::matrix::AccountType::ClubShared => "Club shared",
            };
            let desc_text = if a.description.is_empty() {
                type_label.to_string()
            } else {
                format!("{} · {}", a.description, type_label)
            };
            let active = a.active;
            let is_personal = a.account_type == crate::services::matrix::AccountType::Personal;
            let a_user = a.user_id.clone();
            let a_hs = a.homeserver.clone();
            let a_hs2 = a.homeserver.clone();
            let a_user2 = a.user_id.clone();
            let a_hs3 = a.homeserver.clone();
            let a_user3 = a.user_id.clone();
            account_rows.push(view! {
                div(class="notification is-light") {
                    div(class="is-flex is-align-items-center is-flex-wrap-wrap", style="gap: 0.5rem;") {
                        div {
                            p { (a_user.clone()) }
                            p(class="is-size-7 has-text-grey") { (desc_text) }
                        }
                        div(class="is-flex is-align-items-center is-flex-wrap-wrap", style="gap: 0.35rem;") {
                            (if active {
                                view! { span(class="tag is-success is-small ml-2") { "active" } }
                            } else { view! {} })
                            div(class="buttons has-addons") {
                                button(
                                    class="button is-small is-link",
                                    disabled=active,
                                    on:click=move |_| {
                                        crate::update(model, crate::Msg::Conn(crate::sync::Msg::Relogin(a_hs.clone())));
                                    },
                                ) { "Login" }
                                button(
                                    class="button is-small is-light",
                                    disabled=!active,
                                    on:click=move |_| {
                                        crate::update(model, crate::Msg::Conn(crate::sync::Msg::Logout));
                                    },
                                ) { "Logout" }
                                button(
                                    class="button is-small is-danger is-outlined",
                                    disabled=active,
                                    on:click=move |_| {
                                        crate::services::matrix::remove_account(&a_hs2, &a_user2);
                                        sm.refresh.set(sm.refresh.get() + 1);
                                    },
                                ) { "Forget" }
                            }
                            button(
                                class="button is-info is-outlined",
                                title="Share account credentials via QR",
                                on:click=move |_| {
                                    sm.show_qr.set(Some(QrTarget::Account {
                                        homeserver: a_hs3.clone(),
                                        personal: is_personal,
                                    }));
                                },
                            ) {
                                span(class="icon is-small") { i(class="fa fa-qrcode") }
                            }
                            button(
                                class="button is-light",
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
                h2(class="title is-5") { (label) }
                p(class="subtitle is-6 has-text-grey") { (desc) }
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
    let keys = crate::signing::DeviceKeys::load_from_storage();
    let fp = keys.as_ref().and_then(|k| k.fingerprint().ok());
    let pub_key_b64 = keys.as_ref().map(|k| k.ed25519_public_key.clone());
    let registry = crate::signing::SigningKeyRegistry::load();

    // Build fingerprint + export sub-view
    let key_view = if let Some(fp) = fp {
        let pk = pub_key_b64.unwrap_or_default();
        view! {
            p {
                span(class="tag is-info is-medium") { (fp) }
                span(class="ml-2 has-text-grey") { "device fingerprint" }
            }
            button(
                class="button is-small is-link is-outlined mt-2",
                title="Copy public key to clipboard",
                on:click=move |_| {
                    let nav = web_sys::window().unwrap().navigator().clipboard();
                    let _ = nav.write_text(&pk);
                },
            ) {
                span(class="icon is-small") { i(class="fa fa-copy") }
                span { "Export Public Key" }
            }
        }
    } else {
        view! {
            p(class="has-text-grey") { "No signing key generated yet. It will be created on first use." }
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
                div(class="notification is-light is-flex is-align-items-center is-justify-content-space-between py-2 px-3 mb-2") {
                    div {
                        span(class="tag is-small mr-1") { (key_fp) }
                        span(class="tag is-small") { (uid_text) }
                        span(class={format!("tag is-small ml-1 {status_class}")}) { (status_label) }
                        (linked_text)
                    }
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

fn view_action_buttons(model: crate::Model) -> View {
    let sm = model.screens.accounts;
    view! {
        div(class="buttons mt-4") {
            button(class="button is-link", on:click=move |_| sm.show_create.set(true)) {
                span(class="icon") { i(class="fa fa-plus") }
                span { "Create account" }
            }
            button(class="button is-link is-outlined", on:click=move |_| sm.show_login.set(true)) {
                span(class="icon") { i(class="fa fa-right-to-bracket") }
                span { "Login with existing" }
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
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn view_create_modal(model: crate::Model) -> View {
    let sm = model.screens.accounts;
    let homeservers = crate::services::matrix::load_homeservers();
    let hs_opts: Vec<View> = homeservers
        .into_iter()
        .map(|hs| {
            let label = crate::page::home::hs_host_port(&hs.url);
            view! { option(value=hs.url) { (label) } }
        })
        .collect();
    view! {
        div(class="modal is-active") {
            div(class="modal-background", on:click=move |_| sm.show_create.set(false))
            div(class="modal-card") {
                header(class="modal-card-head") {
                    p(class="modal-card-title") { "Create account" }
                    button(class="delete", on:click=move |_| sm.show_create.set(false))
                }
                section(class="modal-card-body") {
                    div(class="field") {
                        label(class="label") { "Homeserver" }
                        div(class="control") {
                            div(class="select is-fullwidth") {
                                select(bind:value=sm.create_hs) {
                                    option(value="") { "Select..." }
                                    (hs_opts)
                                }
                            }
                        }
                    }
                    div(class="field") {
                        label(class="label") { "Account type" }
                        div(class="control") {
                            label(class="radio") {
                                input(r#type="radio", name="ct", checked=move || sm.create_type.get() == 0,
                                    on:input=move |_| sm.create_type.set(0))
                                " Personal"
                            }
                            label(class="radio") {
                                input(r#type="radio", name="ct", checked=move || sm.create_type.get() == 1,
                                    on:input=move |_| sm.create_type.set(1))
                                " Event shared"
                            }
                            label(class="radio") {
                                input(r#type="radio", name="ct", checked=move || sm.create_type.get() == 2,
                                    on:input=move |_| sm.create_type.set(2))
                                " Club shared"
                            }
                        }
                    }
                    div(class="field") {
                        label(class="label") { "Description" }
                        div(class="control") {
                            input(class="input", placeholder="e.g. Personal SSO", bind:value=sm.create_desc)
                        }
                    }
                }
                footer(class="modal-card-foot") {
                    button(class="button is-link", on:click=move |_| {
                        let hs = sm.create_hs.get_clone();
                        if hs.is_empty() { sm.feedback.set("Pick a homeserver.".into()); return; }
                        let user = format!("kt-{}", crate::ids::gen_short_id().to_lowercase());
                        let sid = format!("@{}:{}", user, crate::event::server_name_from_homeserver(&hs));
                        sm.feedback.set(format!("Created {} — use the Login flow to sign in.", sid));
                        sm.show_create.set(false);
                    }) { "Generate" }
                    button(class="button", on:click=move |_| sm.show_create.set(false)) { "Cancel" }
                }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn view_create_modal(_model: crate::Model) -> View {
    view! {}
}

#[cfg(target_arch = "wasm32")]
fn view_login_modal(model: crate::Model) -> View {
    let sm = model.screens.accounts;
    let homeservers = crate::services::matrix::load_homeservers();
    let hs_opts: Vec<View> = homeservers
        .into_iter()
        .map(|hs| {
            let label = crate::page::home::hs_host_port(&hs.url);
            view! { option(value=hs.url) { (label) } }
        })
        .collect();
    view! {
        div(class="modal is-active") {
            div(class="modal-background", on:click=move |_| sm.show_login.set(false))
            div(class="modal-card") {
                header(class="modal-card-head") {
                    p(class="modal-card-title") { "Login with existing" }
                    button(class="delete", on:click=move |_| sm.show_login.set(false))
                }
                section(class="modal-card-body") {
                    div(class="field") {
                        label(class="label") { "Homeserver" }
                        div(class="control") {
                            div(class="select is-fullwidth") {
                                select(bind:value=sm.login_hs) {
                                    option(value="") { "Select..." }
                                    (hs_opts)
                                }
                            }
                        }
                    }
                    div(class="field") {
                        label(class="label") { "Username" }
                        div(class="control") {
                            input(class="input", placeholder="@user:homeserver", bind:value=sm.login_user)
                        }
                    }
                    div(class="field") {
                        label(class="label") { "Password" }
                        div(class="control") {
                            input(class="input", r#type="password", bind:value=sm.login_pass)
                        }
                    }
                }
                footer(class="modal-card-foot") {
                    button(class="button is-link", on:click=move |_| {
                        let hs = sm.login_hs.get_clone();
                        let user = sm.login_user.get_clone();
                        if hs.is_empty() || user.is_empty() {
                            sm.feedback.set("Fill in homeserver and username.".into());
                            return;
                        }
                        sm.show_login.set(false);
                        crate::update(model, crate::Msg::Conn(crate::sync::Msg::AddHomeserver { hs, username: user }));
                    }) { "Login" }
                    button(class="button", on:click=move |_| sm.show_login.set(false)) { "Cancel" }
                }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn view_login_modal(_model: crate::Model) -> View {
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
                    div(class="field") {
                        label(class="label") { "URL" }
                        div(class="control") {
                            input(class="input", placeholder="https://matrix.org", bind:value=sm.add_hs_url)
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
                            &a.homeserver, &a.user_id, &pass,
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
                    &c.user_id, &c.name, &c.description,
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
                        &a.user_id, &a.user_id, &a.description,
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
