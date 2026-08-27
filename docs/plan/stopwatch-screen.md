# Stopwatch timing screen + "Manual entry" rename

## Goal
1. Rename the command-line stage view to **Manual entry** and link to it from the top of the **Event Config** page.
2. Add a new **Stopwatch** screen (`page/stopwatch.rs`) — a big-button clock for *cooperative* timing: one official starts, another stops; both buttons always shown with the likely one highlighted; per-observation **void** so officials can undo bad starts/stops. Reads `model.app.runs`, which already merges remote room messages, so coop works across phones with zero new plumbing.

Keep Start/Finish screens. Penalty chips appear **after** STOP. Per-official car picker. No settle delay (highlight tracks state immediately).

## Part 1 — Manual entry rename + link (small)

- `src/khana/page/timekeeper.rs`: change the `view` heading from `"Event: {} Stage:{}"` to **"Manual entry"** (keep stage number as secondary text).
- `src/khana/page/event.rs`: add a **"Manual entry"** button in the `level-right` action row (beside edit/copy/create) dispatching `Msg::Show(Screen::Stage)`.

## Part 2 — Stopwatch screen

### New module `src/khana/page/stopwatch.rs` + wiring

```rust
pub struct Model {
    pub test: Signal<u8>,
    pub car: Signal<String>,
    pub penalty: penalty::PenaltyModel,
    pub pending: Signal<Option<PendingFinish>>,
    pub feedback: Signal<Option<String>>,
}

pub struct PendingFinish {
    pub car: String,
    pub run: u8,
    pub elapsed_ds: u16,
}

pub enum Msg {
    Test(u8),
    Start,
    Stop,
    Commit,
    Cancel,
    Void(String), // uid to void
}
```

- `app.rs`: add `Screen::Stopwatch` (`name`/`from_name` = `"stopwatch"`), `Screens.stopwatch`, `Msg::StopwatchMsg`, dispatch. Add to `needs_event`. Navbar icon `fa fa-stopwatch`.
- `page.rs`: `pub mod stopwatch;`.

### Start/Stop (cooperative)

**Highlight**: for selected `(test, car)`, check pending starts via `event::pending_for_car(runs, test, car)` (new pure helper). If pending → STOP highlighted (car on course); else → START highlighted. `runs` already merges remote messages so a remote official's start for this car flips your highlight.

Both buttons always rendered. Highlight via `is-success` (likely) vs `is-light` (fallback).

**START**: require a car; `run = next_run`; `RunRecord { type: start, status: clean }`; `enqueue_run`.

**STOP**: if pending start → `PendingFinish { car, run: start.run, elapsed_ds }`. If orphan → `elapsed_ds = 0`, `run = next_run`. Set `pending`, show penalty chips.

### Penalties after STOP

On `pending` set: render `penalty::view` (flag/garage/NFG/status chips) + **Confirm** and **Cancel**.

- **Commit**: build `RunRecord { type: finish, run: pending.run, time_ds: pending.elapsed_ds, status: penalty.status, flags: penalty.flags }`; `upsert_ktime` + `enqueue_run`; clear pending + car.
- **Cancel**: clear pending only (nothing recorded yet, safe discard).

### Invalidate (void)

**Recent** box (like start.rs `view_last_starts`) listing recent start & finish runs for the **current test only**, newest first. Each run has a **void** button → `enqueue_void(model, &r.uid, r.test, &r.car, r.run)`.

### Pure helpers (testable in `event.rs`)

`pending_for_car(runs, test, car) -> bool` — whether the car has an unfinished start for the test. Unit tests for this and highlight logic.

## Files touched

- `src/app.rs` — Screen::Stopwatch, Screens.stopwatch, Msg::StopwatchMsg, dispatch, navbar, view_content.
- `src/khana/page.rs` — `pub mod stopwatch;`.
- `src/khana/page/stopwatch.rs` — new (model, update, view, recent/void list).
- `src/khana/page/timekeeper.rs` — heading rename.
- `src/page/event.rs` — "Manual entry" link button.
- `src/event.rs` — `pending_for_car` helper (+ tests).

## Verification

`cargo test`, `cargo fmt`, `cargo clippy --all-targets`, `./scripts/check.sh`.
