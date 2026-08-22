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

### V1. Publish: owner account must match selected homeserver
**File:** `src/khana/event.rs` — `publish_errors` (line 1594)
**Severity:** Medium
**Detail:** Currently checks owner is set and in organisers list, but doesn't verify the owner has a Matrix session on the selected homeserver. If the owner's account is on matrix.org but the event publishes to a different homeserver, the owner can't be invited/granted admin.

### V2. Publish: only allow one homeserver
**File:** `src/khana/event.rs` — `publish_errors`
**Severity:** Medium
**Detail:** Currently `event_homeservers` is a `Vec<String>` allowing multiple. User wants single-homeserver only. Need to change the picker to radio-style and update validation.

### V3. Publish: reject homeserver change after publish
**File:** `src/khana/page/event.rs` — homeserver picker (line 1938)
**Severity:** Medium
**Detail:** The UI already locks the homeserver picker for published events ("Homeservers cannot be changed after publishing"), but the validation in `publish_errors` doesn't enforce this. The data model should reject the change at the validation level too.

---

## UI/UX Improvements

### U1. Owner/Organisers pickers — confusing state
**File:** `src/khana/page/event.rs` — `view_owner_picker` (line 1778), `view_organisers_picker` (line 1838)
**Severity:** Medium
**Detail:** Toggle buttons with no clear visual indication of selected state. Need better styling — e.g. filled vs outlined, checkmark icon, or a different control (dropdown, chips with clear active state).

### U2. Event Diff report — not hardcoded per field
**File:** `src/khana/batch.rs` — `event_diff` (line 215)
**Severity:** Low
**Detail:** Each field is manually compared with a dedicated `field_diff` call. When fields are added to `EventInfo`, the diff function must be updated manually. Consider comparing serialized JSON forms (diff the `serde_json::Value` trees) or using a derive macro.
**Note:** JSON diff may lose semantic understanding (e.g. "classes added/removed" vs raw array diff). Hybrid approach: JSON diff for unknown fields, semantic diffs for known structured fields.

### U3. Start/Finish pages — not using consistent car renderers
**File:** `src/khana/page/start.rs`, `src/khana/page/finish.rs`
**Severity:** Low
**Detail:** Start page uses `pad::car_chips` for selection but doesn't use `car_tag` in its view. Finish page uses `car_tag` in the notification but `car_chips` for selection. Should standardize on `car_tag` for display and `car_chips` for selection.

### U4. Sync View — not showing all message types
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

### F2. Rename #stage to #timekeeper
**File:** `src/khana/page/stage.rs`
**Severity:** Medium
**Detail:** The manual timing page (`#stage`) should be renamed to `#timekeeper`. It needs:
- A way to view/edit/approve timing messages/events
- A results table visible in the same view

### F3. Results page: mode picker (live vs official)
**File:** `src/khana/page/results.rs`
**Severity:** Medium
**Detail:** Results page needs a mode toggle:
- **Live results**: shows raw events as they arrive
- **Official results**: shows result records from the timekeeper (approved/edited)

---

## Cross-cutting Concerns

| Item | Related Plan | Status |
|------|-------------|--------|
| Burger menu fix | `layout-navigation.md` | Planned but not implemented |
| Timing page restructure | `layout-navigation.md` | Planned (Stopwatch screen) |
| Mode selector UX | None | New — needs design |
| Home page cleanup | `layout-navigation.md` (partial) | Partially planned |
| Owner/Organisers picker | `event-admin-accounts.md` | Accounts page done, picker UX not |
| SSO login on Accounts page | `matrix-login.md` | SSO flow exists, button placement TBD |

---

## Priority Suggestion

**Phase 1 — Critical bugs:**
- B1 (entry deletion), B2 (burger menu), B3 (mode selector)

**Phase 2 — Navigation restructure (builds on `layout-navigation.md`):**
- F1 (timing page), F2 (timekeeper rename), F3 (results mode)
- U5 (comms to chat), U6 (home cleanup), U7 (handoff to QR)

**Phase 3 — Validation + polish:**
- V1 (owner homeserver), V2 (single homeserver), V3 (lock homeserver)
- U1 (picker UX), U2 (diff report), U3 (car renderers), U4 (sync view)
