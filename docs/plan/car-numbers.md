# Car numbers, entry identity, and shared cars

## Summary

Car numbers are **text** (digits-first, uppercase, no whitespace) — `007`, `0`,
`00A`, `000`, `24TBC` are all distinct.  They are the public face of an entry on
timing sheets and results.  But an entry's **stable identity** is a per-event
counter (`entry_no`), assigned at creation and never reused.  The assigned car
number is settled by the timekeeper at close-entries and can only change before
that number has timing data.

## Domain model (`Entry`)

```rust
pub struct Entry {
    pub entry_no: u32,              // stable PK, per-event counter
    pub car: String,                // assigned number ("" until close-entries)
    pub preferred_car: String,      // entrant's nomination (blank/non-unique OK)
    pub name: String,
    pub vehicle: String,
    pub shared_car: Option<String>, // free text typed by entrant/official
    pub order: u32,                 // running order (0 = unset → arrival)
    pub classes: Vec<String>,
    pub status: EntryStatus,
    pub owner: Option<String>,      // Matrix id of self-entrant
    ...
}
```

- `entry_no` is assigned by `upsert_entry`/`add_entry` (when 0) and survives
  renumbering.  Legacy data is backfilled by `ensure_entry_nos()` (run at the
  end of `replay::replay`).
- `car` is what officials type at timing; scores and `RunRecord`s key on it.
- `preferred_car` is the entrant's wish; duplicates (collisions) are normal and
  resolved by the timekeeper at close-entries.
- `shared_car` is a typed name (rego, owner, description).  Entries whose names
  match (case-insensitive, whitespace-collapsed via `shared_car_key`) share a
  physical car.  Informational only — never affects numbering or timing, so it
  can change at any time.
- `order` = running order, assigned at close-entries (default arrival by
  `entry_no`).  Manual up/down moves rematerialise `order` for every entry.

## Lifecycle

1. **Competitor self-entry** (Entries page): name, preferred car (optional),
   vehicle, shared-car name (optional, with datalist autocomplete of existing
   names).  No number is assigned here.  Re-submits update the entrant's own
   active entry (matches by `owner`), preserving its assigned number + status.

2. **Close-entries** (Entries page, admin mode):
   - Review entries: confirm details, set statuses, withdraw duplicates.
   - **"Assign numbers"** button suggests a number for every unassigned active
     entry (preferred if free, else smallest free pure number), flags preferred
     collisions in the feedback line, and sets running order (`10, 20, 30…`).
   - Timekeeper edits any number (`SetCar`), moves entries up/down (`Move`),
     edits vehicle / shared-car name, withdraw/delete.
   - **Save** → confirm modal (compacted diff) → **Send** broadcasts the
     changes as individual `khanatime_entry` messages (amend-friendly,
     last-writer-wins per entry_no).

3. **Mid-event car change**: the entrant keeps their entry number — the
   timekeeper edits `vehicle`/`shared_car` on the entry, so timing data stays
   attached.  No renumber needed.

## Car number rules

- **Format**: `^[0-9]+[A-Z]*$` — digits, then optional uppercase letters only.
- **Max length**: 8 characters (`CAR_NUMBER_MAX`).
- **Normalisation** on every input path: strip all whitespace → uppercase.
  The stage command line uppercases its car token, so `00a` matches entry `00A`.
- **Uniqueness of assigned numbers**: enforced on edit (`set_car`) and in the
  suggestion pool.  Preferred numbers are *not* uniqueness-checked (the
  timekeeper resolves collisions at close-entries).
- **No number recycling**: `next_free_number` skips any number an (even
  withdrawn) entry holds, as an exact string or numerically (`007` blocks `7`).
- **Renumber guard**: changing `car` is blocked once that **old** car number
  has timing data (scores/runs).  Message: "withdraw + re-add instead".  This
  mirrors the existing amend-only lifecycle and prevents orphaning scores/runs
  across synced devices.

## Suggestion algorithm

```
suggest_car_number(used: HashSet<&str>, preferred) -> String
  if preferred valid AND free (exact string) → preferred
  else → smallest positive int not in used
```

Used pool is caller-controlled (committed + newly-suggested cars at
close-entries).  Bounded: up to `MAX_SUGGESTED_NUMBER` (65535), with a
fallback that always returns a unique number.

## Shared cars

- `shared_car` is plain free text; grouping key = trim, collapse whitespace,
  lowercase (`shared_car_key`).
- Display only, on: home dashboard "Shared cars" block; start/finish car chips
  (warning badge) and pending-start rows; entry list tag.  Singleton groups
  (one member) are not shown.
- No effect on numbering or timing.  Can be set/changed any time.

## Wire format

`khanatime_entry:` message body carries a serialized `EntryMsg`
(`{ event_id, ts, entry, delete }`).  New `Entry` fields use `#[serde(default)]`
so old JSON still round-trips.  Merge key = `entry_no`.  Counter collisions
from concurrent offline creation are detected (different non-empty owners →
renumber the incoming entry) rather than clobbered.  Legacy tombstones with
`entry_no == 0` are skipped (target can't be identified; data is disposable).

`send_log_message` (services/matrix.rs) now sends both `SETUP_PREFIX` and
`ENTRY_PREFIX` bodies as plain chat; timing messages keep the structured key.

## Replay / sync

- `replay::apply` and `sync` merge sink handle `ENTRY_PREFIX`: adopt event id,
  upsert (entry_no) or tombstone (entry_no).
- `replay::replay` runs `ensure_entry_nos()` after reconstructing.
- `page::enqueue_entry` appends an entry message and applies it locally.

## Demo event

Includes a shared pair: Erin (`5`) and Gail (`12`) both type `Erin's MX-5` as
their shared-car name, so the shared-cars block and chip badges are visible in
training.  (Shared numbers need not be related.)
