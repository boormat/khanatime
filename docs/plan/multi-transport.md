# Multi-transport timing, observation ids, and amendments

## Summary

Timing data moves over **three transports**:

1. **Per-event Matrix timing room** on the local LAN homeserver (the current
   single-room design).
2. **A public Matrix homeserver** (e.g. matrix.io) for events with mixed
   coverage — HQ on wifi, officials out on course on mobile data.
3. **QR handoff**: transport-agnostic *parcels* (any subset of the event's
   message log) carried phone-to-phone via sequenced animated QR — the
   no-network / wifi-failed fallback.

Backing all three is a redesign of **identity**: generated short ids for events
and for individual timing observations, so every message can be resolved and
merged regardless of which transport delivered it. Amendments (`amend`/`void`)
reference an observation by id; the **original observation is the indelible
record** — the log never loses a fact, derived state (scores/results) reflects
the latest intent.

**Related docs:** `docs/plan/identity-amendments.md` (Phase 1 — ids + `amend`/`void`
wire v2, implementation detail for Pillar 1); `docs/plan/qr-join.md`
(scan-to-join bootstrap — the join link is how a device gets pointed at a room
transport); `PLAN.md` (Comms section — this plan extends/supersedes the
single-room direction); `docs/research/` (original Matrix/BLE/mesh research).

## Design principles

- **No back-compat.** Pre-release: every client's localStorage and all room
  history are disposable. The wire format changes freely; no versioning or
  migration of old data.
- **The timing observation is the indelible thing.** `void`/`amend` are *new*
  messages that reference the original `uid`; the original stays in the log.
- **Content-addressed dedup** is the shared spine across all three transports:
  every message resolves to one transport-independent id, so merging is
  idempotent and relaying is loop-safe.

---

## Pillar 1 — Identity + amendments

> **Status: implemented (backend).** `src/ids.rs`, `EventInfo.uid`,
> `TimingEvent` v2 (`amend`/`void` + `target`), uid-keyed dedup in
> `add_run`/`log::append_log`, and replay adoption/corrections are in.
> The Correct/Void **UI** (Start/Finish/Stage screens) is not wired yet —
> `page::enqueue_amend`/`enqueue_void` exist as plumbing. Clear
> localStorage + room history once, since old wire v1 messages fail the
> strict v2 parse and are dropped.

Implementation detail (file-by-file, wire spec, task list, gotchas):
`docs/plan/identity-amendments.md`.

### `src/ids.rs` (new)

Pure Rust, native-testable (no wasm gating on the core).

```rust
/// Git-flavoured short id: 10-char Crocker base32 (0-9 A-V, no 0/1/O/I/L),
/// ~50 bits. Collision odds ~1e-9 at 10k ids — fine for this scope.
pub fn gen_short_id() -> String { ... }   // wasm: js_sys::Math::random(); native: SystemTime

/// Transport-independent dedup key for a message body:
/// timing messages -> embedded observation `uid`; otherwise hash(body).
pub fn content_id(body: &str) -> String { ... }
```

Unit tests: charset/length, no collisions in a batch, `content_id` stable and
transport-independent.

### `EventInfo.uid`

- Generated once at draft creation; never renames.
- The **wire identity**: `TimingEvent.event_id` and `EntryMsg.event_id` carry
  the `uid`, not the human slug.
- The slug `id` stays as the human/alias key (room aliases `#kt-…:server`,
  display). The setup manifest carries **both**, so any device resolves
  `uid → event` regardless of which room or QR the message arrived through.
- `merge_setup` and replay adoption key on `uid`. Fresh-device adoption:
  setup message arrives → event adopted by uid → subsequent timing/entry
  messages for that uid merge in.

### `TimingEvent` v2 (`timing_event.rs`)

```rust
pub struct TimingEvent {
    pub r#type: String,   // start | finish | amend | void
    pub event_id: String, // event uid (not the slug)
    pub uid: String,      // observation id — the indelible thing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>, // amend/void -> uid being corrected
    pub test: u8,
    pub car: String,
    pub run: u8,
    pub ts: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_ds: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flags: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub official_id: Option<String>,
}
```

- `start`/`finish`: fresh `uid` assigned at enqueue time in
  `page.rs::enqueue_run` / `enqueue_ktime`. **This kills the
  duplicate-run-on-correction bug**: `RunRecord`s currently dedup by the
  `(type, test, car, run, ts)` tuple (`same_run`), so correcting a time today
  creates a *second* run; with uids, dedup is by `uid`.
- `amend`: `target` = the original's `uid`; carries corrected
  `time_ds`/`status`/`flags`/`car`/`test`/`run`/`official_id`. Replay patches
  the targeted `RunRecord` (car-number corrections land here too — a mis-typed
  car on an observation is amended, not re-entered).
- `void`: `target` = the original's `uid`; the record **stays in the log**
  (indelible) but derived state marks it voided → excluded from start/finish
  pairing, `pending_starts`, and scores. Void is final; if wrong, enter a
  fresh observation.

### Replay (`replay.rs`) changes

- `RunRecord` gains `uid`; `add_run` dedups by `uid`.
- New derived state: a `corrections: HashMap<uid, AmendEvent>` (or similar)
  so an `amend` that arrives **before** its target (QR parcel ordering) is
  applied when the target lands.
- Replay stays pure and idempotent: every state change flows through
  `apply()` over the log + pending, and results recompute from amended/voided
  records each time.

### Authority & UI

- **Any logged-in timekeeper** may amend/void any observation; `official_id`
  is recorded on the amend/void message (last-writer-wins, matching today's
  model).
- Start/Finish/Stage screens: "correct" (amend the last/selected observation)
  vs "new run" (re-enter a fresh attempt); void affordance on `pending_starts`
  and entered times. New `page.rs` helpers `enqueue_amend(model, uid, …)` and
  `enqueue_void(model, uid)`.

---

## Pillar 2 — QR handoff (generic message batch)

### `src/services/qr.rs` (new)

**Parcel wire format** — plain text body so it can also be typed/copied:

```
khanatime_parcel:<json>
```

```json
{ "v": 1, "event_uid": "…", "created": 1234567890,
  "msgs": [ { "body": "KT {…}", "ts": 123, "sender": "@o:server" }, … ] }
```

`msgs[].body` are the **same** strings as room messages (`KT {json}` /
`khanatime_setup:` / `khanatime_entry:`), so **import = append_log + replay +
refresh** — the identical downstream path as a room message. No special QR
state machine; content-id dedup makes re-scanning/overlap idempotent.

**Export**: "everything" or "since last handoff marker" (a stored per-event
cursor) → compress with `flate2`/`miniz_oxide` (pure-Rust, wasm-fine) → chunk
into ~1.2KB frames → sequenced animated QR: each frame carries
`index/total/frame-hash` + CRC, decoded one at a time by the receiving phone's
plain camera app. A full day's log (~10–50KB raw → ~5–15KB compressed) is a
handful of frames.

*Implemented (text mode):* `services/qr.rs` packs the full log (export-all, no
cursor yet) as plain JSON with no compression; the Handoff UI on Results/Events
shows the `khanatime_parcel:` string to copy/scan. Export promotes the outbox
(`log::publish_outbox`); imports land via `append_log` (content-id idempotent);
`sync::relay_to_room` re-broadcasts parcel-origin entries to the connected room
and `log::confirm_in_room` ack-promotes them (echo-safe). Animated QR + `flate2`
+ camera scanning are the remaining Pillar-2 work.

**Directions:**
- official → head timekeeper (results at stage end);
- head TK → officials (amendments/corrections);
- device → fresh device (full event bootstrap, zero network).

**UI**: "Handoff" buttons on the Results and Events screens (export + import);
an import lands in the *current* event only if `parcel.event_uid` matches,
else prompts to switch event first.

---

## Pillar 3 — Dual homeserver + auto-relay

### Content-id dedup (`log.rs`)

- `LogMsg` gains a `content_id`; `append_log` dedups on it across **all**
  transports (today it dedups only by Matrix `mid` + body-based reconcile).
- The same observation arriving from the LAN room, the matrix.io room, and a
  QR collapses to one log entry.

### Multi-connection (`app.rs` / `sync.rs`)

- Keep the existing **primary** connection (identity/login UX) plus
  `Vec<AuxConn { hs, status, rooms }>` for additional servers.
- Home page: "add server" (e.g. local Synapse + `matrix.io`).
- `flush_pending` fans out to every connected transport for the current event;
  incoming messages merge from all of them.

### Room registry (`event.rs`)

```rust
pub struct EventRoom {
    pub homeserver: String,
    pub space_id: Option<String>,
    pub space_alias: Option<String>,
    pub timing_id: Option<String>,
    pub timing_alias: Option<String>,
}
// EventInfo gains:  pub rooms: Vec<EventRoom>
```

Aliases embed the server name (`:localhost` vs `:matrix.org`), so rooms are
per-server. Publishing creates mirror rooms on every configured HS; a device
joins the room set matching the server it can reach.

### Auto-relay

- A dual-connected device (typically the HQ head-TK) merges both rooms'
  histories by content-id and flushes pending to **both** — a field phone on
  mobile data reaches LAN-only officials and vice-versa. No extra machines.
- **Echo loops are harmless**: a message relayed LAN→public that comes back
  LAN-ward is dropped by content-id dedup.
- `services/matrix.rs`: `IncomingMessage` gains `content_id`; publish-to-
  multiple; room lookup via the registry.

---

## Task list

- [x] `src/ids.rs`: `gen_short_id` + `content_id` + unit tests.
- [x] Wire v2: `TimingEvent.uid`/`target` + `amend`/`void`; `EventInfo.uid`;
      `EntryMsg.event_id` on uid; `RunRecord.uid`.
- [x] `replay.rs`: dedup by uid; apply amend/void; `corrections` map for
      amend-before-target; adoption by uid.
- [x] `log.rs`: `content_id` dedup across transports.
- [x] `page.rs` + Start/Finish/Stage: uid at enqueue; `enqueue_amend` /
      `enqueue_void`; correct-vs-new-run UI; void affordances.
- [x] QR: parcel export/import (`services/qr.rs`, `sync::export_parcel` /
      `import_parcel`, Handoff UI on Results/Events, promote-on-export +
      `sync::relay_to_room`, `log::publish_outbox`/`confirm_in_room`).
- [ ] QR rendering (animated/chunked sequence) + camera scanning; `flate2`
      compression (export is plain JSON for now).
- [ ] Multi-connection: app state (`AuxConn`), Home "add server", fan-out
      flush, merge from all transports.
- [ ] Room registry + publish-to-multiple + per-server room join.
- [ ] Relay merge + echo-loop safety.
- [ ] Tests throughout; `./scripts/check.sh` green (fmt + clippy
      `--all-targets` + tests — CI gate).
- [ ] Docs: fold into `PLAN.md` (Comms), `docs/plan/qr-join.md`,
      `AGENTS.md` (src tree + conventions).
- [ ] Manual test (below).

## Gotchas

- **Mixed content**: an HTTPS-loaded app (GitHub Pages) cannot reach a
  plain-`http` LAN Synapse. LAN transports need the app served over LAN http
  (`trunk serve --address 0.0.0.0`); Pages pairs only with TLS/public servers.
- **server_name vs URL**: aliases embed the HS `server_name` (`:localhost`)
  but resolve through any URL pointing at that HS — `#kt-…:localhost` works
  from a phone using `http://192.168.x.x:8008`.
- **slug vs uid**: the slug is the alias/display key; the uid is the wire
  identity. Keep the mapping in the setup manifest; never derive one from the
  other for message routing.
- **amend-before-target**: handle via the `corrections` map; never assume log
  order implies observation order across transports.
- **Echo loops**: only safe because every transport dedups by content-id —
  keep dedup at append, not just at replay.

## Manual test (LAN)

1. `podman start synapse`; serve app on LAN; publish an event from the
   head-TK device (dual-connected: local Synapse + matrix.io test room).
2. Phone A on LAN wifi joins via QR join link → entries/stages sync from the
   LAN room.
3. Phone B on mobile data joins the matrix.io room → sees the same event
   (uid adoption) and its history.
4. Phone B enters a time → appears on phone A through the relay (head-TK).
5. Kill wifi: head-TK exports "since last handoff" → animated QR → phone B
   scans → results merge; phone B amends a time → QR back → head-TK merges.
6. Refresh all devices → `resume_on_load`; no duplicates anywhere.

## Follow-ups (not in scope)

- In-app QR **scanning** (`getUserMedia` + `BarcodeDetector`; jsQR fallback
  for Firefox; HTTPS/localhost only) — the `events.rs` `Msg::ScanQr` stub.
- Event E2EE / device verification (see `docs/research/Cryptography.md`).
- Server-side bridging between homeservers (a Matrix appservice), if relay
  devices are ever offline.
- Multi-stopwatch averaging (already in `PLAN.md`), now that observations
  have stable uids to compare.