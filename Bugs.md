# Bug List

## Bug 1: Forget on inactive session doesn't refresh UI

**Symptom**: Clicking "Forget" on a non-active account, the row stays until manual refresh.

**Root cause**: `view_sessions` (home.rs:148-272) calls `load_sessions()` (localStorage) but only tracks `conn` and `collapsed` signals. `forget` (sync.rs:682-684) for inactive sessions calls `remove_session()` then returns immediately — no signal is bumped, so the reactive closure never re-runs.

**Fix** (2 parts):

1. `src/sync.rs:682-684` — After `remove_session`, bump `refresh`:
   ```rust
   if !is_active {
       crate::services::matrix::remove_session(&hs);
       model.screens.home.refresh.update(|v| v.wrapping_add(1));
       return;
   }
   ```

2. `src/page/home.rs` `view_sessions` — Subscribe to `refresh` inside the reactive closure (like `view_open_events` does at line 829). Add at the top of the closure body:
   ```rust
   let _ = sm.refresh.get();
   ```

---

## Bug 2: Create Event shows 0 tests, then 2 on Add Test

**Symptom**: After "Create New event", header shows "Tests / stages: 0", no stage details visible. Click "Add Test", suddenly 2 tests appear.

**Root cause**: `view_details` at `page/event.rs:562` is called directly (not in a `move ||` reactive closure). When the view is first created with a null event, `view_details` returns the "No event selected" stub and never re-evaluates. `create_draft` sets `model.app.event` but `view_details` doesn't re-run because it's not reactive.

**Fix**:

1. `src/page/event.rs:562` — Wrap `view_details` in a reactive closure:
   ```rust
   // Before:
   (view_details(model))
   // After:
   (move || view_details(model))
   ```

2. Verify `load_details` sets `edit_stages` before `editing` is toggled in `switch_to_draft`, OR rely on the existing reactive count logic (already reads from `edit_stages` when `editing` is true).

---

## Bug 3: "Repeats" and "Best X of Y" labels are confusing

**Symptom**: Column headers "Repeats" and "Best X of Y" don't clearly communicate what the fields mean.

**Root cause**: Labels are terse and use cricket/sports jargon.

**Fix** — rename fields and labels:

| Current field | New field name | New column label | Help text |
|---------------|---------------|------------------|-----------|
| `repeats` | `runs_total` | "Total runs" | "How many runs each car does in this test" |
| `best_x` | `runs_scored` | "Scored runs" | "How many of those runs count toward the score (best N of total)" |

### Backward compatibility

Use `#[serde(rename = "repeats")]` and `#[serde(rename = "best_x")]` on the new field names so existing serialized data (localStorage, Matrix room messages) still deserializes correctly:

```rust
pub struct Stage {
    pub num: u8,
    #[serde(default)]
    pub name: String,
    /// Total runs per car per test.
    #[serde(default = "default_one", rename = "repeats")]
    pub runs_total: u8,
    /// Counted runs (best N of total; N <= runs_total).
    #[serde(default = "default_one", rename = "best_x")]
    pub runs_scored: u8,
    #[serde(default)]
    pub timing: TimingStyle,
}
```

### Files to change

| File | What to change |
|------|---------------|
| `src/event.rs` | Rename `repeats` → `runs_total`, `best_x` → `runs_scored` in `Stage` struct. Add `#[serde(rename)]` for compat. Update `for_test`, `new`, `Default` impl. Update `stage_result()` doc comments and variable names (`y` → `runs_total`, etc.). |
| `src/page/event.rs` | Update column headers ("Repeats" → "Total runs", "Best X of Y" → "Scored runs"). Update `view_stage_row` (input field references). Update `view_stage_row_readonly` (display values). Update `StageAdd` handler (copies `last.runs_total` / `last.runs_scored`). Update validation in `send_batch` (`s.best_x > s.repeats` → `s.runs_scored > s.runs_total`). |
| `src/page/home.rs` | Update `StageProgress` struct fields (`min_runs` → `scored_runs`, `all_runs` → `total_runs`). Update `stage_progress()` function. Update table headers ("Done min runs (X)" → "Scored", "Done all runs (Y)" → "Total"). Update help text. |
| `src/batch.rs` | Update `stage_desc`: `"best {} of {}"` → `"{} of {} scored"`. |
| `src/event.rs` (stage_result) | Update variable names and doc comments to use `runs_total` / `runs_scored`. |

---

## Verification

After each fix:
1. `cargo fmt`
2. `./scripts/check.sh` (fmt + clippy + tests + wasm check)
3. `trunk build --release`
