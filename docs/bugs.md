# Bugs & Feature Requests — 2026-08-20

## Bugs

### ~~B1. Entry editing silently deletes drivers~~ ✅ DONE
**File:** `src/khana/page/event.rs` — `Msg::EditEntry` handler (line 291)
**Severity:** High
**Detail:** Clicking an entry name in the edit form calls `ev.entries.remove(pos)` — the entry is pulled from the list and loaded into the quick-add text box. If the user cancels (Escape/click away), `CancelEdit` clears the text box but never restores the entry. The entry is gone.
**Fix options:**
- Restore entry on cancel (snapshot or lookup from committed event)
- Don't remove until successful re-submit (mark as "being edited")

### ~~B2. Burger menu not working correctly~~ ✅ DONE
**File:** `src/app.rs` — `view_navbar` (line 921)
**Severity:** High
**Detail:** The burger menu HTML structure doesn't follow Bulma's expected pattern. The `navbar-menu` div is inside `navbar-brand` instead of being a sibling. Bulma expects: `<nav><div class="navbar-brand">...</div><div class="navbar-menu">...</div></nav>`. Also the burger toggle and menu are both inside `navbar-brand`.
**See also:** `docs/plan/layout-navigation.md` — target nav structure already defined.

### ~~B3. Mode selector not working on desktop, unusable on mobile~~ ✅ DONE
**File:** `src/app.rs` — mode picker dropdown (line 1015)
**Severity:** High
**Detail:** The mode picker is a `is-hoverable` dropdown inside `navbar-brand`. On desktop it requires hover (unreliable). On mobile the dropdown doesn't work at all. Need a better approach — possibly a separate settings page or a modal.

### ~~B4. QR scan broken on Brave desktop~~ ✅ DONE
**File:** `src/qr_scan.rs` — `run_scan` (line 64)
**Severity:** Medium
**Detail:** `BarcodeDetector` feature-detect fails on Brave desktop (Shape Detection API disabled by default for privacy/fingerprinting). Camera works fine (`getUserMedia` OK — Element Web proves it) but QR decoding never starts. Falls back to "paste the parcel" message.
**Fix:** Add `jsQR` JS library as fallback when `BarcodeDetector` is missing. Works on Brave, Firefox, and any browser with camera + JS.

---

## Validation Issues

### ~~V1. Publish: owner account must match selected homeserver~~ ✅ DONE
**File:** `src/khana/event.rs` — `publish_errors` (line 1594)
**Severity:** Medium
**Detail:** Currently checks owner is set and in organisers list, but doesn't verify the owner has a Matrix session on the selected homeserver. If the owner's account is on matrix.org but the event publishes to a different homeserver, the owner can't be invited/granted admin.

### ~~V2. Publish: only allow one homeserver~~ ❌ DROPPED — keep multi-homeserver (future multi-transport)
**File:** `src/khana/event.rs` — `publish_errors`
**Severity:** Medium
**Detail:** Currently `event_homeservers` is a `Vec<String>` allowing multiple. User wants single-homeserver only. Need to change the picker to radio-style and update validation.

### ~~V3. Publish: reject homeserver change after publish~~ ✅ DONE
**File:** `src/khana/page/event.rs` — homeserver picker (line 1938)
**Severity:** Medium
**Detail:** The UI already locks the homeserver picker for published events ("Homeservers cannot be changed after publishing"), but the validation in `publish_errors` doesn't enforce this. The data model should reject the change at the validation level too.

---

## UI/UX Improvements

### ~~U1. Owner/Organisers pickers — confusing state~~ ✅ DONE
**File:** `src/khana/page/event.rs` — `view_owner_picker` (line 1778), `view_organisers_picker` (line 1838)
**Severity:** Medium
**Detail:** Toggle buttons with no clear visual indication of selected state. Need better styling — e.g. filled vs outlined, checkmark icon, or a different control (dropdown, chips with clear active state).

### ~~U2. Event Diff report — not hardcoded per field~~ ✅ DONE
**File:** `src/khana/batch.rs` — `event_diff` (line 215)
**Severity:** Low
**Detail:** Each field is manually compared with a dedicated `field_diff` call. When fields are added to `EventInfo`, the diff function must be updated manually. Consider comparing serialized JSON forms (diff the `serde_json::Value` trees) or using a derive macro.
**Note:** JSON diff may lose semantic understanding (e.g. "classes added/removed" vs raw array diff). Hybrid approach: JSON diff for unknown fields, semantic diffs for known structured fields.

### ~~U3. Start/Finish pages — not using consistent car renderers~~ ✅ DONE
**File:** `src/khana/page/start.rs`, `src/khana/page/finish.rs`
**Severity:** Low
**Detail:** Start page uses `pad::car_chips` for selection but doesn't use `car_tag` in its view. Finish page uses `car_tag` in the notification but `car_chips` for selection. Should standardize on `car_tag` for display and `car_chips` for selection.

### ~~U4. Sync View — not showing all message types~~ ✅ DONE
**File:** `src/page/chat.rs` — `line_summary` (line 184)
**Severity:** Low
**Detail:** Result snapshots show as just "[result]". Setup messages show as "[setup: name]". Should parse and show more detail. When expanded, should show pretty-printed JSON for all types (currently only timing events get full JSON expansion).

### ~~U5. Offline/comm status → move to Chat page~~ ✅ DONE
**File:** `src/page/home.rs` — `view_comms` (line 260)
**Severity:** Low
**Detail:** The connection status box ("Connected -- room X" / "Not connected") should move from Home to Chat page. Chat is the natural place for transport/connection diagnostics.

### ~~U6. Remove from Homepage: Add homeserver, Manage, Event admins, Change event~~ ✅ DONE
**File:** `src/page/home.rs`
**Severity:** Medium
**Detail:** These buttons clutter the home page:
- "Manage" button → remove (Accounts is in burger menu)
- "Add custom homeserver" → remove from home (available in Accounts page)
- "Event admin" button → remove from home (available in burger menu as "Event config")
- "Change event" button → move to Event config page

### ~~U7. Results: move Offline Handoff to QR page~~ ✅ DONE
**File:** `src/khana/helpers.rs` — `view_handoff` (line 226)
**Severity:** Low
**Detail:** The "Offline handoff" box (QR parcel export/import) is currently on the Results page. Should move to the QR page (`Screen::Qr`). Results page should focus on results only.

---

## Feature Requests — Navigation Restructure

### ~~F1. Timing pages: single #timing entry point with stage picker~~ ✅ DONE
**File:** New `src/khana/page/timing.rs` (or rework `stopwatch.rs`)
**Severity:** High (core UX)
**Detail:** Currently Start, Finish, Stage, Stopwatch are 4 separate screens in the navbar/burger. Replace with a single `#timing` page that:
1. Shows a list of stages with status (how many cars complete)
2. For Stopwatch-style stages: single button → existing stopwatch view
3. For Rally-style stages: two buttons side by side (Start / Finish) → existing views
4. "Leave stage" button to go back to stage list
5. Once signed in as official at a position, don't change position/stage

**See also:** `docs/plan/layout-navigation.md` — this is essentially the Stopwatch screen rework already planned.

### ~~F2. Rename #stage to #timekeeper~~ ✅ DONE
**File:** `src/khana/page/timekeeper.rs`
**Severity:** Medium
**Detail:** The manual timing page (`#stage`) should be renamed to `#timekeeper`. It needs:
- A way to view/edit/approve timing messages/events
- A results table visible in the same view

### ~~F3. Results page: mode picker (live vs official)~~ ✅ DONE
**File:** `src/khana/page/results.rs`
**Severity:** Medium
**Detail:** Results page needs a mode toggle:
- **Live results**: shows raw events as they arrive
- **Official results**: shows result records from the timekeeper (approved/edited)

---

## Cross-cutting Concerns

| Item | Status |
|------|--------|
| Burger menu fix | ✅ DONE (B2) |
| Timing page restructure | ✅ DONE (F1) |
| Mode selector UX | ✅ DONE (B3) |
| Home page cleanup | ✅ DONE (U6) |
| Owner/Organisers picker | ✅ DONE (U1) |
| SSO login on Accounts page | ✅ DONE |

---

## Timing Log Improvements

### ~~U8. Compact timing log — hide paired start/stop records~~ ✅ DONE
**File:** `src/khana/helpers.rs` — `view_timing_log()`
**Detail:** Start/stop observations already referenced by a finish record's `refs` are noise. Filter them out, keeping only orphaned starts/stops (active on-course cars) and all finish records.

### ~~U9. Click-to-edit finish records in timing log~~ ✅ DONE
**File:** `src/khana/helpers.rs` — `view_timing_log()`, `view_edit_row()`
**Detail:** Pencil button on finish records opens an inline edit form (time, flags, garage, status, comment). Save calls `enqueue_amend`. Only available on stopwatch page via `editing_observation` signal.

### ~~U10. Rework time presentation in timing log~~ ✅ DONE
**File:** `src/khana/helpers.rs` — `view_timing_log()`
**Detail:** Replace raw `format!("{:.1}", ds/10.0)` with `show::ktime()` renderer. Finish records show time + flag icons + garage icon. Terminal statuses show status tags.

### ~~U11. Stop record provisional time display~~ ✅ DONE
**File:** `src/khana/helpers.rs` — `view_timing_log()`
**Detail:** `RUN_STOP` records display `time_ds` in grey italic to indicate provisional status (will be superseded by finish record).

---

### ~~B5. BorrowMutError / signal disposal — multiple crash sites~~ ✅ DONE
**Files:** `src/app.rs` (`setup_effects`, `start_tick_timer`), `src/khana/helpers.rs` (`make_edit_state`)
**Severity:** High
**Detail:** Three distinct crash sites, all caused by Sycamore 0.9's single
`RefCell<SlotMap>` architecture (`root.nodes`).  Reading ANY signal borrows
the entire node arena immutably; writing ANY signal borrows it mutably.
**Crash sites:**
1. `app.rs:965` — `tick.set()` from `setInterval` macrotask fires while
   reactive effects hold immutable borrows on `root.nodes`.
2. `app.rs:914` — `hs_status.update()` from `spawn_local` async probe
   fires during tick-triggered re-render.
3. `helpers.rs:335` — `create_signal()` inside `make_edit_state` (called
   from the timing log's reactive closure) creates signals in the
   closure's reactive scope.  When the closure re-runs on `tick`, the old
   signals are disposed but DOM nodes still reference them.
**Fixes:**
- Timer: replaced `setInterval` + `Closure::forget` with
  `spawn_local` + `gloo_timers::future::sleep` (microtask, not macrotask).
- Cascading effects: deferred `.set()` calls via `spawn_local` (existing).
- EditState: changed `get_clone()` to `get_clone_untracked()` when reading
  `edit_state` in `view_edit_row`, preventing the timing log closure from
  tracking it as a dependency.

### ~~B6. Trust gate: unsigned / signature-invalid timing was accepted~~ ✅ DONE
**Files:** `src/signing.rs` (`SigVerdict`, `verdict_with`, `accepted`),
`src/khana/replay.rs` (`apply` gates setup + observations),
`src/sync.rs` (`handle_incoming`), `src/signing.rs` / `event.rs` / `app.rs` /
`services/matrix.rs` / `page/accounts.rs` (`load_or_generate` replaces
`.expect("signing key missing")`)
**Severity:** High
**Detail:** Verification results were computed and discarded (advise-only), so a
public room + WorldReadable history let anyone post forged `KT` bodies (or a
whole event-hijacking `khanatime_setup:`) that were applied to runs/scores. The
trust registry (`KeyTrustStatus` + `set_status`/`find_key`) existed but was never
wired to anything.
**Decision (verified with user):** default-deny. Reject **unsigned** and
**signature-invalid** observations; accept **unknown-key** on TOFU. No emergency
override — a device that cannot sign records nothing, and signing can't fail
(`load_or_generate` mints an in-memory key if storage is blocked). Setup
manifests use the same gate; a bogus/unsigned setup is ignored so the last valid
one stays in place. Rejected messages remain in the durable log (never deleted).
**Fixes:**
- One shared verdict fn (`verdict_with`) used by BOTH `replay` and
  `handle_incoming` so they can't diverge; registry loaded once per replay.
- `RunRecord`/state builds only for `accepted` verdicts.
- Removed the panic-on-missing-key paths; signing is now infallible.

### ~~B7. Run edit: 1s tick refresh steals focus from the time field~~ ✅ DONE
**File:** `src/khana/helpers.rs` — `view_timing_log` (line 182), `view_edit_row` (line 532)
**Severity:** High
**Detail:** The Log box renders inside a closure that subscribes to `model.tick.get()`
(helpers.rs:182) for the live "Xs ago" stamps. That same closure renders the whole
log subtree, including the open inline edit form. On every 1s tick the closure
re-runs and Sycamore rebuilds the edit inputs, so the time field loses focus almost
immediately after clicking into it — making it effectively un-editable.
**Target:** The edit form must not be re-created on tick. Only subscribe to `tick`
when no edit is open (the edit row itself doesn't use `now` — `view_edit_row` reads
`js_sys::Date::now()` directly at line 474). Non-edit rows keep live age stamps.
**Fix:** Hoisted the `editing`/`provisional`/`effective_editing` reads to the top of
the closure and subscribe to `tick` only when no edit is open; closing the edit
re-subscribes. Age stamps briefly freeze while an edit is open (accepted trade-off).

### ~~B8. DNS option missing from run edit~~ ✅ DONE
**File:** `src/khana/page/penalty.rs` — `view_penalty_row` (line 211, DNS chip gated
on `is_manual`); `src/khana/helpers.rs` (line 575, always passes `is_manual=false`)
**Severity:** Medium
**Detail:** The DNS chip only renders when `view_penalty_row(..., is_manual=true, ...)`.
The only call site is the run-edit form, which always passes `is_manual=false`
(helpers.rs:575), so DNS is unreachable anywhere in the UI. It used to be offered on
records created via the manual timing button.
**Target (design decision):** DNS is available on the **manual edit path** (manual-timed /
provisional records). Per the preferred model it should NOT appear on start/stop-derived
runs — "DNS is created only via the manual edit path."
**Fix:** `view_edit_row` now computes `is_manual = r.refs.is_empty()` and passes it to
`view_penalty_row`, so the DNS chip shows only for manual runs. Restored the dropped
`"dns" → KTime::NOSHO` mapping in `finish_to_ktime`, the Confirm/Save KTime builders,
and the log display, so a manual DNS finish scores and renders as DNS.

### ~~B9. Run edit regression: start/stop runs must not be hand-editable~~ ✅ DONE
**Files:** `src/khana/helpers.rs` — `view_edit_row` (lines 532-579); prior version at
commit `9fd8a957`; associated list added in `3ceca8d`
**Severity:** High
**Detail:** The run edit lets an official hand-edit the time of ANY finish record,
including runs derived from start/stop observations. Also reported: the old version
showed an associated list of the finish's timing events. Note: the current code DOES
still render it (`attached_records`, helpers.rs:496-515, rendered at 576 — added in
`3ceca8d`, which is AFTER `9fd8a957`), so verify at test time whether it is actually
visible; do not assume it was dropped.
**Target model (user decision):**
- Start/stop-derived runs: time is **read-only** (shown, not editable). To correct a
  start/stop run the official **voids the record, then creates a manual timed run** to
  replace it — keeps the model simple to explain and preserves records.
- Manual timed runs are created **intentionally** via a mechanism (a keyboard button),
  not by editing a derived run's time in place.
- DNS is only available on the manual edit path (see B8).
- Retain the associated list of timing events in the run edit.
**Fix:** `view_edit_row` renders the editable time field only when `is_manual`
(`refs.is_empty()`); start/stop-derived runs show the computed time read-only (with a
"start/stop" hint), everywhere including the provisional confirm (decision A). Manual
timed runs still get the editable field + DNS. Penalties/comment stay editable on both;
correction path is the log-row void button + the stopwatch Manual button (B11 clears
the staged start).

### ~~B10. Remote amend/void never applied to live state~~ ✅ DONE
**Files:** `src/sync.rs` (`handle_incoming`), `src/khana/event.rs`
(`apply_amend_to_runs` / `apply_void_to_runs`), `src/khana/replay.rs`
(`scores_from_runs` now `pub(crate)`), `src/khana/helpers.rs`
(`enqueue_amend`/`enqueue_void` now share the helpers)
**Severity:** High
**Detail:** `handle_incoming` only mirrored `RUN_START`/`RUN_FINISH` into live
runs/scores. A remote official's signed `amend`/`void` landed in the durable log
but never touched live state — this device showed stale results until reload.
**Fix:** After the signature gate, `amend` patches the target run and `void`
marks it voided, then scores are recomputed from runs (`scores_from_runs`) so the
Live view matches replay. Local `enqueue_amend`/`enqueue_void` use the same pure
helpers, so local and remote can't diverge. Rejected messages still stay in the
log and never reach state.

### ~~B11. Manual time leaves the car staged / waiting on a Stop~~ ✅ DONE
**File:** `src/khana/page/stopwatch.rs` — `commit()`; `src/khana/helpers.rs` —
`view_provisional_buttons` Confirm, new `void_pending_starts_for_car`
**Severity:** Low
**Detail:** If a car is on course (START sent, pending) and the official records a
manual time instead, confirming the manual finish leaves the pending START in
place — the car stays staged and the Stop button keeps showing. `manual_time`
creates a finish with `refs: vec![]`, so `pending_starts`/`pending_for_car`
(which only pair a start to a finish via refs) still consider the car on course.
**Fix:** On confirm of a refs-less (manual) finish, void any pending start(s) for
that car (via `enqueue_void`), so the car leaves the course and other devices see
the start voided. The start stays in the log as a voided record.

### ~~B12. Stale provisional hides timing UI after event reload~~ ✅ DONE
**File:** `src/khana/page/stopwatch.rs` (`reset_transient`),
`src/khana/page/timing.rs` (`EnterStage`/`LeaveStage`),
`src/app.rs` (`SetEvent`/`ClearEvent`/`DeleteEvent`)
**Severity:** Medium
**Detail:** The stopwatch top section (car chips + Start/Stop/Manual buttons) is
hidden while a provisional finish confirm is open (`provisional_uid` set). If the
official leaves without confirming and then reloads/reopens the event, the
replayed runs no longer contain the provisional (it was never enqueued), but
`provisional_uid`/`editing_observation` survive — so the timing page shows only
the Log ("No timing observations yet") with no buttons or car chips.
**Fix:** Clear the transient confirm/edit state whenever the event is loaded or
cleared (`SetEvent`/`ClearEvent`/`DeleteEvent`) and when entering/leaving a stage,
so a stale provisional can't hide the UI. Car/comment are session-persisted and
kept.

### ~~B13. Event-scoped UI state hangover (car/comment, start/finish inputs, stage)~~ ✅ DONE
**Files:** `src/app.rs` (`reset_event_ui`), `src/khana/page/stopwatch.rs`
(`clear_session`), `src/khana/page/finish.rs` / `start.rs` (`reset`),
`src/khana/page/timing.rs` (`reset_stage`), `src/khana/page/event.rs`
(`switch_to_draft`)
**Severity:** Medium
**Detail:** Audit follow-up to B12. sessionStorage car/comment is a deliberate
"survive page refresh" feature, but it was also carried across **event** changes:
a car/comment/penalty selected in event A, the timing stage selection, and
start/finish input state all hung over when a different event was loaded (or a
fresh draft created/cloned).
**Fix:** Central `app::reset_event_ui` clears on event load/clear/delete AND on
create/clone draft: stopwatch transient + session car/comment, timing stage,
start/finish inputs. Kept intentionally: view preferences (results sort/mode,
chat expanded) and the event-config edit context.
**Remaining (not fixed):** the event-config edit context (`edit_event`/`editing`)
can survive an event switch — riskier to reset because of the create/copy/discard
flow (`pre_create`, `edit_base`); follow up if it shows up in practice.

### ~~B14. Stopwatch flow review — TBA comment, staged car, time validation, car correction~~ ✅ DONE
**Files:** `src/khana/page/stopwatch.rs` (view, buttons, comment, picker modal,
`clear_after_confirm`), `src/khana/helpers.rs` (confirm/edit handlers),
`src/khana/event.rs` (`parse_time_ds`)
**Severity:** Medium (UX/data-model; several regressions from the confirm-screen
rework `34a2a2a`/`a0e38a3`)
**Detail:** Review of the stopwatch flow surfaced five related issues:
- **TBA "?" can't be manual-timed.** The stopwatch page lost its comment input
  (only restored from session), so `sm.comment` is always empty and `manual_time`'s
  `check_unknown_comment` gate silently refuses — the Manual button appears dead.
- **Manual confirm leaves the car staged.** The inline Confirm/Cancel
  (`view_provisional_buttons`) never cleared the selected car/comment/time/penalty
  (the old `commit()` did; that Msg is never dispatched). B11 already voids the
  on-course START; this was the leftover cleanup.
- **Start/Stop on TBA don't force a comment.** `start_car`/`stop_car` never called
  `check_unknown_comment`; the old `tba_blocked` button-disable + highlighted
  comment field (1a10eb3) were dropped.
- **Manual time unvalidated + wrong keyboard.** Field was `type="text"`;
  Confirm/Save did `parse::<f32>().unwrap_or(0)` so garbage → 0.0s. And
  `sm.feedback` is set everywhere but was never rendered — messages invisible.
- **Car-number correction lost.** The Change-car modal still exists but
  `show_car_picker.set(true)` was never called (old car-tag click, 4a72047); and
  the confirm closure used a stale captured `car`.
**Fix:** restore the comment input on the stopwatch page (highlighted + "required
for TBA" for "?"); restore `tba_blocked` (disable Start/Stop/Manual until a
comment is entered for "?"); add `check_unknown_comment` backstops to
`start_car`/`stop_car`; render `sm.feedback`; add `parse_time_ds` (positive time
required unless dns/dnf/fts/wd) used by Confirm/Save/amend-car; `inputmode="decimal"`
on the time field; restore the car-tag → picker-modal trigger and make
`apply_car` amend a confirmed finish (new car) while using the live record car.
`clear_after_confirm` clears the selection after a confirmed provisional.

### B15. Edit event → remove a driver → Close → Open → event doesn't open
**Files:** `src/khana/page/event.rs` (`Msg::DeleteEntry`, `save_batch`/confirm),
`src/app.rs` (`OpenSaved`), `src/khana/event.rs` (`load_event`)
**Severity:** High
**Detail:** Edit the event, remove a driver (entry), Save, **Close Event**, then
**Open** it from the Saved list — nothing happens; the event doesn't open (Home
stays "No event open"). `DeleteEntry` removes from the staged `edit_event`; the
suspect is the save/confirm → `enqueue_setup` → `load_event` (replay) path or the
published-`Join` branch of `OpenSaved`.

### ~~B16. Demo opens as Organiser, but refresh drops to Official~~ ✅ DONE
**File:** `src/app.rs` — `Model::init` / `refresh_role`
**Severity:** Medium
**Detail:** Open the demo (or any loaded event) → role shows **Organiser**. A page
refresh → **Official**. `refresh_role` was only called on `Msg::SetEvent`,
`switch_to_draft`, and identity changes — not at init — so the role signal stayed
at its default `Official` after a refresh.
**Fix:** call `refresh_role(m)` at the end of `Model::init` (event + identity are
set there), so the demo and any session-loaded event get the right role on refresh.

### B17. Deleting a published event with pending results is too easy
**Files:** `src/page/home.rs` — `view_delete_modal`; `src/app.rs` `DeleteEvent`
**Severity:** High
**Detail:** The Saved-events delete modal is generic ("Its data is removed from
this device only.") even for a **published** event with an unsent outbox
(`pending`) and recorded results — a serious loss. Needs a strong warning for
published events with a pending backlog/results, plus a second confirmation step
(e.g. type-to-confirm).

### B18. Stage/test naming — standardise to T1 / "T1: name"
**Files:** app-wide (`home.rs` per-test tags, `timing.rs` stage list + header,
results, event config)
**Severity:** Low (consistency)
**Detail:** Stages render as "Test 1", "Test 1 · 80%", etc. Standardise: compact
views show **T1**; where a name is shown use **T1: Dog Trial**; a Stage/Test chip
is just **T1**. (Home tags already use `T{n}`.)

### B19. Timekeeper view needs a compact stage picker with per-test status
**File:** `src/khana/page/timekeeper.rs`
**Severity:** Medium
**Detail:** The Timekeeper (manual entry) view changes stage only via the
"stage N" command-line; there's no visual stage picker. Add a compact stage
picker list at the top with the same per-test status/% colours as the Home page.

### B20. Timing page car chips — colour-code by runs left
**File:** `src/khana/page/stopwatch.rs` (`view_car_chips`)
**Severity:** Medium
**Detail:** Car chips are all the same colour. Colour-code them by how many runs
a car has left: **green** needs to run, **red** done (or too many runs), **blue**
done the minimum (`runs_scored`). Use the shared `car_attempts_done` so DNS
counts.

### B21. Timing page TBA "?" layout
**File:** `src/khana/page/stopwatch.rs` (`view_car_chips`)
**Severity:** Low
**Detail:** The TBA row wastes space with a "TBA" label tag. Drop the "TBA" tag;
make "?" a normal car chip (car icon, like the others) at the **start** of the
row of cars that have done all their runs.

### B22. "Sign in or create" modal — homeserver dropdown alignment
**File:** `src/page/accounts.rs` (`view_create_modal`, homeserver `<select>` at
line 533-538)
**Severity:** Low (UI)
**Detail:** The homeserver picker in the create/sign-in modal is a `<select>`
dropdown with poor alignment. Replace with selectable homeserver **tag/button**
pickers (consistent with the other tag pickers in the app).

### B23. SSO sign-in from #accounts redirects to Home
**File:** `src/sync.rs` (`sso_complete` / `add_homeserver` end with
`Show(Screen::Home)`)
**Severity:** Medium
**Detail:** Signing in via SSO while on the Accounts page bounces you to Home.
`CreateAccount` (app.rs) stays on Accounts — the redirect is the SSO/login path,
which should return to Accounts (or the originating screen), not Home.

### B24. SSO accounts not associated with matrix.org
**File:** `src/services/matrix.rs` (`resolved_homeserver_url`, `save_session`),
`src/sync.rs` (`sso_complete`)
**Severity:** Medium
**Detail:** `new_client("https://matrix.org")` resolves to
`matrix-client.matrix.org` (matrix.rs:674) and SSO saves the session keyed by
`client.homeserver()` (sync.rs:1129), while the homeserver config entry is
`https://matrix.org` — so SSO accounts don't associate with the matrix.org
entry (exact-string match). Non-SSO (local synapse) accounts match fine.
**Fix:** normalise the homeserver key on save (map the resolved endpoint →
canonical URL) so SSO accounts land under `https://matrix.org`.

### B25. Creating an account should also create a Contact
**File:** `src/app.rs` (`Msg::CreateAccount`)
**Severity:** Medium
**Detail:** Creating an account doesn't add the user to contacts. Add a separate
Contact on account create; deleting the account must **not** remove the contact.

### B26. Destructive deletes need a deliberate gesture + confirm
**Files:** app-wide (homeserver/account/saved-event delete buttons)
**Severity:** Medium
**Detail:** Delete actions fire on a stray click. Prefer a deliberate gesture +
a confirm step: click-and-hold on mobile (feasible on WASM via pointer events),
and on desktop right-click / hold / explicit X button — then confirm.

### B27. Signing-key display → compact tag + share modal
**File:** `src/page/accounts.rs` (`view_signing_key`)
**Severity:** Low
**Detail:** Collapse the signing-key card to a small tag near the bottom of the
page with a **share** button that opens a modal (QR / copy URI / other) — the
general share pattern, to save space.

### B28. Add-homeserver quick-pick buttons
**File:** `src/page/accounts.rs` (`view_add_hs_modal`)
**Severity:** Low
**Detail:** Offer one-tap buttons for common local homeservers
(`localhost:8008`, `boomtime.local:8008`). Synapse's default client port is
**8008**; when deployed for real, 8008 is the standard (commonly behind a
reverse proxy).

### ~~B29. #timing — staged car replaceable after it has started~~ ✅ DONE
**File:** `src/khana/page/stopwatch.rs` (`view_action_buttons`,
`view_car_chips`), `styles/app.scss`
**Severity:** Medium
**Detail:** After pressing Start the car stays staged in the selected-car box
(`sm.car` is never cleared), but it can still be replaced — type another car
number, tap a car chip, or reopen the picker. The box is no longer
`is-clickable` once the car is on course, yet the input and chips still change
the staged car, so the user can stage a *different* car while the first is
still out on the course. Confusing: the running car vanishes from the box, and
Stop on the staged car errors ("not on course") or a second Start can be sent.
**Fix:** once the staged car has a pending start it is locked in — the car
chips and TBA chip are disabled until it finishes or is voided, and the
selected-car box turns amber (`.kt-selected-car.is-on-course`) to signal the
locked/running state.

---

## Priority Suggestion

**Phase 1 — Critical bugs:** ✅ Complete (B1–B3)

**Phase 2 — Navigation restructure:** ✅ Complete (F1–F3, U5–U7)

**Phase 3 — Validation + polish:** ✅ Complete (V1✅ V2❌dropped V3✅ U1–U4✅, U8–U11✅)
