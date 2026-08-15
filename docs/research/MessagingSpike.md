# Messaging Spike — Decision Record

> Status: accepted (Sprint 2.5, Aug 2026)
>
> Validated the Matrix transport for timing data with a two-client proof
> against a local Synapse homeserver before restructuring the build plan.

---

## Question

How do timing events (starts, finishes, penalties, results) travel between the
official, timekeeper, and competitor apps?

Two candidate transports were considered:

| Option | Description |
|---|---|
| **A. Direct room events** | Every app is a member of the event's `timing` room. Timing payloads are sent as Matrix events by the client that recorded them. Room history is the source of truth. |
| **B. Bot-mediated (Maubot)** | A bot (appservice) ingests events and re-broadcasts / persists them; clients only ever talk to the bot. |

## What we proved (WP1–WP3)

Runnable proof in `test/services/matrix_sync_test.dart` against a Synapse
container (`localhost:8008`, podman):

1. **Live exchange** — two logged-in clients; the sender posts a `start`
   TimingEvent into a shared `timing` room and the receiving client picks it up
   from its sync loop.
2. **Store-and-forward / reconnect** — the receiver goes offline (disposes its
   client + database), the sender posts a `finish` event, and a new client built
   on the *same* database file with the *same* client name restores the session
   via `Client.init()` (no fresh login) and retrieves the missed event.

The famedly `matrix` SDK provides all of this out of the box:

- session persistence + restore (`MatrixSdkDatabase` + `Client.init()`)
- continuous sync (`client.onSync`)
- backfill (`client.getRoomEvents(room.id, Direction.b)`)
- room management (`createGroupChat`, `Room.join()`, `room.sendEvent()`)

Wire format: `TimingEvent` (`lib/services/timing_event.dart`) serialised to a
JSON map under the `khanatime` content key, sent as an `m.room.message`.

## Decision

**Option A — direct room events. No bot.**

Rationale:

- The SDK already does store-and-forward, reconnect, and history backfill —
  the offline queue we wanted is free.
- No extra infrastructure (Maubot appservice) and no single point of failure
  on the write path.
- Room history is naturally the source of truth: any client can replay the
  full event log from the `timing` room and rebuild consistent local state.
- A bot adds server-side code, config, and a deployment step for zero benefit
  at a grassroots event (one room, handful of officials).

When to revisit the bot: if we later need server-side aggregation, cross-event
federation, or a central results authority that all clients must defer to.

## Conventions locked in

- One Matrix room per event, named `timing`, created by the timekeeper and
  joined by officials (`createGroupChat` with `publicChat` preset for now;
  invites may be needed once room access is tightened in the identity sprint).
- Every timing record is a single `m.room.message` whose content carries the
  `khanatime` payload — one Matrix event per start/finish/penalty/result.
- App DB (drift) becomes a cache; the `timing` room is the source of truth.
- Chat / voice stays out of the app — Element X handles that.

## Local test environment

- Homeserver: Synapse in podman, container `synapse`, port `8008`.
- Config: `/tmp/opencode/synapse/data/homeserver.yaml` — registration enabled
  (without verification), `registration_shared_secret: testsecret123`,
  relaxed login/registration rate limits.
- Test users: `kt1` / `kt2` (`passkt1123` / `passkt2123`), created via
  `register_new_matrix_user` with the shared secret.
- Proof tests skip themselves if `localhost:8008` is not reachable.

## Follow-ups (Sprints 3–5)

- Wire `MatrixService` into the timekeeper + official features (send on record,
  watch for live feed, backfill on startup).
- Reconcile drift cache against room history on reconnect.
- Identity: official UUIDs (`@official:<server>`) replace the placeholder
  `official_id`; Ed25519 signatures on payloads move to v2.
- Access control on the `timing` room once onboarding exists.
