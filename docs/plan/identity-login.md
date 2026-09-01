# Identity & login flow — SSO-first audit identity

> Status: **Part 1 live** (identity + join precedence). Parts 2–3 planned.
> Related: `docs/plan/matrix-login.md` (OAuth/SSO), `docs/plan/multi-transport.md`
> (dual homeservers), `docs/plan/event-admin-accounts.md` (accounts/contacts).

## Goal

A simple, clear accounts/login/welcome experience where every timing record is
attributed to a person/device for audit, and identity ties back to a Matrix id
whenever possible.

## Identity model

- **App identity** (`kt_identity`, stored, no session): the canonical audit id
  stamped on every record as `official_id`. Matrix SSO **promotes** it to the
  matrix id; a local (synapse) login sets it only when empty (never downgrades).
  Only the demo/dev path leaves it blank.
- **Session identity** (`sync.identity`): the active homeserver's user id —
  transport only (outbox sender, room membership).

## Part 1 — join precedence (live)

For an invite to an **open-registration** (local synapse) event with no stored
session:

1. **No app identity yet + matrix.org reachable** → matrix SSO first; the
   stored matrix id then drives the tieback when the join resumes.
2. **Identity present** → tie the identity's **localpart** into the event
   homeserver: register `@alice:synapse` if free.
3. **Localpart taken** → smart modal: username/password + **Create new account**
   (`alice2`… auto-suggested), **Scan your account QR** (import creds, resumes
   the join), or **Sign in manually**.
4. **matrix.org offline / no identity** → local username/password on the event
   homeserver (the same modal, manual mode).
5. **Direct from install URL** (no event) → dismissible "Sign in to Matrix.org"
   prompt on Home so recordings are attributed.

`Sso`-reg invites (matrix.org) keep the existing OAuth flow; the matrix id is
promoted to the app identity on completion.

## Files

- `services/matrix.rs` — `store_app_identity`/`load_app_identity`, `matrix_org_online`
- `khana/event.rs` — `user_id_localpart`, `extend_username` (pure, tested)
- `sync.rs` — join precedence, tieback, `Tieback*` Msgs, identity set-points
- `app.rs` — `app_identity`/`tieback` signals, global tieback modal, ImportAccount
  resumes a parked join
- `page/home.rs` — first-run SSO prompt

## Parts 2–3 (planned)

- **Part 2**: officials as contact cards (role/name/phone/matrix id/hs ids) +
  publish validation that key officials carry Real Name + mobile.
- **Part 3**: practice-event doco (published event from the Demo template,
  printable invite QR; shared timekeeper-laptop setup).