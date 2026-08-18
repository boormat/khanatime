# Identity + amendments — wire format v2 (Phase 1)

> **Stale references (2026-08-18):** This document references `EntryMsg` and
> `ENTRY_PREFIX` which have been removed from event.rs. Entry messages are no
> longer part of the wire format.

## Summary

Give every event and every timing observation a **stable generated id**, and
let corrections target observations by id. The original observation is the
**indelible record**: `amend`/`void` are new messages referencing a target
`uid`; the log never loses a fact, derived state (runs/scores/results)
reflects the latest intent. This is the foundation for QR parcel handoff and
the dual-homeserver relay (see `multi-transport.md`), and it fixes today's
duplicate-run-on-correction bug: `RunRecord`s currently dedup by the
`(type, test, car, run, ts)` tuple (`same_run`), so correcting a time creates
a *second* run instead of fixing the first.

**Locked decisions**

- ID: 10-char unambiguous base32, charset `0123456789ABCDEFGHJKMNPQRSTVWXYZ`
  (~50 bits; no `I/L/O/U`, so no 0-vs-O / 1-vs-L ambiguity). Git-partial-sha
  flavour without the collision risk at this scale.
- Wire `event_id` becomes the **event uid**; the slug stays the
  storage/session/alias key (log keys `log:<slug>`, room aliases, session
  storage, display).
- `amend`/`void` carry their own `uid` plus a `target` (the corrected
  observation's uid).
- Any logged-in timekeeper may amend/void; `official_id` is recorded on the
  amend/void message (last-writer-wins, matching today's model).

## Wire format v2

```rust
// timing_event.rs
pub struct TimingEvent {
    pub r#type: String,   // start | finish | amend | void
    pub event_id: String, // event uid (was: slug)
    pub uid: String,      // this observation's id — the indelible thing
    pub target: Option<String>, // amend/void -> corrected observation's uid
    pub test: u8,
    pub car: String,
    pub run: u8,
    pub ts: i64,
    pub time_ds: Option<u16>,
    pub status: Option<String>,
    pub flags: Option<u8>,
    pub official_id: Option<String>,
}

// event.rs
EventInfo { …, pub uid: String }        // generated at draft creation
RunRecord  { …, pub uid: String, #[serde(skip)] pub voided: bool } // voided is derived, never wire
```

## Task list

> Status: backend done (no fallbacks for existing data — clear localStorage +
> room history once; wire v1 bodies fail the strict v2 parse and drop).

- [x] **`src/ids.rs`** (new, pure/native-testable)
  - `gen_short_id() -> String`: 10 × `Math.random()*32` (wasm) /
    `SystemTime` nanos (native) → charset lookup.
    Tests: charset/length, 10k batch → no collisions, stable across calls.
  - `content_id(body) -> String`: if `KT `-prefixed, parse and return the
    embedded `uid`; else FNV-1a hash of the body → 16-char hex.
    Tests: timing uid round-trips; setup/entry/chat stable; two identical
    bodies → same id.
- [x] **`event.rs`**: add `EventInfo.uid` (**required**, no serde default —
    no legacy fallback); helper `ensure_uid(&mut EventInfo)` (fill with
    `gen_short_id()` when empty) called at draft creation and demo seed
    (publish calls it too before writing the space meta).
- [x] **`timing_event.rs`**: add required `uid` + optional `target` fields;
  `new()`/`finish()` auto-generate `uid`; new `amend(target, …)` /
  `void(target, …)` constructors.
- [x] **`event.rs` / wire plumbing**: `EntryMsg.event_id` now carries the uid
  (caller passes `e.uid` in `page::enqueue_entry`). Publish meta gains `"uid"`;
  `open_published_event` reads it into `ev.uid`. `merge_setup` stays plain
  last-writer-wins replace — every setup carries a uid, so no uid-graft branch.
- [x] **`replay.rs`**:
  - `add_run` dedups by `uid` only (no tuple fallback — fixes the
    duplicate-run-on-correction bug).
  - `apply()` new arms: `amend` → find run by `target`, patch
    `time_ds/status/flags/car/test/run/official_id`; `void` → set `voided`
    (derived, `#[serde(skip)]`, never on the wire).
  - `corrections: HashMap<uid, Vec<TimingEvent>>` — stash amend/void when the
    target is absent (amend-before-target via QR ordering); retried when a
    matching run lands.
  - Adoption: `if ev.uid.is_empty() { ev.uid = msg.event_id }`; skip messages
    whose uid ≠ adopted uid. `pending_starts` filters `voided`.
  - Tests: amend patches time/flags; void excludes from scores +
    `pending_starts`; amend-before-target applies on arrival; cross-transport
    dup (same uid, two log entries) → one run.
- [x] **`sync.rs`**: `handle_incoming` entry guard + adoption moved to uid;
  log key stays the slug; `merge_setup` unchanged (LWW replace).
- [x] **`page.rs`**: `enqueue_run`/`enqueue_ktime` stamp `uid` (from the
  constructed `TimingEvent`); new `enqueue_amend(model, target_uid, …)` /
  `enqueue_void(model, target_uid)` — build message, `enqueue_pending`, apply
  locally, `flush_pending`, `refresh_feed`. Wire `event_id` carries `e.uid`;
  localStorage keys stay the slug `e.id`.
- [x] **`log.rs`**: `append_log` dedups by `mid` **or** `content_id(body)`;
  `stale_pending` matches pending-vs-log by content_id as well as body.
  Tests: same observation uid in two serializations → single pending match;
  `same_uid_via_two_log_entries_collapses` in replay.
- [ ] **UI**: Start/Finish/Stage screens — "correct" (amend) on recent/pending
  observations, "void" affordance on `pending_starts` and entered times;
  correct vs "new run" distinction (amend patches in place; new run bumps
  `next_run`). Minimal Phase-1 scope: amend/void on the *most recent*
  observation per car+stage.
- [x] **Tests + gate**: all above unit tests; `./scripts/check.sh` green
  (fmt + clippy `--all-targets` + tests) + `cargo build
  --target wasm32-unknown-unknown`.
- [x] **Docs**: mark Phase 1 done in `multi-transport.md`; update `AGENTS.md`
  (tree gains `ids.rs`, note wire v2) and `PLAN.md`.

## Gotchas

- **Adoption order**: a fresh device must see the setup manifest before
  timing/entry messages stick; the log key stays the slug throughout.
- **No fallbacks**: uid is required on `EventInfo`/`TimingEvent`, `voided` is
  `#[serde(skip)]` (derived), dedup is uid-only — legacy v1 data fails parse
  and is dropped. Clear localStorage + room history once after this lands.
- **`merge_setup` is plain LWW replace** — it carries a uid in every setup, so
  no uid-graft branch was added (and none is needed).
- **`enqueue_ktime` run numbering** unchanged (amend is not a new run); void
  does not reset `next_run`.
- **File drift**: re-read current files before editing (another instance may
  be mid-fix); start with the isolated `src/ids.rs`.