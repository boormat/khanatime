# Event Admin Accounts + Accounts Page

> Status: **core infrastructure live**, publish flow partially wired.
> Related: `docs/plan/multi-transport.md`, `docs/plan/car-photos.md`.

## 1. Homeserver + Account model ✅ DONE

Types defined in `services/matrix.rs`. localStorage migration wired in
`Model::init()`.

```rust
pub struct HomeserverConfig {
    pub url: String,
    pub name: String,
    pub description: String,
    pub reg: RegistrationMode,
    pub element_link: String,
}

pub enum AccountType { Personal, EventShared, ClubShared }

pub struct Account {
    pub homeserver: String,
    pub user_id: String,
    pub description: String,
    pub account_type: AccountType,
    pub kind: StoredAuth,
    pub active: bool,
    pub event_uid: Option<String>,
}

pub struct Contact {
    pub user_id: String,
    pub name: String,
    pub description: String,
    pub phone: Option<String>,
}
```

### localStorage keys

| Key | Content |
|---|---|
| `kt_homeservers` | `Vec<HomeserverConfig>` |
| `kt_accounts` | `Vec<Account>` |
| `kt_contacts` | `Vec<Contact>` |

---

## 2. EventInfo changes ✅ DONE

Removed 8 dead fields (`entry_open`, `entry_close`, `stripe_link`, `cost`,
`max_entries`, `info_links`, `officials`, `entries_enabled`).

Added `event_homeservers`, `event_admins`, `owner`, `parent_rooms`.
Removed `homeserver`, `reg`, `space_alias`, `timing_alias`, `parent_room`,
`element_link`. Kept `space_id`, `timing_id`.

```rust
pub struct EventInfo {
    pub name: String,
    pub stages: Vec<Stage>,
    pub classes: Vec<String>,
    pub entries: Vec<Entry>,
    pub uid: String,
    pub id: String,
    pub sponsoring_club: String,
    pub year: String,
    pub event_date: String,
    pub organisers: Vec<Official>,
    pub status: EventStatus,
    pub event_homeservers: Vec<String>,
    pub event_admins: Vec<String>,
    pub owner: Option<String>,
    pub space_id: Option<String>,
    pub timing_id: Option<String>,
    pub parent_rooms: Vec<String>,
}
```

### Invite (extended for official QR)

```rust
pub struct Invite {
    pub homeserver: String,
    pub event: String,
    pub sid: String,
    pub tid: String,
    pub reg: RegistrationMode,
    pub admin_user: Option<String>,
    pub admin_pass: Option<String>,
}
```

---

## 3. Publish flow — IN PROGRESS

- [x] Confirmation dialog shows errors OR summary (single button)
- [x] Owner check blocks publish when missing
- [x] history_visibility: world_readable on room creation
- [ ] Warn if alias already taken, show in confirm modal
- [ ] Invite event_admins + grant admin PL after rooms created

### Publish steps

1. Owner account creates the space (owner is automatically a member)
2. Owner account creates the timing room (owner is automatically a member)
3. Link space ↔ timing room
4. Set history_visibility to world_readable on both rooms
5. For each event_admin: invite to space + timing room, grant admin PL
6. Publish event setup manifest to the timing room

### Room ownership model

| Role | Who | What |
|---|---|---|
| Owner | Admin account (`event.owner`) | Creates rooms, is room creator |
| Event admins | `event.event_admins` | Invited to rooms, granted admin PL |
| Organisers | `event.organisers` | Invited to rooms/chats |

---

## 4. Accounts page ✅ DONE

Homeservers list with accounts grouped under each. Contacts list (with phone
number). Create account / Login existing / Add homeserver / Add contact modals.
Share QR for accounts and contacts. QR scan button.

---

## 5. QR sharing ✅ DONE

| QR type | Payload |
|---|---|
| Account | `khanatime_account:{homeserver, user_id, password, description, account_type}` |
| Contact | `khanatime_contact:{user_id, name, description, phone}` |
| Official invite | Existing invite URL + admin_user + admin_pass |

Personal account QR shows loud warning. Shared account QR shows no warning.
Accounts can be shared as contacts (no passwords).

---

## 6. Auth model (future)

- Messages not from `event_admins` are ignored + warning
- Results validated against admin list

---

## 7. Backlog

- **Signing keys for event admins/contacts** — per-admin key pairs, public
  keys in event manifest, message signatures for tamper-proofing.
- **EventInfo field cleanup** — review `uid`/`id` consolidation.
- **Bugs.md removal** — accidentally committed.
