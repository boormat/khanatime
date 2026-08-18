# Event Admin Accounts + Accounts Page

> Status: future change, not yet implemented.
> Related: `docs/plan/multi-transport.md`, `docs/plan/layout-navigation.md`,
> `docs/plan/car-photos.md`.

## 1. Homeserver + Account model

### HomeserverConfig

```rust
pub struct HomeserverConfig {
    pub url: String,
    pub name: String,
    pub description: String,
    pub reg: RegistrationMode,
    pub element_link: String,
}
```

### Account

```rust
pub struct Account {
    pub homeserver: String,
    pub user_id: String,
    pub description: String,
    pub account_type: AccountType,
    pub kind: StoredAuth,
    pub active: bool,
    pub event_uid: Option<String>,
}

pub enum AccountType {
    Personal,
    EventShared,
    ClubShared,
}
```

### Contact

```rust
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

Migration: read `kt_sync_sessions` → extract homeservers → convert sessions to
accounts.

---

## 2. EventInfo changes

### Removed (this change)

`homeserver`, `reg`, `space_id`, `space_alias`, `timing_id`, `timing_alias`,
`parent_room`, `element_link` — all redundant or moved elsewhere.

### Added (this change)

| Field | Type | Notes |
|---|---|---|
| `event_homeservers` | `Vec<String>` | Homeservers this event publishes to |
| `event_admins` | `Vec<String>` | Flat list of Matrix user IDs (authorization) |
| `owner` | `Option<String>` | Creator's personal user ID |
| `parent_rooms` | `Vec<String>` | Parent room aliases/IDs (one per HS) |

### Final EventInfo

```rust
pub struct EventInfo {
    // ---- core ----
    pub name: String,
    pub stages: Vec<Stage>,
    pub classes: Vec<String>,
    pub entries: Vec<Entry>,

    // ---- identity ----
    pub uid: String,
    pub id: String,
    pub sponsoring_club: String,
    pub year: String,
    pub event_date: String,
    pub entry_open: String,
    pub entry_close: String,
    pub stripe_link: String,
    pub cost: String,
    pub max_entries: Option<u32>,
    pub info_links: Vec<String>,
    pub organisers: Vec<Official>,
    pub officials: Vec<Official>,
    pub status: EventStatus,

    // ---- Matrix ----
    #[serde(default)]
    pub event_homeservers: Vec<String>,
    #[serde(default)]
    pub event_admins: Vec<String>,
    #[serde(default)]
    pub owner: Option<String>,

    // ---- optional config ----
    #[serde(default)]
    pub parent_rooms: Vec<String>,
    pub entries_enabled: bool,
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
    /// Admin account credentials (for official QR only).
    #[serde(default)]
    pub admin_user: Option<String>,
    #[serde(default)]
    pub admin_pass: Option<String>,
}
```

---

## 3. Publish flow

1. Confirmation dialog shows accounts, rooms, and invites
2. Create admin accounts on each homeserver
3. Create rooms — warn if alias already taken, option to continue
4. Invite owner — skip silently if already member
5. Publish setup manifest — warn if already published

Each step handles "already exists" gracefully. No recovery tracking.

---

## 4. Accounts page (burger menu)

- Homeservers list with accounts grouped under each
- Contacts list (with phone number)
- Create account / Login existing / Add homeserver / Add contact
- Share QR for accounts and contacts

---

## 5. QR sharing

| QR type | Payload |
|---|---|
| Account | `khanatime_account:{homeserver, user_id, password, description, account_type}` |
| Contact | `khanatime_contact:{user_id, name, description, phone}` |
| Official invite | Existing invite URL + admin_user + admin_pass |

---

## 6. Auth model (future)

- Messages not from `event_admins` are ignored + warning
- Results validated against admin list

---

## 7. Backlog

- **Signing keys for event admins/contacts** — per-admin key pairs, public
  keys in event manifest, message signatures for tamper-proofing. For
  inter-club or untrusted environments.
- **EventInfo field cleanup** — review `uid`/`id` consolidation, move
  `event_date`/`entry_open`/`entry_close`/`stripe_link`/`cost`/`max_entries`
  /`entries_enabled`/`organisers`/`officials` to separate config or simplify.
