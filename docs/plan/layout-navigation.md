# Nav & layout rework — burger menu, stopwatch, COC status

> **Stale references (2026-08-18):** This document references `EntryStatus`
> and `EntryMsg` which have been removed from event.rs. The app mode system
> (Testing/Organiser/Spectator/Official/Competitor) has been implemented.
> Burger menu is live. See `docs/plan/app-mode-and-qr-signing.md`.

## Summary

Tighten the top navbar by moving the admin-type screens under a Burger menu,
fold the three separate timing screens so all flag timing lives behind a single
visible **Stopwatch** tab, add an **Event status** (COC) read-only view and an
**About** page, and extend the data model with a withdrawal **reason** and an
explicit per-stage **closed** flag.

The visible screen a stage shows depends on its `TimingStyle`:

- **Stopwatch**-timed stage → one screen: car picker + **START / STOP / DNS**
  buttons, single operator times each car with a watch.
- **Rally**-timed stage → the official first picks their control, **Start** or
  **Finish**, which are the existing `start.rs` / `finish.rs` flag screens.

**Related:** `docs/plan/car-numbers.md` (entries/order), `PLAN.md` Comms and
Nav sections. Pre-release: no back-compat — screen hash names and the wire
format may change freely.

## Current state

- 11-icon navbar in `app.rs` (`view_navbar`): Home, Events, Event, Start,
  Finish, Stage, Results, Entries, Chat, Help, KhanaRules. No burger.
- Screens are hash-routed (`Screen::name` / `from_name`, `#results`, `#stage`
  …) with back/forward support (`push_screen_hash`, popstate/hashchange).
- Timing split three ways, each with its own `test` chip selector
  (`pad::test_chips`):
  - `page/start.rs` — start control (flag out, START, DNS).
  - `page/finish.rs` — finish control (pending starts, time entry, penalties).
  - `page/stage.rs` — command-line manual entry (`parse_command`/`TimeCmd`) +
    the Publish box.
- Home dashboard already has an "Event status" box (`home.rs`): `entry_counts`
  (`home.rs:217`) and a per-stage `stage_progress` table (`home.rs:246`,
  completed / min-runs / all-runs).
- Data gaps: `EntryStatus::Withdrawn` exists but `Entry` has no reason;
  nothing marks a stage as done/closed (only the implicit "all counting runs
  in"). `Stage { num, name, repeats, best_x, timing }` (`event.rs:58`) and
  `TimingStyle { Stopwatch, Rally }` (`event.rs:44`) already exist.
- No About page. Version available via `env!("CARGO_PKG_VERSION")` (0.1.0).
  Service worker is disabled (broke dev/test), so Chrome install prompts are
  limited — fall back to "Add to Home Screen" instructions.

## Target navigation

**Top tabs (visible):** Home · Stopwatch · Results · Chat

**Burger menu:** Event status (COC) · Event config · Manual timing entry ·
Entries · Events (picker + storage) · Help · KhanaRules · About

| Screen | Nav placement | Hash |
|---|---|---|
| Home | top | `#home` |
| **Stopwatch** (new) | top | `#stopwatch` |
| Results | top | `#results` |
| Chat | top | `#chat` |
| **Status** (new, COC) | burger | `#status` |
| Event config (`page/event.rs`) | burger | `#event` |
| Manual timing entry (`page/stage.rs`) | burger | `#stage` |
| Entries | burger | `#entries` |
| Events hub + storage | burger | `#events` |
| Help | burger | `#help` |
| KhanaRules | burger | `#rules` |
| **About** (new) | burger | `#about` |

`Screen` gains `Stopwatch` / `Status` / `About`; `Start` / `Finish` are dropped
(hashes removed — `from_name` returns `None`, unknown hash falls back to Home;
no back-compat). `needs_event` covers `Stopwatch` / `Status` / `Event` / `Stage`
/ `Entries` / `Results` / `Chat`, so burger items stay disabled without a
loaded event; the top tabs Home / Events / About / KhanaRules / Help never are.

Burger = a Bulma-style dropdown toggled by a hamburger button, backed by a
`Signal<bool>` on `AppState` (closed again on any navigation).

## Stopwatch screen — `page/stopwatch.rs` (new)

"Pick a Stage" first (chips of `#num name` with a stopwatch/rally tag), then
render per the selected stage's `TimingStyle`:

- **Stopwatch stage** → single-operator flow:
  - car picker (keypad + car chips, as today);
  - big **START** → records a `start` run (`page::enqueue_run`), arms STOP;
  - big **STOP** → resolves the car's pending start (`event::pending_starts`)
    and records the finish with auto-computed elapsed time; run numbers via
    `next_run`;
  - **DNS** for a car that doesn't run.
- **Rally stage** → a "Start control / Finish control" toggle:
  - Start control renders `start.rs` (flag out + DNS);
  - Finish control renders `finish.rs` (pending starts + time + penalties +
    FINISH).

`start.rs` / `finish.rs` stay intact as sub-screens dispatched from
`Screen::Stopwatch` (their messages keep flowing through the app.rs
`Msg::StartMsg` / `Msg::FinishMsg` dispatch); their per-screen `test` chips are
superseded by the Stopwatch stage picker (single shared stage signal lives on
the Stopwatch sub-model).

## Event status screen — `page/status.rs` (new, COC view)

Read-only overview (close-stage action lives on Event config, not here):

- Entry counts — reuse `home::entry_counts` (total / active / withdrawn /
  draft / reserve) + unassigned cars + shared cars.
- **Withdrawal list** — car, name, and `withdraw_reason`.
- **Per-stage table** — Open/Closed tag; cars **on course** (`pending_starts`);
  cars **missed** (active, stage closed, fewer than `best_x` counting runs).
- **Per-car × stage grid** — done / on course / due (running order, not yet
  started) / missed / DNS.
- "Current stage" = the lowest-numbered non-closed stage with timing activity
  (fallback: first stage).

## About screen — `page/about.rs` (new)

- App version: `env!("CARGO_PKG_VERSION")` (+ link to the repo).
- Runtime state: WASM active indicator; running mode (PWA standalone vs browser
  tab) via `matchMedia('(display-mode: standalone)')` + `navigator.standalone`
  (iOS); "installed?" yes/no.
- Install / bookmark instructions per platform: iOS Safari (Share → Add to Home
  Screen), Android Chrome (menu → Install app / Add to Home screen), desktop
  (bookmark; note the service worker is disabled so full install prompts are
  limited).
- Browser detection: `navigator.userAgent` / `userAgentData` → name + version;
  warning list for in-app browsers (Facebook, Instagram, TikTok, Messenger,
  WeChat, Line) and generic WebViews (shared/localStorage quirks).

## Data model changes (`src/event.rs`, pre-release-safe)

- `Entry.withdraw_reason: Option<String>` (`#[serde(default)]`) — captured at
  the two withdraw points in `page/entries.rs`:
  - admin withdraw `staged_set(... status = Withdrawn)` (`entries.rs:255`) —
  - self-withdraw builds the Entry at `entries.rs:840` — reason input beside
    the button; flows over the wire unchanged because `EntryMsg` carries the
    full `Entry` snapshot.
- `Stage.closed: bool` (`#[serde(default)]`) — checkbox in the stage editor
  (`page/event.rs` `view_stages`/`view_stage_list`, stage structs built at
  `:167`). "Missed a test" = active car, stage closed, < `best_x` counting
  runs.

## Smaller moves

- `page/stage.rs` Publish box → Event config (`page/event.rs`); stage.rs stays
  as the burger "Manual timing entry".
- `page/events.rs` `view_saved` rows gain "Remove from device"
  (`log::remove_event_log`) + a demo reset control (event storage management).
- `page/home.rs` keeps the slim status box (counts + shared cars); the
  per-stage progress table moves to the Status screen.

## Task list

- [ ] `event.rs`: `Entry.withdraw_reason`, `Stage.closed`, and a `stage_missed`
      helper (active, closed, < best_x counting runs) — unit tests for the
      missed calc and reason serde round-trip.
- [ ] `app.rs`: add `Screen::{Stopwatch, Status, About}`; drop `Start`/`Finish`
      from `name`/`from_name`/`needs_event`; burger open signal; rewrite
      `view_navbar` (top tabs + burger dropdown) and `view_content` dispatch
      (StartMsg/FinishMsg routed from Stopwatch).
- [ ] `page/stopwatch.rs`: stage picker + TimingStyle branch; stopwatch-mode
      START/STOP/DNS flow; rally-mode Start|Finish toggle wiring into
      `start.rs`/`finish.rs`.
- [ ] `page/status.rs`: counts, withdrawal list (reason), per-stage table,
      per-car × stage grid.
- [ ] `page/about.rs`: version, runtime mode, install instructions, browser
      name + in-app/WebView warnings.
- [ ] `page/entries.rs`: reason input on both withdraw paths.
- [ ] `page/event.rs`: `closed` checkbox in the stage editor; receive the
      Publish box from `stage.rs`.
- [ ] `page/events.rs`: storage management (remove saved event, demo reset).
- [ ] `page/home.rs`: slim status box (drop the progress table → Status).
- [ ] `main.rs` / warm-start: unchanged except hash fallback for dropped
      screens.
- [ ] Docs: this plan; AGENTS.md src tree + nav; PLAN.md Related docs.
- [ ] `./scripts/check.sh` green (fmt + clippy `--all-targets` + tests).

## Gotchas

- **Other instance edits** the repo concurrently — re-read files before
  editing; keep `cargo fmt` + `cargo clippy --all-targets` green as the merge
  surface shrinks.
- **`from_name` fallback**: dropping `#start`/`#finish` means stale URLs
  resolve to `None` → Home. Acceptable (no back-compat).
- **Stage signal ownership**: `start.rs`/`finish.rs` each own a `test: Signal`
  today; the Stopwatch must own the single stage signal and feed it to the
  sub-screens (or the sub-screens read it from the Stopwatch sub-model).
- **`pending_starts` scope**: only considers the passed `test` — the Status
  "on course" column must pass the current stage explicitly.
- **serde defaults**: `withdraw_reason` / `closed` must be `#[serde(default)]`
  so old local events and room history replay cleanly (no migration, per
  policy).
- **Stage editor rebuild**: `edit_stages` are plain `Stage` structs — adding
  `closed` is just a checkbox; keep `stage_has_timing` guard on closing a stage
  with live timing (warn, don't block).

## Manual test

1. Plan + publish an event with one stopwatch-style stage and one rally-style
   stage; close the rally stage in Event config.
2. Stopwatch tab: pick the rally stage → Start/Finish toggle; time a car on
   both controls.
3. Pick the stopwatch stage → car picker + START/STOP; time a car; check the
   finish auto-fills elapsed and the run counts.
4. Event status: counts, withdrawal list (withdraw an entry and enter a
   reason), closed stage shows the missed car for the rally stage.
5. Withdraw via self-entry — reason shows in the COC list.
6. About: version "0.1.0", browser detected, install instructions render;
   in-app browser warning appears with a spoofed UA.
7. Remove a saved event from Events → storage; demo reset works.
8. Back/forward across `#stopwatch` / `#status` / `#about` works.

## Follow-ups (not in scope)

- A "current stage" picker on the Status screen that also sets Event config
  (single place the COC moves the event on).
- Full PWA installability (re-enable the service worker behind a flag) so the
  About page can offer a real Chrome install prompt.
- Voice/widget entry point from the Event config screen.
- Results: mark tied/shared positions — `Pos.eq` is already set by
  `calc_rank` (`event.rs`) for both stage Pos and cumulative O/R; just render
  an `=` prefix (e.g. `=1`) in the stage Pos cell (`show_rs`,
  `page/results.rs`) and the O/R cell (`cum_or`). Purely presentational.