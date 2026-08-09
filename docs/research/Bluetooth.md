# Bluetooth Multihop Integration Design

> Every device is a relay. No special roles, no hoping someone has signal.

This document describes how timing data flows across disconnected checkpoints
using Bluetooth, and how every phone at the event automatically forwards data
to Matrix when it reaches connectivity.

---

## The Problem

In rally and rallysprint events, checkpoints are spread over 3–50 km of forest
road with no phone coverage. Timing data recorded at remote checkpoints must
reach HQ. The only physical connection between checkpoints is the competitors
themselves — they drive past each checkpoint, and their phones pass within
Bluetooth range (~5–20 m) of the official's phone.

---

## Long-Term Architecture: Every Phone Is a Relay

The goal is not to designate relay officials. Every device that handles timing
data — officials, competitors, anyone — automatically syncs data over BLE and
automatically forwards it to Matrix when connectivity is available.

```
                    No connectivity                Connectivity
                    (forest checkpoints)           (HQ / road / village)
                    ─────────────────              ─────────────────────

  ┌──────────┐     BLE sync        ┌──────────┐
  │ Checkpoint│ ◄────────────────► │  Roving  │
  │ Official  │                    │ Official │
  │ (PWA)     │                    │ (PWA)    │
  └──────────┘                     └────┬─────┘
                                        │
                                   BLE sync
                                        │
  ┌──────────┐     BLE sync        ┌────▼─────┐     WiFi/Cell      ┌──────────┐
  │ Checkpoint│ ◄────────────────► │Competition│ ──────────────►  │ HQ       │
  │ Official  │                    │  (PWA)    │                   │ Homeserver│
  │ (PWA)     │                    └──────────┘                   │ + Element │
  └──────────┘                                                     └──────────┘

  All data eventually reaches HQ. No single device needs direct connectivity.
```

**The principle:** Data enters the system at any checkpoint. It propagates
via BLE through every phone it encounters. The moment any of those phones
gets WiFi or cellular data, it posts the data to the Matrix homeserver.
The timekeeper sees everything in Element.

There is no relay role to assign. The physics of the event do the relaying —
competitors drive between checkpoints, officials move between locations, and
every phone in their pocket carries data forward.

---

## How the App Handles This

The timing app (PWA inside a native wrapper, or native app) has two transport
layers that work simultaneously and transparently:

```
┌──────────────────────────────────────────────────────┐
│                   Timing App                         │
│                                                      │
│  ┌──────────────┐                                    │
│  │  Timing UI   │  Official enters data              │
│  └──────┬───────┘                                    │
│         │                                            │
│         ▼                                            │
│  ┌──────────────┐                                    │
│  │  Local Store │  SQLite / IndexedDB                │
│  │  (pending +  │  Everything saved locally first    │
│  │   synced)    │                                    │
│  └──┬───────┬───┘                                    │
│     │       │                                        │
│     ▼       ▼                                        │
│  ┌──────┐ ┌──────────┐                               │
│  │  BLE │ │  Matrix   │                               │
│  │ Sync │ │  Client   │                               │
│  │      │ │           │                               │
│  │ Advertise│ │ Sends to homeserver  │                │
│  │ pending  │ │ when WiFi/cell       │                │
│  │ data to  │ │ available            │                │
│  │ nearby   │ │                      │                │
│  │ devices  │ │ Receives forwarded   │                │
│  │          │ │ data from peers      │                │
│  └──────────┘ └──────────┘                               │
└──────────────────────────────────────────────────────┘
```

**Data flow for a single timing submission:**

1. Official enters car 23's time at checkpoint 3
2. App saves to local store with status `pending`
3. App advertises the pending entry over BLE
4. If a nearby phone is in BLE range, it receives the entry and stores it
   locally (also marked `pending` from its perspective — it hasn't reached
   Matrix yet)
5. The receiving phone advertises it further over BLE to any other nearby phone
6. Eventually some phone in the chain gets WiFi or cell connectivity
7. That phone's Matrix client sends the entry to the homeserver
8. Homeserver stores it, Element picks it up, timekeeper sees it
9. That phone marks the entry as `synced` in its local store
10. On next BLE sync, it advertises that the entry is `synced` so other phones
    can mark it too (avoids re-forwarding)

**No phone needs to know who the "relay" is.** Every phone is always both
a sender and a forwarder.

---

## Transport Layer Options

The BLE sync module is the critical piece. Three options for implementing it,
in order of increasing maturity risk:

### Option 1 — Wesh Protocol (Berty's Underlying Transport)

Berty is built on the Wesh Protocol — a libp2p-based store-and-forward
protocol that uses BLE and WiFi Direct for discovery and transport, with
CRDTs for conflict-free merging.

```
Timing App
    │
    ▼
┌─────────────────────┐
│  Wesh Protocol SDK  │  Apache-2.0 licensed
│  (embedded library) │  Runs as a background service
│                     │  Handles BLE advertising, discovery,
│  - BLE + WiFi       │  key exchange, CRDT merge
│  - Store-and-forward│
│  - E2EE             │
└─────────────────────┘
```

**How it would be used:**
- The Wesh library is compiled as a native module (C/Go → iOS framework /
  Android .so) and embedded in the app
- The app creates a Wesh group for the event (like a Berty group, but
  without the Berty app)
- Timing data is written to the Wesh group as messages
- The library handles BLE advertising, discovery, key exchange, and sync
  automatically in the background
- When connectivity is available, the app reads new entries from the Wesh
  group and posts them to Matrix via the client API

**Strengths:**
- Purpose-built for exactly this: offline P2P store-and-forward
- CRDT-based — concurrent edits merge cleanly, no conflicts
- E2EE built in — BLE exchange is encrypted
- Multihop by design — each device forwards to every other device it meets
- Apache-2.0 license — can be embedded without open-sourcing the app
- Already proven in Berty (cross-platform, iOS + Android)

**Weaknesses:**
- Wesh SDK is not published as a standalone library — must be extracted from
  Berty's codebase or built separately
- Berty team is small (~10 people), Wesh is not independently maintained
- iOS background BLE limitations still apply — the library can't control
  what the OS does to background processes
- Needs investigation: can Wesh run without the Berty app's UI layer?

**Status:** Needs investigation. Can Wesh be used as a standalone library?
What is the API surface? Is there a React Native or Flutter binding?

### Option 2 — @offline-protocol/mesh-sdk

A React Native SDK with a Rust core that provides BLE + WiFi + Reticulum
transport. Specifically designed for cross-platform offline mesh apps.

```
Timing App (React Native / Expo)
    │
    ▼
┌─────────────────────┐
│  @offline-protocol   │  MIT licensed
│  /mesh-sdk          │  Rust core, React Native bindings
│                     │
│  - BLE + WiFi       │
│  - Store-and-forward│
│  - Multihop mesh    │
│  - Reticulum routing│
└─────────────────────┘
```

**Strengths:**
- Purpose-built for this use case (React Native + offline mesh)
- Rust core — fast, low battery, cross-platform
- MIT license — permissive
- Multihop with Reticulum routing — data finds its own path
- Active development (v0.10 as of July 2026)

**Weaknesses:**
- Very early — v0.10, not production-tested at scale
- Reticulum adds a dependency on the Reticulum network stack
- No known motorsport or timing app deployments
- iOS background BLE support unverified

**Status:** Needs a proof-of-concept build. Can it sync 100-byte messages
between two phones reliably?

### Option 3 — Custom BLE Sync (Build from Scratch)

Build a minimal BLE GATT service that exchanges timing entries directly.
No protocol library — just raw BLE characteristic read/write.

```
Timing App
    │
    ▼
┌─────────────────────┐
│  Custom BLE Module  │  ~500 lines of platform code
│                     │
│  - GATT Server      │  Advertises a custom service UUID
│  - GATT Client      │  Scans for the same UUID
│  - Characteristic   │  Read/Write for data exchange
│  - Local queue      │  Pending entries waiting to sync
└─────────────────────┘
```

**How it would work:**
- Each phone advertises a custom BLE GATT service (e.g., `0xKT26`)
- A single characteristic holds the latest pending entry (~60–80 bytes)
- When two phones are in range, the client reads the server's characteristic
- Acknowledged entries are removed from the pending queue
- Simple store-and-forward: each phone carries a queue of unacknowledged
  entries and offers them to every peer it discovers

**Strengths:**
- No external dependencies — just platform BLE APIs (Core Bluetooth, Android
  BLE)
- Minimal code — the protocol is trivial (offer entries, acknowledge, done)
- Full control over iOS background BLE (can optimise for throttled
  background scanning)
- No protocol library to extract or maintain

**Weaknesses:**
- Device-to-device only — no true multihop mesh (must physically encounter
  each peer)
- No CRDT merge — need to handle duplicates and ordering in the app layer
- No E2EE at the transport layer (must encrypt the payload before sending)
- Must build and maintain for both iOS and Android natively
- GATT characteristic size limits: 512 bytes max per write (enough for
  timing data, but tight)

**Best for:** If Wesh and mesh-sdk are both too immature, this is the
fallback. Simple, reliable, no dependencies. Accept the limitation that
multihop requires physical proximity between sequential devices.

---

## BLE Transport Comparison

| | Wesh Protocol | mesh-sdk | Custom BLE |
|---|---|---|---|
| **Maturity** | Moderate (in Berty) | Early (v0.10) | N/A (custom) |
| **Multihop** | Yes (built-in) | Yes (Reticulum) | No (device-to-device) |
| **CRDT merge** | Yes | Unknown | No (app layer) |
| **E2EE** | Yes (built-in) | Unknown | Must add |
| **iOS background** | Throttled (OS limit) | Unverified | Throttled (OS limit) |
| **Android background** | Full | Full | Full |
| **BLE throughput** | ~50 KB/s | Unknown | ~50 KB/s (limited by BLE) |
| **Extraction effort** | Medium (from Berty) | Low (npm package) | Low (build it) |
| **License** | Apache-2.0 | MIT | N/A |
| **Risk** | Berty team dependency | Early, untested | None (own code) |

---

## Matrix Integration: How BLE Data Reaches Element

Regardless of which BLE transport is chosen, the bridge to Matrix follows
the same pattern. Every device that receives BLE data forwards it to the
homeserver when connectivity is available:

```
BLE received entry → Local store (status: pending) → Matrix client API → Homeserver → Element
```

**The Matrix client does not need to be custom.** Options:

### Option A — Use Element X as the Matrix client

- The timing app saves data locally and syncs over BLE
- When connectivity is available, the timing app opens Element X via
  deep-link with the data pre-filled
- User taps send
- **Problem:** Same UX issue as Option A in the relay model — app switching

### Option B — Embedded Matrix SDK

- The timing app includes the `matrix-sdk` (Rust) or a JS/Dart wrapper
- Posts directly to `/_matrix/client/v3/rooms/{room_id}/send/m.time/`
- No Element needed on the device for data forwarding (Element still used
  for chat/voice separately)
- **Problem:** Must handle Matrix client responsibilities (auth, sync, E2EE)

### Option C — Lightweight HTTP forwarder

- The timing app runs a minimal HTTP client (not a full Matrix client)
- When connectivity is available, it POSTs each pending entry to the
  homeserver's `/_matrix/client/v3/sendMessage` endpoint using a pre-shared
  access token
- The homeserver stores it in the room, Element picks it up
- No E2EE for timing data (use plaintext rooms for timing, encrypted rooms
  for chat)
- **Simplest option** — just HTTP POSTs with JSON payloads

**Recommendation:** Start with Option C. It's the smallest amount of work
to get BLE data into Matrix. Upgrade to Option B if E2EE for timing data
becomes a requirement.

---

## Phased Delivery

### Phase 5a — Validate BLE Sync

Prove that timing data can travel between phones over BLE at a motorsport
event. No Matrix integration yet.

1. Build or obtain a BLE sync module (evaluate Wesh extraction, mesh-sdk,
   or custom BLE)
2. Create a minimal test app: enter a timing entry, sync over BLE to a
   second phone
3. Field test at a khanacross or rallysprint event
4. Measure: sync latency, range, reliability, battery drain over a day

**Exit criteria:** Two phones exchange timing entries over BLE at 10+ m
range, < 30 s latency, no data loss over 50 entries. If this fails at the
BLE level, multihop is not viable and the project stays on Matrix + WiFi.

### Phase 5b — Multi-Device Relay

Extend from two phones to the full event topology.

1. 5+ phones running the test app
2. Some phones have connectivity, some don't
3. Entries propagate through the chain via BLE
4. Verify that entries eventually reach the connected phone
5. Measure: total propagation time from remote checkpoint to "Matrix"

**Exit criteria:** An entry from a disconnected checkpoint reaches a
connected phone within 5 minutes of the competitor driving past the
checkpoint.

### Phase 5c — Matrix Bridge

Connect the BLE sync to the Matrix homeserver.

1. When a phone gets connectivity, POST pending entries to the homeserver
2. Entries appear in the `#timing` room in Element
3. Timekeeper sees BLE-relayed data alongside WiFi-submitted data
4. Verify: no duplicates, correct ordering, audit trail intact

**Exit criteria:** Timekeeper cannot distinguish WiFi-submitted entries from
BLE-relayed entries in Element. Both appear in the same room with the same
format.

### Phase 5d — Production Hardening

1. Handle edge cases: duplicate entries, out-of-order delivery, partial
   syncs, phone restarts mid-event
2. Battery optimisation: duty-cycle BLE scanning, batch syncs
3. UI: show sync status in the timing app (pending / synced / confirmed)
4. Monitoring: log sync events for post-event audit

---

## Open Questions

- [ ] Can the Wesh Protocol be extracted from Berty as a standalone library?
  What is the API surface? (Investigation needed)
- [ ] Does `@offline-protocol/mesh-sdk` work on iOS? (v0.10 claims iOS
  support but needs verification)
- [ ] What is the actual BLE range in a forest environment with trees and
  terrain? (Field test at a rally venue needed)
- [ ] How does iOS Background App Refresh interact with BLE scanning over
  6+ hours? Does the OS kill the process?
- [ ] What is the minimum viable payload for a timing entry compressed for
  BLE? (Current estimate: ~60–80 bytes)
- [ ] Can a minimal Matrix HTTP forwarder (Option C above) handle 50
  entries per minute without a full SDK?
- [ ] Should timing data rooms be encrypted or plaintext? (Trade-off:
  security vs. simplicity of the HTTP forwarder)
