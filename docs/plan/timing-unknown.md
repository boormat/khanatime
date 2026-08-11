# Timing — UNKNOWN / unresolved entries

> Status: planned
>
> Adds an **UNKNOWN** path to the timing screens (Start, Finish). When the car
> number is missed, the official still records the timing event with a free-text
> note ("red car", "was 2 or 4", "late entrant Bill in the mazda", "test run").
> The chief timekeeper later resolves unknown records to a car (or discards
> them) from the Event / organiser page.

---

## Requirements

1. On the timing screens the *normal* path stays: pick the car from the entrant
   list, or type the number (keyboard-friendly).
2. **Always** show an UNKNOWN button. It records the timing event without a car
   number, with a notes field to capture enough info to sort it out later.
   Examples: "Was 2 or 4, not sure", "Red car", "Late entrant Bill in the
   mazda", "Bill doing a test run".
3. Unknowns are resolved later by the chief timekeeper.

## Decisions (recorded with the requester)

| Decision | Choice |
|---|---|
| Resolver location | **Event (organiser) page** for now — a section on `view_event` |
| Assigning an unknown = overwrite the car's score? | **No.** Assignment only re-points the run record. The chief timekeeper **picks and amends** the accepted stage score (Stage page) — assignment never writes `scores`. |
| Keyboard entry | **Editable text input** for car number (and finish time) alongside the on-screen keypad |

---

## Prior planning this builds on

Earlier design docs already anticipated car-less timing — this plan makes it
concrete:

- `PLAN.md` (Finish mode): "**⏹ FINISH always works** even with no car selected
  (timestamp saved; car assigned later via event editor)".
- `docs/KhanacrossStopwatch.md` (khanatime26): orphan finish
  (`status: "unassigned"`, `car: null`) + an edge-case table (finish with no
  start / comms fail / official misses finish) that the resolver must cover.
- **New in this plan** (missed before): the free-text **note** (identity hints,
  late-entrant/test-run flags) and the explicit rule that **assignment never
  writes scores** — the chief timekeeper accepts the stage score separately.

## Data model (`src/event.rs`, `src/timing_event.rs`)

- `pub const UNKNOWN_CAR: &str = "?";` — sentinel car value. Never collides with
  a real (numeric) car number.
- Add `#[serde(default)] pub note: String` to `RunRecord`.
- Add `#[serde(default, skip_serializing_if = "String::is_empty")] pub note: String`
  to `TimingEvent` (additive wire change — old messages still decode; see
  `record-versioning.md`).
- `same_run` (dedupe) must **not** include `note`: two officials may record the
  same observation with different notes; the observation identity is still
  `(type, test, car, run, ts)`.

## Start screen (`src/page/start.rs`)

- Add an always-visible **UNKNOWN** button (warning style) next to START / DNS.
- Tapping it expands an inline panel: note textarea + **Record unknown start**
  and cancel.
- Records `RunRecord { type: start, test, car: UNKNOWN_CAR,
  run: next_run(runs, test, UNKNOWN_CAR), note, ts, status: clean, official_id }`
  and broadcasts (existing `record`/`broadcast_run` path).
- Recent-starts list shows unknowns with their note.
- DNS still requires a real car.

## Finish screen (`src/page/finish.rs`)

- Same UNKNOWN button + note panel, usable in both Car and Time modes.
- Time resolution: auto-elapsed from a matching unknown pending start
  (`car == "?"`, same test) if present, else the typed time in Time mode.
- **No score write for unknowns.** `do_finish` skips `upsert_ktime` when
  `car == UNKNOWN_CAR` — the time lives on the `RunRecord` only. This keeps the
  standings free of `?` rows and matches "chief picks and amends the score".
- Pending-starts list shows unknown starts with their note so a finish official
  can match ("red car") or record the sibling unknown finish.
- Broadcasts the finish (with `note`, `car="?"`) as normal.

## Keyboard entry (`src/page/pad.rs`)

- Replace the keypad's `disabled` display `<input>` (pad.rs:46) with an
  editable `<input type="text">` driven by `on:input` into the same
  `Signal<String>`. On-screen keypad push/backspace/clear still mutate the same
  signal, so both input methods coexist. Apply to both the car number keypad
  and the finish `time` keypad.

## Resolver — Event (organiser) page section

Lists every run with `car == UNKNOWN_CAR`, grouped by test. Each group shows:

- paired start + finish (grouped by the shared `(test, "?", run)` pairing), or
  an unpaired start/finish on its own
- note(s), recorded times, elapsed, official, timestamp

Actions per group:

| Action | Effect |
|---|---|
| **Assign #** | Re-point every record in the group: `car` ← entered number, `run` ← `next_run(runs, test, car)` (same new run for both records of a pair), preserving `ts` / `time_ds` / `status` / `flags` / `note`. **Does not touch `scores`.** |
| **Discard** | Remove the run record(s) — used for "test run", "not an entrant". |

Edge cases:

- **Paired unknown start+finish** must be assigned to the **same** car and the
  **same** target run number so the pairing survives.
- **Late entrant** — unknown belongs to someone not yet on the entry list:
  resolver offers "add as new entrant" (creates `Entry` with the chosen car
  number) before/after assign.
- **Test run / non-entrant** — discard.
- Reassigning an unknown whose time the chief already accepted (Stage page)
  should leave the accepted score alone; the note stays as audit trail.

## Sync / merge (`src/page/sync.rs`)

- Mirror `note` through the incoming-run construction in the sync sink.
- Guard the finish merge: skip `upsert_time`/`upsert_ktime` when
  `te.car == UNKNOWN_CAR` (mirror the no-score rule from the finish screen).
- Unknown events flow as ordinary `start`/`finish` messages — no extra room
  type, no special handler.

## Results / standings

- Results already key by `car`; with no `?` score rows (see Finish screen) there
  is nothing to filter. Add a defensive filter (`skip car == "?"`) in the
  results builder in case a stray row arrives.

## Out of scope (future)

- **Both mode** (start+finish on one phone) — same UNKNOWN treatment when the
  screen is built.
- Role gating so only the chief timekeeper / key official can resolve; revisit
  in the identity sprint.
- "Orphan finish" without even a note (just an unassigned timestamp) — today the
  UNKNOWN flow always captures a note; a bare orphan is a `car="?"` run with an
  empty note.

---

## Checklist

- [ ] `UNKNOWN_CAR` const + `note` on `RunRecord` / `TimingEvent` (+ version bump)
- [ ] `same_run` unchanged by `note`
- [ ] Keypad → editable input (car + time)
- [ ] Start screen UNKNOWN button + note panel
- [ ] Finish screen UNKNOWN button + note panel + no-score guard
- [ ] Sync sink: note passthrough + no-score guard
- [ ] Resolver section on Event page (assign/discard/add-entrant)
- [ ] Results defensive `?` filter
- [ ] Tests for the above (run numbering for unknowns, pairing, no-score rule)
