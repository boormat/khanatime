# Agents Guide

> Khanatime — grassroots khanacross timing app (Rust WASM + Sycamore + Matrix).
> Successor to the archived `khanatime26` Flutter prototype; design history in
> `docs/research/` and the current plan in `PLAN.md`.

## Commands

```bash
# Dev server (Trunk, auto-reload on save)
trunk serve

# Release build
trunk build --release

# Releasable check (fmt + clippy warnings-as-errors + tests) —
# same script CI runs; gates the deploy workflow
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

## Architecture

```
src/
├── main.rs             # entry: panic hook, render, warm-start/join-link startup
│                       #   (re-exports Model/Msg/Screen/update from app.rs)
├── lib.rs              # log! macro + web_log (console.log)
├── app.rs              # Screen enum, Model, Msg, AppState, update(), navbar, view
├── sync.rs             # Matrix connect/logout/resume/join + merge sink (wasm)
├── view.rs             # small view helpers
├── event.rs            # EventInfo, Entry, EntryMsg, RunRecord, KTime, KTimeTime,
│                       #   car-number/shared-car helpers, Invite, results calc
├── batch.rs            # staged-edit ops (EditOp, compact_ops, diffs)
├── input.rs            # keyboard/input helpers
├── log.rs              # per-event message log + pending outbox (localStorage)
├── replay.rs           # pure rebuild of event/scores/runs from the log
├── timing_event.rs     # TimingEvent wire format (KT {json}, khanatime_* prefixes)
├── page.rs             # page modules + shared enqueue_run/enqueue_ktime/enqueue_entry
├── services/
│   ├── mod.rs
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
  `event::stage_has_timing`/`entry_has_timing` guards). Fixed details lock at
  publish; "Publish as New" clones to a fresh draft id.
- A new event must publish before any timing starts (`event::publish_errors`:
  no demo, at least one stage, no scores/runs). Demo = `demo-training`, local
  only, never joins a room (`sync::join_current_event`/`resume_on_load` skip it).
- Amending a published event sets `needs_sync`; "Sync setup to room" re-broadcasts
  the manifest (`matrix::send_setup`, last-writer-wins).

## Sync model (current direction)

- Single shared Matrix room per event named "timing"; room history replays as
  store-and-forward offline sync. See `docs/research/MessagingSpike.md` and the
  Comms section of `PLAN.md`.
- **No back-compat.** Pre-release: every client's localStorage and all room
  history is disposable. Any device may start empty and any room may be
  wiped or re-created. Don't build versioning, migration, or merge/ordering
  protection around existing data surviving; plain replay-in-order +
  last-writer-wins is fine.
- The single-room baseline is being extended to **multi-transport** (dual
  homeservers with content-id merge + auto-relay, QR parcel handoff, and
  generated event/observation ids with `amend`/`void`) — plan and wire-format
  v2 in `docs/plan/multi-transport.md`; Phase 1 (ids + amend/void wire v2)
  implementation in `docs/plan/identity-amendments.md`; scan-to-join bootstrap
  in `docs/plan/qr-join.md`. Touch the wire format (`timing_event.rs`) and
  sync plumbing with that in mind.
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
sync page or Element. The shared room `#timing:localhost` is public, so any
account on the server can join it from Element and exchange chat/timing
messages with the app.

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
