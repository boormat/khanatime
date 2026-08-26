# Timing Code Review — 2026-08-26

Deep review of stopwatch, finish, timekeeper, start, penalty, and helpers for
bugs, cleanups, and UX issues.

## High Severity

### T1. `commit()` penalty signal round-trip breaks concurrent pending finishes
**File:** `src/khana/page/stopwatch.rs` — `commit()` (line 265)
**Detail:** `commit()` copies `pending.flags/garage/status` into the shared `sm.penalty` signals, then calls `penalty::to_ktime(sm.penalty, time_ds)`. If two pending finishes exist, the second overwrites the first's penalty state in the shared signals. The `to_ktime` should read directly from `PendingFinish` fields instead of round-tripping through shared signals.
**Fix:** Add `PendingFinish::to_ktime(&self, time_ds: u16) -> KTime` that reads `self.status/flags/garage` directly.

### T2. Stop button fires with no pending start
**File:** `src/khana/page/stopwatch.rs` — `stop_car()` (line 207)
**Detail:** Pressing Stop for a car with no pending start creates a finish with `elapsed = 0.0s`. The Start button checks `pending_for_car` and disables itself, but Stop doesn't. Should either disable Stop or show a warning when there's no active start for the selected car.
**Fix:** In `stop_car()`, check `pending_for_car` and return early with feedback if the car isn't on course. In the view, disable the Stop button when `!has`.

### T3. Start/Stop fire with no car selected
**File:** `src/khana/page/stopwatch.rs` — `view_action_buttons` (line 523, 543)
**Detail:** Clicking Start with no car selected calls `resolved_car()` which returns `"?"`. This creates a start for an unknown car without the user intending it. Both buttons should be disabled when `sm.car` is empty.
**Fix:** Disable Start/Stop when `sm.car.get_clone().trim().is_empty()`.

## Medium Severity

### T4. `cancel_warn` is dead code
**File:** `src/khana/page/stopwatch.rs` (lines 85, 98, 320)
**Detail:** The `cancel_warn` signal is declared, initialized, and set to `None`, but never read in any view. The intent was a confirmation step before cancelling (which voids attached observations), but the view never checks it. Clicking Cancel immediately voids everything with no undo.
**Fix:** Either implement the confirmation UI or remove the signal.

### T5. `fmt_ts` dead branch for negative age
**File:** `src/khana/helpers.rs` — `fmt_ts()` (line 229-231)
**Detail:** `now.saturating_sub(ts)` on `i64` never returns negative — it returns 0. The `if age_ms < 0` branch is dead code.
**Fix:** Change to `if ts > now { return time; }`.

### T6. `stop_car()` clears car field after pushing to pending
**File:** `src/khana/page/stopwatch.rs` — `stop_car()` (line 251)
**Detail:** After stopping, the car field is cleared (`sm.car.set(String::new())`). But the user might want to stop the same car again (e.g. if they hit stop accidentally) or the car is still relevant for the confirm panel. The confirm panel already shows the car, so clearing is fine UX-wise, but it means the user can't see which car they just stopped in the action row.
**Note:** Low priority — the confirm panel shows the car. No change needed unless user reports confusion.

### T7. `manual_time()` doesn't clear car after adding
**File:** `src/khana/page/stopwatch.rs` — `manual_time()` (line 324-358)
**Detail:** After adding a manual time, `sm.time` is cleared but `sm.car` is not. The user has to manually deselect the car. Compare with `stop_car()` which does `sm.car.set(String::new())`.
**Fix:** Add `sm.car.set(String::new())` after pushing to pending.

### T8. Finish screen doesn't use `fmt_ts` for live timestamps
**File:** `src/khana/page/finish.rs` (lines 263, 297)
**Detail:** The pending starts list uses `fmt_age` (relative only) and `view_selected` uses `fmt_age` + `fmt_ds`. Neither shows the wall-clock time. The stopwatch log shows `HH:MM:SS (Xs ago)` but the finish screen shows only "42s ago". Inconsistent.
**Fix:** Use `fmt_ts` in finish.rs for consistency, or leave as-is if the different context justifies it.

### T9. `view_comment` always visible even when car is empty
**File:** `src/khana/page/stopwatch.rs` — `view_comment()` (line 631-639)
**Detail:** The comment input shows regardless of whether a car is selected. It's only required for "?" cars. Could be hidden until a car is selected to reduce clutter.
**Fix:** Wrap in `move || { if !sm.car.get_clone().trim().is_empty() { ... } }`.

### T10. `penalty::view` in finish.rs has verbose garage button
**File:** `src/khana/page/penalty.rs` (lines 123-138)
**Detail:** The full-width "Garage penalty (+5s) ON/OFF" text button is verbose. The stopwatch confirm panel uses a compact warehouse icon. The finish screen should match for consistency.
**Fix:** Replace with a compact icon button in `penalty::view`.

## Low Severity

### T11. Duplicate `is-small is-small` class
**File:** `src/khana/page/stopwatch.rs` — `view_attached_events` (line 864)
**Detail:** `class=format!("button is-small is-small {}", ...)` has a duplicate class.
**Fix:** Remove the duplicate.

### T12. `compute_runs_remaining` clones entries and runs separately
**File:** `src/khana/page/stopwatch.rs` — `compute_runs_remaining()` (lines 401, 415)
**Detail:** Two separate `with` calls that each clone. Could be combined into one.
**Fix:** Combine into a single `with` block.

### T13. Duplicate `unknown_comment_required` pattern
**Files:** `stopwatch.rs:164-172`, `finish.rs:93-97`, `start.rs:48-52`
**Detail:** All three implement the same "?" + comment check. Should be a shared helper.
**Fix:** Extract to a shared function in `helpers.rs` or `event.rs`.

### T14. `PendingFinish` fields duplicate `PenaltyModel`
**File:** `src/khana/page/stopwatch.rs` — `PendingFinish` (line 50-59)
**Detail:** `PendingFinish` has `status`, `flags`, `garage` fields that mirror `PenaltyModel`. This duplication is the root cause of T1. Consider making `PendingFinish` carry its own mini-penalty state (which it already does) and having `to_ktime` accept raw values.
**Note:** Already addressed by T1 fix.

### T15. Stale comment in penalty.rs
**File:** `src/khana/page/penalty.rs` (lines 5-6)
**Detail:** Comment says "Shared by the Finish screen" but it's now also used by the stopwatch confirm panel.
**Fix:** Update comment.

## Priority Order

1. **T1** — Penalty round-trip bug (concurrent pending breaks)
2. **T2** — Stop button fires with no pending start
3. **T3** — Start/Stop fire with no car
4. **T4** — Remove dead `cancel_warn` code
5. **T5** — Fix dead `fmt_ts` branch
6. **T7** — Clear car after manual time
7. **T11** — Fix duplicate class
8. **T15** — Update stale comment
9. **T12** — Combine clones in `compute_runs_remaining`
10. **T13** — Extract shared `unknown_comment_required`
