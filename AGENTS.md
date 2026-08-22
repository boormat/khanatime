# Agents Guide

> Khanatime — grassroots khanacross timing app (Rust WASM + Sycamore + Matrix).
> Successor to the archived `khanatime26` Flutter prototype; design history in
> `docs/research/` and the current plan in `PLAN.md`.

## Commands

```bash
# Dev server (Trunk, auto-reload on save)
trunk serve -p 8080

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

## Worktrees

For anything except trivial changes, work in an isolated git worktree. The
agent MUST default to a worktree; only edit the main repo directly for trivial
single-line fixes. **All git integration (commit / merge / rebase / push /
worktree cleanup) is performed by the user, never by the agent** — the global
opencode config (`~/.config/opencode/opencode.jsonc`) denies those commands,
and that is intentional.

Use the `using-git-worktrees` skill to create and work inside the worktree
(convention: `~/work/<abbrev>-<slug>` on branch `feature/<slug>`, where
`<abbrev>` is the repo-name abbreviation, e.g. `khanatime`). The skill owns
creation/isolation only; it never commits or merges.

### Testing workflow (MANDATORY)

1. Create worktree, make changes (include `docs/bugs.md` / `docs/plan/` completion marks), run `./scripts/check.sh`
2. **Mark worktree ready**: `touch ~/work/khanatime-<feature>/test-me-please`
3. **Tell user**: "Ready to test **khanatime-<feature>** — run `scripts/test.sh`"
4. **STOP and wait** for user to test and approve
5. **After approval**: commit, squash-merge, clean up (see below)

### Agent testing (quick serve)

Agents can test their worktree with a random port and pid file:

```bash
mise run serve start khanatime-<feature>   # start serving
mise run serve status                      # check what's running
mise run serve stop khanatime-<feature>    # stop serving
```

The script allocates a random port, writes PID to `.serve.pid` in the
worktree, and shows the port in output. Kill with `kill $(cat .serve.pid)`.

### Merge workflow (after user approval)

**Goal:** No merge commits in main — only fast-forward merges.

1. Commit changes in worktree (include `docs/bugs.md` completion marks in this commit)
2. In main repo, try `git merge --ff-only feature/<name>`
3. **If FF succeeds:** skip to step 6 (cleanup)
4. **If FF fails (diverged):** squash the branch
   ```bash
   # In worktree:
   git branch baseline                          # save original
   git reset --soft main                        # move HEAD to main, keep changes staged
   git commit -m "original message"             # recommit as single commit
   git diff baseline                            # verify identical tree content
   # If diff is empty (identical):
   git branch -d baseline
   # In main repo:
   git merge --ff-only feature/<name>           # now FF works
   ```
5. **If diff shows differences:** tell user there's an issue, ask to retest
6. Tell user: "Done! Please `git pull` in the main repo"
7. Clean up:
   ```bash
   git worktree remove ~/work/khanatime-<feature>
   git branch -d feature/<name>
   ```
8. Verify all tasks are complete

Never merge to main until the user says "looks good" or approves the changes.

Never edit `docs/bugs.md` in the main repo after merge — the update must ride
along with the code commit in the worktree.

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
├── app.rs              # Screen enum, Model, Msg, Mode, KhanaState, SyncState, update(), navbar, view
├── sync.rs             # Matrix connect/logout/resume/join + merge sink, QR parcel
│                       #   export/import + relay-to-room (wasm)
├── ids.rs              # generated short ids (Crocker base32) + content_id(body)
├── input.rs            # keyboard/input helpers
├── join.rs             # QR join-link arrival: parse location query + consume (wasm)
├── qr_scan.rs          # camera QR scanning for parcel import (wasm; BarcodeDetector)
├── log.rs              # per-event message log + pending outbox (localStorage)
├── entry_app/          # independent entry management (types, batch, sync, UI)
│   ├── types.rs        #   EntryEvent, Entry, EntryStatus, EventStatus
│   ├── batch.rs        #   staged-edit ops (EditOp, compact_ops, entry_diff)
│   ├── sync.rs         #   entry wire format, enqueue_entry, parse_entry_body
│   └── mod.rs          #   Model, Msg, init(), update(), view()
├── services/
│   ├── qr.rs           # QR parcel codec (DEFLATE+base64, SVG rendering)
│   └── matrix.rs       # matrix-sdk transport wrapper (wasm)
├── page/               # shared pages (generic, not khanacross-specific)
│   ├── home.rs         # sign-in + current-event dashboard
│   ├── events.rs       # event hub: demo / search published / QR / plan new
│   ├── accounts.rs     # account/homeserver management
│   ├── chat.rs         # read-only room message view
│   └── help.rs         # usage help
└── khana/              # khanacross-timing domain
    ├── event.rs        # EventInfo, Entry, Stage, KTime, scoring, car-number
    ├── batch.rs        # staged-edit ops (EditOp, compact_ops, event_diff)
    ├── replay.rs       # pure rebuild of event/scores/runs from the log
    ├── timing_event.rs # TimingEvent wire format (KT {json}, khanatime_* prefixes)
    ├── view.rs         # KTime rendering (ktime, show_ktimetime, car_number)
    ├── helpers.rs      # enqueue_run/ktime/amend/void, view_timing_log, view_handoff
    └── page/
        ├── event.rs    # event setup (classes, stages, lifecycle)
        ├── results.rs  # results + score computation (ResultRow/ResultScore/Pos)
        ├── stage.rs    # TIMER — command-line stopwatch entry
        ├── start.rs    # start flag screen
        ├── finish.rs   # finish flag screen
        ├── stopwatch.rs # cooperative stopwatch
        ├── penalty.rs  # penalty-flag input helper
        ├── pad.rs      # keypad input helper
        └── khana_rule.rs # rendered rules reference
```

### Domain model (see PLAN.md + docs/KhanacrossRules.md)

- `khana::event::EventInfo { name, stages_count, classes, entries }`
- `khana::event::Entry { car, name, vehicle, description, shared, classes, passenger }`
  — `car` is the primary key (text: digits-first, uppercase, e.g. "00 0B 24TBC");
  entries vector position is the running order. See `docs/plan/car-numbers.md`.
- `khana::event::ScoreData { stage, car, time }` — one record per stage per car
- `KTime` enum + `KTimeTime { time_ds, flags, garage }` — time stored in
  **deciseconds** (`time_ds`), plus flag penalties (count) and garage flag.

### Event lifecycle

- `EventStatus` is `Draft → Published → Running → Finished`. Anything after
  draft is **amend-only**: never delete data, change state instead (entries get
  `Withdrawn`, a used test/entry can't be removed — see
  `event::stage_has_timing` guards). Event details stay
  editable (the class list never renames; the publish homeserver/reg lock once
  published). "Clone Event" copies the opened event (entrants + tests, entrant
  state reset) into a fresh editable draft id/name/uid.
- The event **id is a random opaque key** (`event::fresh_event_id`, e.g.
  `kt-3K9XQ2MNVZ`) — never derived from the human fields. Name/club/year are
  ordinary editable fields that only matter at publish, when they form the room
  alias (`build_event_id`). An event can be created with an empty name (must be
  named before publish).
- A new event starts with a single test (`EventInfo::default`); `Add test`
  duplicates the last test's settings. Officials can always manage entries.
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
- Navigation/layout rework: app mode picker added (see `docs/plan/app-mode-and-qr-signing.md`);
  burger menu, unified stopwatch, COC event status, About page still planned —
  see `docs/plan/layout-navigation.md` before restructuring `Screen`/`view_navbar`
  or the timing pages.
- Bug reports go in `docs/bugs.md` (not at the repo root).

## Post-plan checklist

After completing a multi-file change or feature:

1. **Dead code audit**: `cargo clippy --all-targets -- -D warnings` and
   `cargo clippy --target wasm32-unknown-unknown -- -D warnings`. Remove or
   `#[allow(dead_code)]` (with justification) any new dead code.
2. **Redundancy check**: Search for duplicate functions, orphaned helpers,
   and test helpers that no longer test anything.
3. **Stale docs**: Grep for removed concepts in comments and doc strings.
   Update or remove.
4. **Stale plans**: Check if `docs/plan/` files reference work that is now
   complete. Mark them done or remove. Also mark completed items done in
   `docs/bugs.md` — both must be in the **same commit** as the code change.
5. **Test coverage**: Verify `cargo test` passes. Check that new code paths
   have test coverage and broken tests are fixed or removed.
6. **Formatting**: Run `cargo fmt` before committing.
