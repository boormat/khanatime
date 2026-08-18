use sycamore::prelude::*;

/// UI state for the accounts page.
#[derive(Clone, Copy)]
pub struct Model {
    pub refresh: Signal<u8>,
}

impl Model {
    pub fn new() -> Self {
        Self {
            refresh: create_signal(0u8),
        }
    }
}

pub fn init() -> Model {
    Model::new()
}

pub fn view(model: crate::Model) -> View {
    let _ = model;
    view! {
        div(class="section") {
            h1(class="title is-4") { "Accounts" }
            (view_homeservers())
            (view_contacts())
        }
    }
}

fn view_homeservers() -> View {
    #[cfg(target_arch = "wasm32")]
    {
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
            let empty = hs_accounts.is_empty();
            let mut items: Vec<View> = Vec::new();
            for a in hs_accounts {
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
                let active_tag = if a.active { " · active" } else { "" };
                items.push(view! {
                    div(class="notification is-light") {
                        p { (a.user_id) }
                        p(class="is-size-7 has-text-grey") { (desc_text) (active_tag) }
                    }
                });
            }
            let body = if empty {
                view! { p(class="has-text-grey") { "No accounts." } }
            } else {
                view! { (items) }
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
            view! {
                (heading)
                p(class="has-text-grey") { "No homeservers configured." }
            }
        } else {
            view! {
                (heading)
                (boxes)
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        view! {}
    }
}

fn view_contacts() -> View {
    #[cfg(target_arch = "wasm32")]
    {
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
            let phone = match &c.phone {
                Some(p) => format!(" · {}", p),
                None => String::new(),
            };
            items.push(view! {
                div(class="notification is-light") {
                    p { (c.user_id) }
                    p(class="is-size-7 has-text-grey") { (label) (phone) }
                }
            });
        }
        let heading = view! { h2(class="title is-5") { "Contacts" } };
        if items.is_empty() {
            view! {
                (heading)
                p(class="has-text-grey") { "No contacts yet." }
            }
        } else {
            view! {
                (heading)
                (items)
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        view! {}
    }
}
