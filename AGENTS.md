# Agents Guide

> Khanatime — grassroots khanacross timing app (Rust WASM + Sycamore + Matrix).
> Successor to the archived `khanatime26` Flutter prototype; design history in
> `docs/research/` and the current plan in `PLAN.md`.

## Commands

```bash
# Dev server (Trunk, auto-reload on save)
trunk serve

# HTTPS dev server — OIDC/SSO testing against matrix.org (needs the /etc/hosts
# alias khanatime.test; see the script header for one-time sudo setup)
scripts/serve_https.sh

# Release build
trunk build --release

# Build, test, lint.
# same script CI runs; gates the deploy workflow
# always check code changes with this. Must pass before commit
./scripts/check.sh

# Tests
cargo test

# Format
cargo fmt

# Lint (matches CI; --all-targets also lints test code)
cargo clippy --all-targets
```

Deploy is a manual GitHub Actions workflow (`deploy.yml`) that updates
https://boormat.github.io/khanatime/ — run it from the Actions tab. It is
gated on the `check` job (`scripts/check.sh`), so a release can't ship
unlinted code.

## Framework notes

- **Sycamore 0.9** reactive framework.
  State via `Signal<T>`; views via `view! { ... }` macro.
- **Trunk** for the WASM build (see `Trunk.toml`); styling is SASS/Bulma in
  `styles/`, FontAwesome icons.
- **Elm-style architecture:** central `Model` + `Msg` enum + `update()` in
  `src/app.rs` (re-exported from `src/main.rs`). Each page owns a sub-model and
  sub-msg; page updates are dispatched from `app.rs` via `Msg::XxxMsg(...)`.
- **No server.** Client is WASM only; sync is Matrix (single shared room per
  event, extended toward dual homeservers + QR parcel handoff — see
  `docs/plan/multi-transport.md`). `matrix-sdk` 0.18 (features `js`,
  `indexeddb`) is already a dependency. `getrandom` 0.3/0.4 `wasm_js` backends
  are pinned for wasm32-unknown-unknown — keep those if you touch `Cargo.toml`.
- Sign-in is username/password (+ local self-register) with an OIDC/SSO path
  for passwordless accounts (matrix.org/MAS) — see `docs/plan/matrix-login.md`
  before touching `sync.rs` connect or `services/matrix.rs` auth/session.
- **Multiple homeserver sessions** are kept (`kt_sync_sessions` keyed by
  homeserver, active pointer) so a join/publish never creates a session or
  account when one already exists. Events store their publish `homeserver` +
  `reg` (`RegistrationMode`: `open` auto-registers, `sso` never does);
  `resume_on_load` is driven by the current event's homeserver. Join invites
  carry `homeserver/event/sid/tid/reg` only (room ids, no aliases/fallbacks) —
  see `docs/plan/qr-join.md`.

## Architecture

```
src/
├── main.rs             # entry: panic hook, render, warm-start/join-link startup
│                       #   (re-exports Model/Msg/Screen/update from app.rs)
├── lib.rs              # log! macro + web_log (console.log)
├── app.rs              # Screen enum, Model, Msg, AppState, update(), navbar, view
├── sync.rs             # Matrix connect/logout/resume/join + merge sink, QR parcel
│                       #   export/import + relay-to-room (wasm)
├── view.rs             # small view helpers
├── event.rs            # EventInfo, Entry, EntryMsg, RunRecord, KTime, KTimeTime,
│                       #   car-number/shared-car helpers, Invite, results calc
├── batch.rs             # staged-edit ops (EditOp, compact_ops, diffs)
├── ids.rs               # generated short ids (Crocker base32) + content_id(body)
│                       #   dedup key: KT bodies -> embedded observation uid
├── input.rs             # keyboard/input helpers
├── join.rs              # QR join-link arrival: parse location query + consume (wasm)
├── qr_scan.rs           # camera QR scanning for parcel import (wasm; BarcodeDetector)
├── log.rs              # per-event message log + pending outbox (localStorage);
│                       #   LogMsg.origin tracks the publishing transport (room id /
│                       #   "parcel" / outbox), publish_outbox + confirm_in_room
├── replay.rs           # pure rebuild of event/scores/runs from the log
├── timing_event.rs     # TimingEvent wire format (KT {json}, khanatime_* prefixes)
├── page.rs             # page modules + shared enqueue_run/enqueue_ktime/enqueue_entry
│                       #   + view_handoff (offline handoff box)
├── services/
│   ├── mod.rs
│   ├── qr.rs           # QR parcel codec: khanatime_parcel:{json} pack/unpack +
│   │                   #   khanatime_qr: frame codec (DEFLATE+base64) + SVG
│   │                   #   rendering + filter_timing (pure)
│   └── matrix.rs       # matrix-sdk transport wrapper (wasm)
└── page/
    ├── home.rs         # sign-in + current-event dashboard
    ├── events.rs       # event hub: demo / search published / QR / plan new / saved
    ├── chat.rs         # read-only room message view
    ├── event.rs        # event setup (classes, stages, lifecycle)
    ├── entries.rs      # competitor entry + admin close-entries workflow
    ├── stage.rs        # TIMER — command-line stopwatch entry
    │                   #   parse_command()/parse_car(), CmdParse, TimeCmd
    ├── start.rs        # start flag screen
    ├── finish.rs       # finish flag screen
    ├── pad.rs          # keypad input helper
    ├── penalty.rs      # penalty-flag input helper
    ├── results.rs      # results + score computation (ResultRow/ResultScore/Pos)
    ├── help.rs         # usage help
    └── khana_rule.rs   # rendered rules reference
```

### Domain model (see PLAN.md + docs/KhanacrossRules.md)

- `EventInfo { name, stages_count, classes, entries }`
- `Entry { entry_no, car, preferred_car, name, vehicle, shared_car, order,
  classes, status, owner }` — `entry_no` is the stable per-event PK (a
  counter); `car` is the assigned number (text: digits-first, uppercase, no
  whitespace, e.g. "00 0B 24TBC"), "" until the timekeeper assigns it at
  close-entries; `preferred_car` is the entrant's nomination; `shared_car` is a
  free-text (rego/owner/description) tying entries that share a physical car;
  `order` is the running order (0 = arrival).  See `docs/plan/car-numbers.md`.
- `ScoreData { stage, car, time }` — one record per stage per car
- `KTime` enum + `KTimeTime { time_ds, flags, garage }` — time stored in
  **deciseconds** (`time_ds`), plus flag penalties (count) and garage flag.

### Event lifecycle

- `EventStatus` is `Draft → Published → Running → Finished`. Anything after
  draft is **amend-only**: never delete data, change state instead (entries get
  `Withdrawn`, a used test/entry can't be removed — see
  `event::stage_has_timing`/`entry_has_timing` guards). Event details stay
  editable (the class list never renames; the publish homeserver/reg lock once
  published). "Clone Event" copies the opened event (entrants + tests, entrant
  state reset) into a fresh editable draft id/name/uid.
- The event **id is a random opaque key** (`event::fresh_event_id`, e.g.
  `kt-3K9XQ2MNVZ`) — never derived from the human fields. Name/club/year are
  ordinary editable fields that only matter at publish, when they form the room
  alias (`build_event_id`). An event can be created with an empty name (must be
  named before publish).
- A new event starts with a single test (`EventInfo::default`); `Add test`
  duplicates the last test's settings. In-app self-entry is **off by default**
  (`entries_enabled`); officials can always manage entries.
- The publish homeserver is picked from the **saved logins** via a checkbox
  list on the details form ("Offline only" = no homeserver, a local-only event
  until one is chosen). Homeservers are added on the Home page.
- A new event must publish before any timing starts (`event::publish_errors`:
  no demo, a name + 4-digit year (room alias), at least one stage, no
  scores/runs). Demo = `demo-training`, local only, never joins a room
  (`sync::join_current_event`/`resume_on_load` skip it).
- Saving a **draft** writes the setup manifest locally (Save Local); saving a
  **published** event is "Save and Publish" — the diff is confirmed, then the
  manifest is re-broadcast (last-writer-wins). First publish creates the rooms
  and pushes the setup manifest (which carries the entrant list) into the room;
  `sync::join_current_event` then joins the timing room so it flushes. Starting
  an edit of a published event refreshes from the room and records the base
  snapshot; if the room gained updates mid-edit, the confirm modal warns and
  merges best-effort.
- Publish homeserver/Element-link defaults are centralized in `event.rs`
  (`is_matrix_org_homeserver` / `element_link_default`); `matrix::is_matrix_org`
  delegates to them — keep that single source of truth.

## Sync model (current direction)

- Single shared Matrix room per event named "timing"; room history replays as
  store-and-forward offline sync. See `docs/research/MessagingSpike.md` and the
  Comms section of `PLAN.md`.
- **No back-compat.** Pre-release: every client's localStorage and all room
  history is disposable. Any device may start empty and any room may be
  wiped or re-created. Don't build versioning, migration, or merge/ordering
  protection around existing data surviving; plain replay-in-order +
  last-writer-wins is fine.
- **QR parcel handoff is live** (offline mode): the Handoff box on Results/Events
  exports the event's full log as a `khanatime_parcel:{json}` string and imports
  one from another device — no network needed. Export promotes the local outbox
  into the log as `origin="parcel"` (handing a message off is publishing it); on
  reconnect `sync::relay_to_room` re-broadcasts anything not yet confirmed in
  the room, and content-id dedup keeps re-import idempotent. QR rendering
  (animated `khanatime_qr:` frames rendered as SVG) and camera scanning
  (`qr_scan.rs`, browser BarcodeDetector; paste fallback) are live. Frames
  carry the parcel DEFLATE-compressed + base64; export is **Full event** or
  **Timing only** (`qr::filter_timing`), picked on the Handoff box.
- The single-room baseline is being extended to **multi-transport** (dual
  homeservers with content-id merge + auto-relay, QR parcel handoff, and
  generated event/observation ids with `amend`/`void`) — plan and wire-format
  v2 in `docs/plan/multi-transport.md`; Phase 1 (ids + amend/void wire v2)
  implementation in `docs/plan/identity-amendments.md`; QR join links (scan to
  connect + adopt a published event) are live — see `docs/plan/qr-join.md`.
  Touch the wire format (`timing_event.rs`) and
  sync plumbing with that in mind. **Wire v2 is live** (event/observation
  uids, `amend`/`void` with `target`); no fallback for old v1 bodies — they
  fail parse and are dropped, so clear localStorage + room history once.
- Voice = Push-to-Talk via an embedded Element Call voice widget (MatrixRTC /
  LiveKit), driven host-side through the Rust SDK `widget` module; needs a
  LAN homeserver + LiveKit SFU + `lk-jwt-service`. See "Voice — Push-to-Talk"
  in `PLAN.md`.

## Local Matrix stack (dev)

Synapse + Element Web run as podman containers for browser testing.

```bash
podman start synapse        # homeserver http://localhost:8008 (server_name localhost)
scripts/serve_element.sh start   # Element Web http://localhost:8085, defaults to the local Synapse
scripts/serve_element.sh stop|restart|status|log
```

Synapse registration is open (no verification); users register via the app's
sync page or Element. Each published event creates public per-event space +
timing rooms, so Element can join them to exchange chat/timing messages with
the app.

## Conventions

- `cargo fmt` before finishing; match existing code style (short fns, terse
  comments).
- Keep domain logic in `event.rs` / `results.rs` (pure, testable), UI in
  `page/`.
- Don't add new server-side components; Matrix-only.
- Multi-transport timing (QR handoff, dual homeservers, observation ids with
  `amend`/`void`) is planned — read `docs/plan/multi-transport.md` and its
  Phase 1 detail `docs/plan/identity-amendments.md` before touching the wire
  format (`timing_event.rs`) or sync plumbing.
- Navigation/layout rework (burger menu, unified stopwatch, COC event status,
  About page) is planned — see `docs/plan/layout-navigation.md` before
  restructuring `Screen`/`view_navbar` or the timing pages.
