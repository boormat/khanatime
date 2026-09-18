# App release pinning + schema versioning

> Status: **design for first official release.** Extends (does not replace)
> `docs/plan/record-versioning.md` (wire/localStorage schema) with an
> **app release** pin so an event stays on a locked WASM build.
>
> Related: `docs/plan/qr-join.md`, `docs/plan/room-security.md`,
> `docs/plan/remote-setup.md`, `docs/plan/multi-transport.md`.

## Why

Organisers need a **locked-in app version for the weekend**. A floating
GitHub Pages `main` deploy mid-event can change UX or wire behaviour under
them. Invite QRs must open **that** build, not “whatever is latest”.

Two version axes (do not conflate):

| Axis | What | Where |
|------|------|--------|
| **App release** | Which WASM binary / UI the official is running | Build tag, event pin, invite URL path |
| **Schema version** | Wire + localStorage shape | `TimingEvent.version`, `Envelope.version`, setup manifest |

An older app must refuse newer schema; a newer app may migrate or refuse an
event pinned to an incompatible release (warn loudly either way).

**CI / Pages / update UX:** `docs/plan/release-ci.md`.

### Semver vs a live event

| Part | Meaning | Change during a live event? |
|------|---------|------------------------------|
| **major** | Breaking changes, major reorgs | **Never** |
| **minor** | New features, screen changes, wire-format changes | **Never** |
| **patch** | Hotfix for a problem being experienced | **Yes** — see release-ci |

Patch upgrades mid-event are opt-in (open `/vX.Y.Z+1/`, optionally bump the
event pin + re-issue QR). Major/minor bumps wait for a new event.

---

## App release model

### Artefacts

- **Tagged release** `vMAJOR.MINOR.PATCH` (semver; matches `Cargo.toml` /
  release tag).
- **Versioned static host** (preferred over a single floating Pages root):

  ```
  https://boormat.github.io/khanatime/v0.2.0/
  https://boormat.github.io/khanatime/v0.2.0/index.html?...
  ```

- **Floating “latest”** (optional marketing / casual try):
  `https://boormat.github.io/khanatime/` → redirect or copy of newest tag.
  **Never** embed the floating URL in an event invite QR.

GitHub **Release assets** (zip of `dist/`) are the build input; **Pages
versioned paths** (or equivalent immutable CDN path) are what phones open.
A Release tag without a browsable URL does not help camera-scan join.

### Dev vs release

| Channel | URL / load | `app_version` baked in |
|---------|------------|-------------------------|
| Dev | `scripts/serve.sh` / trunk | `dev` + short git hash |
| Release | `/vX.Y.Z/` on Pages | exact `X.Y.Z` |

Dev may talk to a local Synapse; it must not be what practice/prod invite
QRs point at unless the organiser is deliberately running a LAN-only event
(see second-release LAN fallback).

### Event pin

At **first publish** (or when the organiser freezes invites), the event
stores:

```text
app_version: "0.2.0"          # exact pin
# optional later:
# app_version_min / app_version_max
```

Carried in the setup manifest so every device that adopts the event sees the
pin. Re-issuing invites after a deliberate upgrade = bump pin + new QRs
(organiser action, not silent).

**Client behaviour**

- On load / adopt: if running `app_version` ≠ event pin → blocking banner:
  “This event expects Khanatime v0.2.0; you are on v0.3.0 / dev. Open the
  invite QR (or the pinned URL) before timing.”
- Soft-warn only in pure local Demo; hard-block once published.

### Invite QR base URL

```
https://boormat.github.io/khanatime/v0.2.0/?homeserver=…&event=…&sid=…&tid=…&reg=…
```

- Query string stays as today (`docs/plan/qr-join.md`) — room ids, no aliases.
- **Path** encodes the app pin; camera → browser → correct WASM → join.
- Regenerating the QR always uses the event’s stored `app_version`, not
  “current build”.

### Version on messages + localStorage

Beyond schema versioning in `record-versioning.md`:

1. **Every outbound wire body** (setup, `KT`, parcel envelope) carries
   `app_version` (string) alongside schema `version` (u8).
2. **localStorage**: envelope / event blob records `app_version` of the
   writer; useful for support (“what built this cache?”).
3. Receivers **do not** reject on foreign `app_version` alone if schema
   decodes; the event pin gate is the organiser lock. Log mismatches in Chat
   for diagnostics.

Schema rules from `record-versioning.md` still apply: `version > current` →
skip message, never partial-decode.

---

## Open decisions (first release)

- Exact Pages layout: `/v0.2.0/` vs `/releases/0.2.0/` — pick one; document in
  deploy workflow.
- Whether publish **defaults** pin to the running build (yes) and whether
  organiser can pick an older tag from a list (nice-to-have).
- Floating latest site: redirect to newest `/v…/` or a separate unpinned demo
  only.

---

## Second release (see also multi-transport)

Not required to lock v1, but shapes QR design now so we do not paint into a
corner:

- **Dual homeserver / event webserver** — invite must work when the phone
  already has an **installed** release build (PWA / home-screen).
- **In-app scan of invite URLs**: parse query (`homeserver/event/sid/tid/reg`),
  **ignore the URL host/path** for join purposes; keep running the installed
  binary. Still honour event `app_version` pin (refuse or prompt to open the
  pinned URL if installed build ≠ pin).
- **QRs still point at the GitHub release URL** as the default camera target
  (works for officials with no app installed yet).
- **LAN fallback**: optional secondary QR or multi-frame parcel with
  `http://<laptop>/v0.2.0/?…` when the venue is offline — only for devices
  that already trust that LAN; primary printed QR remains the GitHub release
  URL when any uplink exists.

---

## Checklist (first release)

- [ ] Bake `app_version` into the wasm build (release tag / `dev+hash`)
- [ ] Deploy workflow publishes immutable `/vX.Y.Z/` (and optional latest)
- [ ] `EventInfo` / setup manifest: `app_version` pin at publish
- [ ] Invite QR / join link uses pinned base URL, not floating Pages
- [ ] Adopt / resume: mismatch banner (hard on published events)
- [ ] Wire + localStorage carry `app_version`; schema `version` per
      `record-versioning.md`
- [ ] About / Help show running `app_version`
- [ ] Docs: practice guide + remote-setup say “print the pinned QR”
- [ ] Tests: pin parse, QR base URL, reject/skip future schema version
