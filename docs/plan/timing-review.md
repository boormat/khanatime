# Timing Code Review — 2026-08-26

Deep review of stopwatch, finish, timekeeper, start, penalty, and helpers for
bugs, cleanups, and UX issues.

## High Severity

### ~~T1. `commit()` penalty signal round-trip breaks concurrent pending finishes~~ ✅ DONE
**File:** `src/khana/page/stopwatch.rs` — `commit()` (line 265)
**Detail:** `commit()` copies `pending.flags/garage/status` into the shared `sm.penalty` signals, then calls `penalty::to_ktime(sm.penalty, time_ds)`. If two pending finishes exist, the second overwrites the first's penalty state in the shared signals. The `to_ktime` should read directly from `PendingFinish` fields instead of round-tripping through shared signals.
**Fix:** Add `PendingFinish::to_ktime(&self, time_ds: u16) -> KTime` that reads `self.status/flags/garage` directly.

### ~~T2. Stop button fires with no pending start~~ ✅ DONE
**File:** `src/khana/page/stopwatch.rs` — `stop_car()` (line 207)
**Detail:** Pressing Stop for a car with no pending start creates a finish with `elapsed = 0.0s`. The Start button checks `pending_for_car` and disables itself, but Stop doesn't. Should either disable Stop or show a warning when there's no active start for the selected car.
**Fix:** In `stop_car()`, check `pending_for_car` and return early with feedback if the car isn't on course. In the view, disable the Stop button when `!has`.

### ~~T3. Start/Stop fire with no car selected~~ ✅ DONE
**File:** `src/khana/page/stopwatch.rs` — `view_action_buttons` (line 523, 543)
**Detail:** Clicking Start with no car selected calls `resolved_car()` which returns `"?"`. This creates a start for an unknown car without the user intending it. Both buttons should be disabled when `sm.car` is empty.
**Fix:** Disable Start/Stop when `sm.car.get_clone().trim().is_empty()`.

## Medium Severity

### ~~T4. `cancel_warn` is dead code~~ ✅ DONE
**File:** `src/khana/page/stopwatch.rs` (lines 85, 98, 320)
**Detail:** The `cancel_warn` signal is declared, initialized, and set to `None`, but never read in any view. The intent was a confirmation step before cancelling (which voids attached observations), but the view never checks it. Clicking Cancel immediately voids everything with no undo.
**Fix:** Either implement the confirmation UI or remove the signal.

### ~~T5. `fmt_ts` dead branch for negative age~~ ✅ DONE
**File:** `src/khana/helpers.rs` — `fmt_ts()` (line 229-231)
**Detail:** `now.saturating_sub(ts)` on `i64` never returns negative — it returns 0. The `if age_ms < 0` branch is dead code.
**Fix:** Change to `if ts > now { return time; }`.

### T6. `stop_car()` clears car field after pushing to pending
**File:** `src/khana/page/stopwatch.rs` — `stop_car()` (line 251)
**Detail:** After stopping, the car field is cleared (`sm.car.set(String::new())`). But the user might want to stop the same car again (e.g. if they hit stop accidentally) or the car is still relevant for the confirm panel. The confirm panel already shows the car, so clearing is fine UX-wise, but it means the user can't see which car they just stopped in the action row.
**Note:** Low priority — the confirm panel shows the car. No change needed unless user reports confusion.

### ~~T7. `manual_time()` doesn't clear car after adding~~ ✅ DONE
**File:** `src/khana/page/stopwatch.rs` — `manual_time()` (line 324-358)
**Detail:** After adding a manual time, `sm.time` is cleared but `sm.car` is not. The user has to manually deselect the car. Compare with `stop_car()` which does `sm.car.set(String::new())`.
**Fix:** Add `sm.car.set(String::new())` after pushing to pending.

### T8. Finish screen doesn't use `fmt_ts` for live timestamps
**File:** `src/khana/page/finish.rs` (lines 263, 297)
**Detail:** The pending starts list uses `fmt_age` (relative only) and `view_selected` uses `fmt_age` + `fmt_ds`. Neither shows the wall-clock time. The stopwatch log shows `HH:MM:SS (Xs ago)` but the finish screen shows only "42s ago". Inconsistent.
**Fix:** Use `fmt_ts` in finish.rs for consistency, or leave as-is if the different context justifies it.

### ~~T9. `view_comment` always visible even when car is empty~~ ✅ DONE
**File:** `src/khana/page/stopwatch.rs` — `view_comment()` (line 631-639)
**Detail:** The comment input shows regardless of whether a car is selected. It's only required for "?" cars. Could be hidden until a car is selected to reduce clutter.
**Fix:** Wrap in `move || { if !sm.car.get_clone().trim().is_empty() { ... } }`.

### ~~T10. `penalty::view` in finish.rs has verbose garage button~~ ✅ DONE
**File:** `src/khana/page/penalty.rs` (lines 123-138)
**Detail:** The full-width "Garage penalty (+5s) ON/OFF" text button is verbose. The stopwatch confirm panel uses a compact warehouse icon. The finish screen should match for consistency.
**Fix:** Replace with a compact icon button in `penalty::view`.

## Low Severity

### ~~T11. Duplicate `is-small is-small` class~~ ✅ DONE
**File:** `src/khana/page/stopwatch.rs` — `view_attached_events` (line 864)
**Detail:** `class=format!("button is-small is-small {}", ...)` has a duplicate class.
**Fix:** Remove the duplicate.

### ~~T12. `compute_runs_remaining` clones entries and runs separately~~ ✅ DONE
**File:** `src/khana/page/stopwatch.rs` — `compute_runs_remaining()` (lines 401, 415)
**Detail:** Two separate `with` calls that each clone. Could be combined into one.
**Fix:** Combine into a single `with` block.

### ~~T13. Duplicate `unknown_comment_required` pattern~~ ✅ DONE
**Files:** `stopwatch.rs:164-172`, `finish.rs:93-97`, `start.rs:48-52`
**Detail:** All three implement the same "?" + comment check. Should be a shared helper.
**Fix:** Extract to a shared function in `helpers.rs` or `event.rs`.

### ~~T14. `PendingFinish` fields duplicate `PenaltyModel`~~ ✅ DONE
**File:** `src/khana/page/stopwatch.rs` — `PendingFinish` (line 50-59)
**Detail:** `PendingFinish` has `status`, `flags`, `garage` fields that mirror `PenaltyModel`. This duplication is the root cause of T1. Consider making `PendingFinish` carry its own mini-penalty state (which it already does) and having `to_ktime` accept raw values.
**Note:** Already addressed by T1 fix.

### ~~T15. Stale comment in penalty.rs~~ ✅ DONE
**File:** `src/khana/page/penalty.rs` (lines 5-6)
**Detail:** Comment says "Shared by the Finish screen" but it's now also used by the stopwatch confirm panel.
**Fix:** Update comment.

## Resolved in this pass (stopwatch simplification)

All items marked ✅ above were resolved as part of the stopwatch page simplification
(worktree `work-stopwatch-tidy`, branch `feature/stopwatch-tidy`). The key structural
change was replacing `Signal<Vec<PendingFinish>>` (stack) with `Signal<Option<PendingFinish>>`
(single pending), making penalty controls read/write `PendingFinish` fields directly
(no shared signal round-trip), and locking the UI into a strict
Select → Start/Manual → Stop → Confirm workflow.

## Remaining

- **T6** — Low priority, no change needed.
- **T8** — Finish screen timestamp inconsistency, separate concern.

## Log improvements (2026-08-29)

### T16. Compact log — hide paired start/stop records ✅ DONE
**File:** `src/khana/helpers.rs` — `view_timing_log()`
**Detail:** Build a `HashSet` of UIDs from `RUN_FINISH.refs`. Filter the log: keep all finish records, plus any start/stop whose UID is NOT in that set. Collapses raw observations into just the finish record for completed runs. Orphaned starts/stops (active on-course cars) remain visible.

### T17. Click-to-edit finish records ✅ DONE
**File:** `src/khana/helpers.rs` — `view_timing_log()`, `view_edit_row()`
**Detail:** Added pencil button on each `RUN_FINISH` row. Clicking it opens an inline edit form replacing the log row. Form shows: car (read-only), time (editable seconds), status (clean/DNF/FTS/WD dropdown), flags count, garage toggle, comment text. Save calls `enqueue_amend`; cancel closes the form. Only available when `editing_uid` signal is passed (stopwatch page). New model field: `editing_observation: Signal<Option<String>>` on `StopwatchModel`.

### T18. Rework time presentation in log ✅ DONE
**File:** `src/khana/helpers.rs` — `view_timing_log()`
**Detail:** Replaced raw `format!("{:.1}", ds/10.0)` with the existing `show::ktime()` renderer. Finish records show time + flag icons + garage icon. Terminal statuses (DNF/FTS/WD/DNS) show status tags.

### T19. Stop record time — grey italic provisional display ✅ DONE
**File:** `src/khana/helpers.rs` — `view_timing_log()`
**Detail:** `RUN_STOP` records display `time_ds` as elapsed time in `has-text-grey-light` italic style. Visually marks it as provisional (will be superseded by the finish record's time).

### T20. Hide action buttons during confirm ✅ DONE
**File:** `src/khana/page/stopwatch.rs` — `view_buttons()`
**Detail:** Action buttons (Start, Stop, Manual, etc.) are hidden when the confirm panel is open (`sm.pending.is_some()`). Prevents confusing dual-state where both the confirm panel and timing buttons are visible simultaneously.

### T21. Car picker modal — select + confirm flow ✅ DONE
**File:** `src/khana/page/stopwatch.rs` — car picker modal
**Detail:** Modal now uses a two-step flow: click to highlight (yellow `is-warning`), then "Change" button to apply. Current car is `is-link`, selected car is `is-warning`, others are `is-light`. Footer has Change + Cancel buttons. Added `selected_picker_car: Signal<Option<String>>` to Model. Selection is reset when the modal closes.

### T22. Shared penalty row layout ✅ DONE
**File:** `src/khana/page/penalty.rs` — `view_penalty_row()`, `src/khana/helpers.rs` — `view_edit_row()`
**Detail:** Extracted a compact `view_penalty_row(status, flags, garage, time_ds, is_manual, on_change)` function used by both the confirm panel (sync callback writes PendingFinish fields) and the inline edit form (no-op callback). Eliminates duplicate status/garage/flags rendering between confirm and edit. The `on_change` closure must be `Clone + 'static`.

### T23. Provisional finish flow ✅ DONE
**File:** `src/khana/event.rs`, `src/khana/page/stopwatch.rs`, `src/khana/helpers.rs`
**Detail:** Major architecture change: Stop now creates a provisional `RUN_FINISH` record in the log (not in outbox), auto-opens the inline edit form, and Confirm sends it to the outbox. Manual time entry uses the same flow. Replaced `PendingFinish`/`AttachedEvent`/`PendingMode` with `provisional_uid: Signal<Option<String>>`. Added `provisional: bool` field to `RunRecord` (serde skip). Removed the separate confirm panel (`view_pending`) and attached events view. Edit form shows Confirm/Cancel for provisional, Save/Cancel for existing. Car picker modal extracted to standalone `view_car_picker_modal`. All 187 tests pass.
