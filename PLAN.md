# Khanatime — Plan

> Rust WASM khanacross timing app. Successor to the archived `khanatime26`
> Flutter prototype. This plan adapts the khanatime26 design to the Rust +
> Sycamore + Matrix rewrite. Domain rules are the same; transport and storage
> are simplified (Matrix single room, localStorage).

**Related docs:**
- `docs/KhanacrossRules.md` — timing, scoring & results rules (regs reference)
- `docs/KhanacrossStopwatch.md` — start/finish separate-record design
- `docs/research/` — comms research (Matrix, BLE, Berty, mesh, crypto)
- `docs/plan/timing-unknown.md` — UNKNOWN entry flow + resolver (timing screens)
- `docs/plan/car-numbers.md` — car numbers, entry identity, shared cars
- `docs/plan/record-versioning.md` — schema versioning for storage + Matrix wire format (first official release)
- `docs/plan/release-versioning.md` — **app release pin**: event locked to `/vX.Y.Z/` URL; QR + `app_version` on messages/localStorage
- `docs/plan/room-security.md` — timing-room lockdowns (invite-only write; QR levels 1–3; spectators read-only)
- `docs/plan/ui-e2e.md` — **Playwright E2E** for click-flow / presentation / state regressions (pre-release)
- `docs/plan/qr-join.md` — QR join links: scan-to-join bootstrap (URL → connect + adopt published event)
- `docs/plan/multi-transport.md` — multi-transport timing: QR parcel handoff + dual homeserver relay, and event/observation ids with amendments
- `docs/plan/identity-amendments.md` — Phase 1: event/observation ids + `amend`/`void` wire format v2 (implementation detail)
- `docs/plan/layout-navigation.md` — nav/layout rework: burger menu, unified stopwatch, COC event status, About page
- `docs/plan/matrix-login.md` — passwordless matrix.org login via OIDC (MAS) SSO: auth-code + PKCE flow, new-tab BroadcastChannel handoff
- `AGENTS.md` — build/test commands and code layout
- **Things to sort** (this file) — before show-and-tell / practice day / prod / 2nd release

---

## Direction

- **Single web app** (not two): one WASM app with pages for timer, results,
  event setup, rules, help. (khanatime26 tried a multi-app split; not needed.)
- **Framework:** Sycamore 0.9, Trunk build, SASS/Bulma. Elm-style
  `Model`/`Msg`/`update` in `src/main.rs`.
- **Serverless:** no Rocket server. Comms via a **single shared Matrix room per
  event** named "timing". Room history is the store-and-forward offline sync
  layer.
- **Storage:** localStorage is the local source of truth (via
  `event::load_event/save_event`, `load_times/save_times`). Matrix room history
  merges into it on join/reload.

---

## Current state (Rust repo)

| Area | State |
|------|-------|
| Pages | Home, Events, Event, Timing, Stopwatch, Timekeeper, Results, Chat, Accounts, Help, KhanaRules |
| Timer | Cooperative stopwatch (auto-attached observations, session persistence) + command-line timekeeper |
| Results | `results.rs` computes scores from `RunRecord` (start/finish pairing, elapsed, penalties) |
| Event | `event.rs`: `EventInfo{name, stages_count, classes, entries}`; entries = car/name/vehicle/classes |
| Storage | localStorage for event data; sessionStorage for stopwatch UI state (car, comment, pending finish) |
| Matrix | `matrix-sdk` 0.18 — fully wired: connect/login/resume, room join, broadcast, relay-to-room, QR parcel |
| Server | none |

### Storage model

| Key | Storage | Scope | Purpose |
|-----|---------|-------|---------|
| `kt_active_stage` | localStorage | Persistent | Which stage is selected in the timing hub |
| `kt_sw_car` | sessionStorage | Tab session | Stopwatch car selection |
| `kt_sw_comment` | sessionStorage | Tab session | Stopwatch run comment |
| `kt_sw_pending` | sessionStorage | Tab session | Uncommitted finish (PendingFinish JSON) |

**Why the split?** Stage selection persists across browser restarts — the user
returns to the same event and wants to pick up where they left off. Stopwatch
UI state (car, comment, pending finish) is transient: meaningless after tab
close, and stale pending finishes would be confusing on a fresh start.

---

## Target data model (Rust serde structs)

Stored as serde JSON in localStorage, keyed by event. Start and finish are
separate records; elapsed is computed when both exist.

```rust
struct Event {
  id: String,
  name: String,
  date: Option<i64>,          // epoch ms
  organiser: Option<String>,
  best_x: u8,                 // default 1
  best_y: u8,                 // default 1
  scheduled_tests: u8,
  status: EventStatus,        // setup | running | finished
  tests: Vec<Test>,
  entries: Vec<Entry>,
  categories: Vec<Category>,
  entry_categories: Vec<EntryCategory>,
  officials: Vec<Official>,
}

struct Test {
  test_number: u8,            // 1-indexed
  name: Option<String>,
  start_type: StartType,      // same_garage | separate
  status: TestStatus,         // pending | running | done
}

struct Entry {
  car_number: String,         // "00 0B 24TBC" — string, not int (existing code)
  driver_name: String,
  licence: Option<String>,
  passenger: Option<String>,
  status: EntryStatus,        // active | ...
  join_at_test: Option<u8>,
  scratch_test: Option<u8>,
}

struct Category { name: String, sort_order: u8, is_outright: bool }
struct EntryCategory { entry: String, category: String }
struct Official { id: String, name: Option<String>, role: OfficialRole }
```

### Start / finish events (the timing core)

```rust
struct StartEvent {
  test_number: u8,
  car_number: String,
  run_number: u8,             // auto-incremented per car per test by start official
  official_id: Option<String>,
  timestamp_ms: i64,
  status: StartStatus,        // pending | clean | DNS | jump_start
  synced_at: Option<i64>,
}
struct FinishEvent {
  test_number: u8,
  car_number: String,
  run_number: u8,
  official_id: Option<String>,
  timestamp_ms: i64,
  marker_hits: u8,            // +5s per flag hit (Rule 12.1)
  status: FinishStatus,       // pending | clean | DNF | NFG | wrong_direction
                              // | missed_stop | reversed | wrong_order | DSQ
  synced_at: Option<i64>,
}
```

**Pairing key:** `(test_number, car_number, run_number)`.

Multiple officials may submit a start (or finish) for the same key — **average
the timestamps** (regs: ≥2 stopwatches when manual). Timekeeper can discard an
outlier start or finish independently.

### Computed run_result

```
key = (test_number, car_number, run_number)
start  = avg timestamp of start_events[key]   // null if none
finish = avg timestamp of finish_events[key]  // null if none
elapsed_ms = finish - start                   // null if no finish
penalties  = start status + finish status + marker_hits
net_ms     = elapsed_ms + penalty_ms
```

---

## Penalties

| Location | Records | Penalties |
|---|---|---|
| **Start** | Start timestamp, run number | DNS (no start), jump start (**+5s flat**) |
| **Finish** | Finish timestamp, marker_hits | Flags (**+5s each**), NFG (**+5s + flags**), wrong direction, missed stop, reversed, DNF, wrong order, DSQ |

| Status / field | Effect |
|---|---|
| `marker_hits` | +5.00s × count (flags) |
| `NFG` | +5.00s **plus** marker hits (Rule 12.1) |
| wrong_direction, missed_stop, reversed, DNF | Slowest clean + 5s (capped at 2× fastest) |
| wrong_order | Slowest clean + 10s (capped) |
| DSQ | Disqualification |

```
penalty_for_run(start, finish, test_results):
  if start.status == DNS:             return SLOWEST_PLUS_10, status DNS
  penalty_ms = 0
  if start.status == jump_start:      penalty_ms += 5000
  if finish is null:                  return null, in_progress
  penalty_ms += finish.marker_hits * 5000
  match finish.status:
    clean:                            pass
    NFG:                              penalty_ms += 5000   // markers already added
    wrong_direction|missed_stop|reversed|DNF:
                                      penalty_ms += min(SLOWEST_CLEAN+5000, FASTEST_CLEAN*2)
    wrong_order:                      penalty_ms += min(SLOWEST_CLEAN+10000, FASTEST_CLEAN*2)
    DSQ:                              return DSQ
  return penalty_ms, status
```

Note: NFG is "+5 plus any marker hit" — base +5 for status plus marker_hits × 5,
**do not** double-count markers.

NFG = Not Finished Garage: stopped at finish with any part of the car outside
the garage.

### Best X of Y & aggregate

- Best X of Y operates on completed run_results (paired start+finish with
  net_ms).
- Aggregate = sum of counting run net times across tests. DNS for missing tests
  uses slowest + 10. Rank overall and **per category**.

---

## Comms — Matrix single room (current direction)

- One shared Matrix room per event, named `timing`. All officials join it
  offline-capable; devices sync when coverage allows.
- **Store-and-forward:** every timing payload is posted as a room message. Any
  device that joins (or reconnects) replays room history and merges it into
  local storage. This replaces the BLE/mesh transports researched in
  `docs/research/` (Bluetooth.md, Berty.md, StoreAndForward.md).
- Room history merging mirrors the khanatime26 Matrix spike
  (`docs/research/MessagingSpike.md`): local echo + broadcast model.
- Multiple officials on the same pairing key → multi-stopwatch averaging (see
  above). Outlier discard is a timekeeper action, not automatic.
- Identity: officials are room members; optional E2EE (Matrix device
  verification). See `docs/research/Cryptography.md` for background.
- `docs/research/Matrix.md` described a multi-room split (general/timing/
  results/safety/location) — **superseded**; one room per event.
- Extended by `docs/plan/multi-transport.md`: dual homeservers (LAN + public)
  with content-id merge + auto-relay, QR parcel handoff for no-network days,
  and generated event/observation ids with `amend`/`void`. **Phase 1
  (identity + amendments) is implemented (backend)** — wire v2 live; the
  Correct/Void UI on the timing screens is still to come. **QR parcel handoff
  is live**: export/import on Results/Events with promote-on-export and
  relay-on-reconnect (`sync::relay_to_room`); QR rendering + scanning pending.

### Payload

Compact text, broadcast as `m.room.message` (custom `msgtype`):

```
[TIME] EVENT <id> | TEST <n> | BIB <car> | RUN <r> | START <ts> | <status>
[TIME] EVENT <id> | TEST <n> | BIB <car> | RUN <r> | FINISH <ts> | <status> | FLAGS <count>
```

Or a structured JSON body if text proves lossy.

### Voice — Push-to-Talk (current direction)

Live voice chat via **Element Call as an embedded widget** (MatrixRTC/LiveKit).
The Rust `matrix-sdk` has no native VoIP on WASM (WebRTC is native-only), so the
widget is the supported path. The Rust SDK's `widget` module (feature
`experimental-widgets`), already used by Element X, hosts it:
`WidgetSettings::new_virtual_element_call_widget(...)` builds the URL and
`run_client_widget_api(...)` runs the host side.

**PTT button lives in the Khanatime app**, not inside the widget:

- Embed Element Call in a hidden/headless iframe, voice-only
  (`intent=join_existing_voice`, `showControls=false`, `header=none`), joined
  muted (`defaultAudioMuted` / `skipLobby` intent default).
- Big **hold-to-talk** button (and a keyboard shortcut, e.g. spacebar) in the
  Khanatime UI. On press → send widget action `io.element.device_mute` with
  `audio_enabled: true`; on release → `audio_enabled: false`. Element Call's
  `MuteStates` handles the action and replies with the resulting state.
- The widget's own in-call controls are hidden; our button is the only PTT.

**Targets:**
- **Group** — the shared event `timing` room; everyone hears the transmission.
- **1:1 with the COC** — a second widget instance pointed at a DM room with the
  Clerk of Course (`intent=join_existing_dm_voice` / `start_call_dm_voice`),
  with its own PTT button (e.g. hold + "COC" target toggle).

**Infrastructure (voice is inherently live — not store-and-forward):**
- Homeserver (existing Synapse) for signalling + room history.
- A deployed Element Call build (self-hosted *embedded package*).
- A LiveKit SFU + MatrixRTC auth service (`lk-jwt-service`), discovered via
  `.well-known/matrix/client` → `org.matrix.msc4143.rtc_foci`.
- Realistic deployment: one laptop at basecamp running Synapse + LiveKit +
  lk-jwt-service on the event LAN. Voice only works while connected; checkpoints
  without connectivity fall back to room-history sync (no voice).

---

## Screens

### Onboarding / Home

```
[Home] → [Pick event / create event] → [Stage | Results | Event | Rules | Help]
```

### Stage — Start mode

```
Test 3 — START          Official: Mat

            Car #17  Williams
            Run: 1   (auto-increment)

              [ ▶ START ]
          [DNS]  [Jump Start]

  Recent starts:
  Car 23 R1  2s ago ✓
```

- Run number auto-increments per car per test after each successful start.
- DNS: no timestamp, status DNS. Jump start: timestamp recorded, status
  jump_start (+5s).

### Stage — Finish mode

```
Test 3 — FINISH         Official: Sarah

  ── Waiting for finish ─────────────
  Car 17 R1  Start 11:05:23  12s ago
  Car 7  R1  Start 11:05:45   8s ago
  Car 45 R1  Start 11:06:01   2s ago

  Selected: Car 17
  [type car #] [pick from list]

              [ ⏹ FINISH ]
      (always enabled — car optional)

  Last: Car 23 — 01:12.34 ✓
```

- Tap pending car to select, or type car number / pick from entrant list.
- **⏹ FINISH always works** even with no car selected (timestamp saved; car
  assigned later via event editor).
- After finish → post-finish summary:

```
Car #17 — Run 1 — Williams

Start:   11:05:23.456  (Mat)
Finish:  11:06:35.796  (Sarah)
Elapsed: 01:12.34

Start: Clean ✓
Flags:  [−]  0  [+]     (+5s each)
[NFG] [Wrong Dir] [Missed Stop]
[Reversed] [DNF] [Wrong Order] [DSQ]

Net: 01:12.34
[ ✓ SUBMIT ]  [ Discard ]
```

### Stage — Both mode

One phone, two buttons; still creates **two** records (start + finish) with the
same official.

### Results

Tabs: **Overall** | per category. Overall = all timed drivers. Category tabs =
members of that category only.

### Event setup

Entries (car, driver, licence, passenger, classes), stages/tests count, best X
of Y, categories with Outright locked.

---

## Things to sort (release gates)

Working priority list. Check items off here; detail lives in `docs/plan/` and
`docs/bugs.md`. The milestone roadmap below (M1–M5) is historical and partly
stale — prefer this section when deciding what to do next.

### 0. Pre-release tooling — UI E2E (must start early)

Unit + `wasm-bindgen-test` miss **click flows, presentation, and cross-page
state** — those regressions keep hitting officials. Add Playwright before we
lean on practice/prod. Detail: `docs/plan/ui-e2e.md`.

- [ ] Scaffold `e2e/` Playwright project + `scripts/e2e.sh` (Trunk build/serve)
- [ ] Stable `data-testid` (or aria) hooks on Timing / Results / navbar controls
- [ ] Core specs green: Demo Start→Stop→Confirm→Results; Manual/TBA comment
      gate; provisional edit doesn’t steal focus / hide buttons; event-switch
      clears car/stage hangover
- [ ] Desktop + one mobile Chromium project; traces on failure
- [ ] Wire into CI / gate deploy once stable (may start advisory in `check.sh`)
- [ ] Document in `AGENTS.md` next to wasm-test

Do **not** wait for Matrix: first slice is Demo / localStorage-only. Publish +
QR join specs follow once Level 1 room security exists.

### 1. Before show-and-tell demo

Short scripted walkthrough (demo-training or a throwaway local event). Goal:
looks solid, doesn't crash, timing → results story is clear. Prefer the E2E
core specs green so the walkthrough isn’t the only safety net.

- [ ] Scripted walkthrough: open Demo → time a few cars (Start/Stop + Manual) →
      confirm provisional → Results update; note any panic / blank UI
- [ ] Mobile viewport pass on Timing + Results (phones are the audience)
- [ ] Help / About enough to answer "what is this?" without a live narrator
      (`docs/plan/layout-navigation.md` About page — or a short Help rewrite)
- [ ] Deployed Pages build works for the demo URL
      (https://boormat.github.io/khanatime/) — or a pinned `serve.sh` fallback
- [ ] Optional wow: QR parcel handoff between two devices (offline box)
- [ ] Known demo-killers fixed if they still reproduce (focus-steal, stale
      provisional hiding buttons, TBA comment gate) — see `docs/bugs.md` B7–B14

### 2. Before practice / training day

Published practice event on a LAN Synapse with real officials and shared
devices. Goal: join → time → sync across phones without a developer in the
room. See `docs/practice-event-guide.md` + `docs/plan/remote-setup.md`.

- [ ] Organiser setup one-pager: travel router / laptop hotspot / serve app over
      LAN http (finish `docs/plan/remote-setup.md` checklist)
- [ ] Practice event path verified end-to-end: Plan → name/year → publish →
      invite QR → phone scan/join → time → second device sees results
- [ ] Publish gaps closed enough for practice: alias-already-taken warning;
      invite organisers / grant admin PL after rooms created
      (`docs/plan/event-admin-accounts.md` §3)
- [ ] QR join camera path on the phones you actually use (jsQR fallback already
      for Brave; confirm Safari/Chrome Android)
- [ ] Shared timekeeper laptop story documented and tried (event shared login
      vs per-device accounts) — `docs/practice-event-guide.md`
- [ ] Signing / trust gate explained to officials (unsigned drops; TOFU on
      unknown keys) — no surprise "where did my times go?"
- [ ] Mistake recovery on course: void + re-enter / amend UI usable without
      Chat diagnostics (`docs/plan/identity-amendments.md` Correct/Void UI)
- [ ] Wifi-fail fallback rehearsed: QR parcel export/import + relay-on-reconnect
- [ ] Key-official real name + mobile enforced at publish (already required —
      confirm the UX is obvious)

### 3. Before prod / first official release

Hardening practice can live without — until real weekend data must survive a
deploy. **Two must-lands:** locked app releases + timing-room write lockdown.

#### 3a. App release pin + schema version (must)

Organiser locks the event to a **tagged WASM build**; invite URLs open that
build, not floating Pages. Detail: `docs/plan/release-versioning.md` +
`docs/plan/record-versioning.md`.

- [ ] Versioned host paths: `…/khanatime/vX.Y.Z/` (immutable); floating
      `…/khanatime/` is latest/demo only — **never** in event invite QRs
- [ ] Bake `app_version` into the build (`X.Y.Z` vs `dev+hash`)
- [ ] Event pin: store `app_version` on publish / freeze; setup manifest carries it
- [ ] Invite QR base URL = pinned release path + existing join query
      (`homeserver/event/sid/tid/reg`)
- [ ] Adopt/resume: hard banner if running build ≠ event pin (published events)
- [ ] Schema version on wire + localStorage envelopes; reject `version > current`
- [ ] Carry `app_version` on outbound messages and localStorage for support
- [ ] Deploy workflow publishes `/vX.Y.Z/` + optional latest; `check.sh` gates it
- [ ] About shows running `app_version`

#### 3b. Timing-room security (must)

Timing room is **not public-write**. Official write QR is a secret.
Detail: `docs/plan/room-security.md`.

- [ ] Publish: `join_rules: invite`, elevated `events_default` (writers need PL)
- [ ] Level 1 (standard): Matrix invite embedded in event/QR → join with **write
      PL**; only officials scan; leaked QR ≈ rotate invite
- [ ] Spectators: read-only invite **or** discover/peek — **no** write PL
      (default discover OK for practice)
- [ ] Level 2: same crypto/ACL, ops-only — invite shown screen-to-screen, not
      printed
- [ ] Results finalisation: mark Finished + Results UI; **no** separate public
      results room for v1 (read-only timing access is enough)
- [ ] Practice guide: treat write QR as a key; don’t post it publicly
- [ ] Keep body signing/TOFU as defence-in-depth (not a substitute for ACL)

#### 3c. Other first-release polish

- [ ] E2E harden: nav/Results specs; optional screenshots; Synapse publish/join
      smoke on pinned `/vX.Y.Z/` (`docs/plan/ui-e2e.md`)
- [ ] Panic / error reporting polish
- [ ] Mobile layout pass; decide PWA / service worker (SW removed earlier)
- [ ] COC Event Status + withdraw reason + stage closed/missed
      (`docs/plan/layout-navigation.md`)
- [ ] Timing UNKNOWN / TBA resolver (`docs/plan/timing-unknown.md`)
- [ ] Multi-stopwatch averaging + timekeeper outlier discard (regs path)
- [ ] Results: tied positions (`Pos.eq` → `=1`); Finish live timestamps (T8)
- [ ] Event-config edit-context hangover (B13 leftover)

### 4. Second release

After a locked v1 is in the wild. Detail: `docs/plan/multi-transport.md` +
`release-versioning.md` § Second release.

- [ ] Dual homeserver / event-LAN webserver support
- [ ] Invite QRs still **point at the GitHub release URL** by default
- [ ] In-app scan of invite URLs: parse query, **ignore host/path**, run the
      installed release; still honour event `app_version` pin
- [ ] LAN fallback: local served `/vX.Y.Z/` (or laptop http) when offline —
      secondary to the GitHub release QR
- [ ] Level 3 enrolment: official shows identity QR → key official scans →
      register + room invite + write PL (`room-security.md`)
- [ ] Dual-HS relay + auto-fanout (Pillar 2)

**Explicitly later (not a release gate unless an event demands it):** Voice PTT /
Element Call widget; car photos; RaptorQ fountain QR; full start/finish
domain-model rewrite in M1 (current stopwatch + observation model is the live
path); derived public results room.

---

## Roadmap / TODO

Historical milestone order (partly stale vs current app). Prefer **Things to
sort** above. Check items off as done only if still accurate.

### M1 — Domain model + storage (no Matrix yet)
- [ ] Move timing to separate start/finish event records (pairing key
  `(test, car, run)`); keep `ScoreData` path until migration complete
- [ ] Extend `EventInfo`/`Entry` with date, organiser, best_x/best_y,
  scheduled_tests, licence, passenger, join/scratch test
- [ ] Run pairing + `run_result` computation (avg start/finish, elapsed,
  penalties, net) in pure module
- [ ] Penalty calculator per table above (+ unit tests against regs examples)
- [ ] Categories many-to-many + auto-Outright; entry ↔ category editor
- [ ] Officials + role (official | timekeeper | competitor)
- [ ] Best X of Y + aggregate + per-category ranking in `results.rs`

### M2 — Stage timer UX
- [ ] Start/finish/both modes with run-number auto-increment
- [ ] Post-finish summary sheet (flags stepper, status chips, net preview)
- [ ] Multi-stopwatch averaging + timekeeper outlier discard
- [ ] Keep + harden the command-line entry (`parse_command`); add tap paths

### M3 — Matrix sync
- [ ] Matrix client init + login/session (matrix-sdk `js` + `indexeddb`)
- [ ] Join/create event room named `timing`; QR/invite for officials
- [ ] Broadcast start/finish payloads; local echo
- [ ] Replay room history on join/reload → merge into localStorage
- [ ] Conflict handling: outlier discard, duplicate suppression, official
  precedence
- [ ] Broadcast results / publish to room
- [ ] Voice PTT: embed Element Call voice widget + host via `matrix-sdk` widget
  module; hold-to-talk button (DeviceMute actions) for group and 1:1-COC;
  bundle/configure LiveKit SFU + lk-jwt-service for the event LAN

### M4 — Event + results polish
- [x] Event editor UI (entries, classes, stages, best X of Y)
- [x] Events hub screen (demo / search published / plan new / saved)
- [x] Event lifecycle: draft → publish → amend-only; demo local-only;
  publish validation (stages present, no timing data)
- [x] Category result tabs — class tabs with Outright always-first + active state
- [x] Rules reference page (done — `khana_rule.rs`, keep current)

### M5 — Finish
- [x] Print stylesheet for results (navbar + publish box + tabs hidden, print-only
  header, rows kept together; `bulma-print` already vendored)
- [ ] Mobile layout, offline manifest (SW removed in `36283c6` — broke dev/testing)
- [ ] Panic/reporting polish; release build + deploy flow
- [ ] Record/message versioning (localStorage `Envelope<T>` + `TimingEvent.version`)
  — just in time for the first official release (`docs/plan/record-versioning.md`)

### Chat window (diagnostics viewer, `src/page/chat.rs`)
- [x] Pending messages show no details when selected: `LogMsg::new_pending`
  stores an empty `raw` (no server JSON yet), and the expand view pretty-printed
  only `raw`. Expanded pending lines now pretty-print the wire `body`
  (`pretty_body` — strips `KT `/`khanatime_setup:`/`khanatime_result:`
  /`khanatime_entry:` prefix, pretty-prints the JSON payload).
- [x] Pending lines all share `mid == ""`, so every pending line expanded
  together (and `++` folded them to one id). `FeedEntry` now carries
  `local_id`; expansion keys on `local_id` for pending messages, `mid` for
  room messages, so each line toggles independently.
- [x] Unit tests for `pretty_body` / `line_key` (chat.rs has none today).
- [ ] Uncommitted — working tree `src/page/chat.rs` pending; gate green
  (fmt + clippy + tests + wasm build). Note: a second agent edits code/docs
  concurrently (see `docs/plan/layout-navigation.md` gotchas).

### Home accounts + join-by-link (`src/page/home.rs`, `src/sync.rs`)
- [x] Accounts box rework: sessions rendered identically logged in/out, each
  with per-state controls (Logout only active, Login only while signed out,
  Forget only signed out); Forget is a confirm modal (`forget_target`);
  connection status folded into the box; `kt-session-row` divider style.
- [x] Join from a pasted invite link: `Msg::JoinUrl` + `Invite::from_url`
  (accepts an absolute URL or a bare `homeserver=…&…` query) + paste box on the
  pick-event area (Enter or Join button; invalid → inline error). QR-scan button
  is a placeholder until camera invite-scanning lands.
- [x] SSO unified: shared `sso_begin` (matrix.org login, add-custom-homeserver,
  and join-invite paths all identical); `join_via_link` drives SSO directly for
  a public `reg=sso` homeserver with no stored account — tab opened
  synchronously, popup blocked → invite parked + Home "sign in to join".
- [x] `sso_complete` resumes a parked join on its homeserver instead of the
  plain Home connect (tail of `connect()` untouched).
- [ ] Uncommitted — working tree `src/app.rs`, `src/event.rs`,
  `src/page/home.rs`, `src/sync.rs`, `styles/app.scss` pending; gate green
  (fmt + clippy + tests + wasm build) before commit.

---

## Open questions

Carried from `docs/research/Architecture.md`:
- Does everyone join one room, or do officials need per-test rooms at scale?
  (Single room chosen for now.)
- Timekeeper-only actions (discard outlier, revoke official) — how are they
  authorised in a serverless Matrix room?
- Identity: Matrix accounts vs event-local officials table — which is the
  source of truth?

**Resolved:** Car number format is text-only — digits-first, uppercase, no
whitespace (`^[0-9]+[A-Z]*$`, ≤8 chars).  Entry identity is a per-event counter
(`entry_no`), not the car number; the number is assigned by the timekeeper at
close-entries.  Shared cars = a free-text typed name (rego/owner/description).
See `docs/plan/car-numbers.md`.
