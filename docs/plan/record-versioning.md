# Record / message versioning

> Status: deferred — implement just in time for the first official release.
> Pre-release, clients and room history are disposable (see AGENTS.md, no
> back-compat), so versioning buys nothing until real data must survive.
>
> Versioning for the two persistence layers: localStorage serde payloads
> (`event:`, `times:`, `runs:`) and the Matrix `TimingEvent` wire format
> (`khanatime` content key). Motivated by the UNKNOWN feature adding a `note`
> field, but designed so any future schema change is detectable and safe.

---

## Current state

- **localStorage** (`src/event.rs`): `EventInfo` under `event:<id>`,
  `Vec<ScoreData>` under `times:<id>`, `Vec<RunRecord>` under `runs:<id>`,
  plain `serde_json` strings. No version marker.
- **Matrix** (`src/timing_event.rs`): `TimingEvent` sent as the `khanatime`
  content key of an `m.room.message`. No version field.
- Forward compatibility today is accidental: `#[serde(default)]` on added
  fields makes old→new safe for additive changes, but nothing *declares* a
  version, nothing detects a **future** payload a client can't read, and enum
  changes (`KTime`, `EntryStatus`, `EventStatus`) can silently corrupt.

## Goals

1. Every payload carries a schema version so readers can:
   - accept `<= current` (decode, migrate if needed),
   - reject `> current` loudly (never guess).
2. Wire changes are cheap: additive fields (like `note`) bump the minor/handled
   list, breaking changes bump the major and are handled explicitly.
3. No silent corruption in local storage or across the room.

---

## Design

### `TimingEvent` (`src/timing_event.rs`)

```rust
pub const TIMING_EVENT_VERSION: u8 = 1;

pub struct TimingEvent {
    pub r#type: String,
    pub event_id: String,
    pub test: u8,
    pub car: String,
    pub run: u8,
    pub ts: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_ds: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub official_id: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    #[serde(default = "current_version")]
    pub version: u8,
}
```

Rules:

- `version` is **always** serialised (no `skip_serializing_if`) so every message
  declares its schema.
- `from_matrix_content` decodes `version`; if absent → treat as `1` (the first
  released format). If `version > TIMING_EVENT_VERSION` → return a typed error
  (currently `Option`), never a partial decode.
- Additive changes (new optional fields with serde defaults) are **minor**:
  bump to `2`, old clients still read (unknown fields ignored). Required by the
  UNKNOWN feature: adding `note` moves the wire format to **version 2**.

### localStorage (`src/event.rs`)

Keep the existing per-key JSON but wrap the mutable collections in a versioned
envelope:

```rust
const STORAGE_SCHEMA_VERSION: u8 = 1;

#[derive(Serialize, Deserialize)]
struct Envelope<T> {
    version: u8,
    #[serde(default)]
    data: T,
}
```

- `save_runs` / `save_times` write `Envelope { version: 2, data }`.
- `load_runs` / `load_times` decode as a **raw `Vec` first** (bare array = v1,
  backward compatible with what's already in browsers), else as `Envelope`.
  `version > STORAGE_SCHEMA_VERSION` → fail open to empty + surface a warning
  (local storage is a cache; the Matrix room is the source of truth).
- `EventInfo` keeps `#[serde(default)]` on all new fields; a full envelope for
  the event can follow when a breaking event-schema change appears.
- One `migrate_*` hook per type, called at load, alongside the existing
  `migrate_times_if_needed`.

### Why an envelope for runs/times but raw-fallback for load

Users already have un-versioned data in browsers. Load must accept both. Writes
always emit the new envelope, so the old shape disappears over time.

## Interaction with merge / replay

- `merge_setup` (last-writer-wins on the whole `EventInfo`) is unaffected by
  `TimingEvent` versioning — setup travels as its own message.
- Room replay (`sync.rs` sink) must skip `TimingEvent`s it cannot decode
  (`version > current`) instead of crashing the sync loop; log/feed them as
  "unrecognised message".
- `same_run` dedupe ignores `note` and `version` — the observation identity is
  `(type, test, car, run, ts)`.

## Results versioning (audit)

Already stated in `docs/research/Matrix.md`: each results publication is a new
room message; older versions stay in history for audit. No change needed here —
recorded for completeness.

---

## Checklist

- [ ] `TIMING_EVENT_VERSION` + `version` field on `TimingEvent`; bump to 2 for
      `note`
- [ ] `from_matrix_content` rejects `version > current`
- [ ] Sink tolerates undecodable messages
- [ ] `Envelope<T>` + raw-fallback load for `runs` / `times`
- [ ] `STORAGE_SCHEMA_VERSION` and migrate hooks
- [ ] Tests: v1 message decode, v99 reject, envelope round-trip, raw-array
      migration, `same_run` ignores `note`
