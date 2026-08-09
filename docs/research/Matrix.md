# Element / Matrix for Khanatime26

> Matrix is an open standard for decentralised, real-time communication. Element
> is the most mature client application for Matrix — available on iOS, Android,
> Desktop, and Web. Together they provide chat, voice, video, file sharing, and
> store-and-forward messaging that can run entirely on a local network with no
> internet connection.

This document describes how Matrix/Element can serve as the communication and
messaging backbone for Khanatime26 events, the phases of capability development,
the experience for each user role, and what event organisers must do to set it up.

---

## Why Matrix / Element

| Requirement | Matrix / Element |
|-------------|-----------------|
| Works offline on local WiFi | Proven air-gapped deployment (used militarily). Local homeserver on a laptop, no internet needed. |
| Small download | Element X is ~30 MB on iOS/Android. No custom build required. |
| Voice messaging and calls | Native voice messages in Element X. VoIP via Element Call / LiveKit. Replaces walkie-talkies. |
| Store-and-forward | Built into the Matrix protocol — server queues messages, clients sync when they reconnect. |
| Structured data via bots | Matrix appservice protocol allows bots to send and receive structured data (JSON). Maubot provides a plugin framework. |
| End-to-end encryption | Enabled by default. Device verification via SAS/QR. |
| Multiple client options | Element, Element X, FluffyChat, SchildiChat — users choose what they prefer. |
| Cross-platform | iOS, Android, macOS, Windows, Linux, Web. |
| Open source | Homeserver (Continuwuity) and all clients are AGPL / GPL / Apache licensed. |
| Low resource usage | Continuwuity homeserver: ~150 MB RAM, single binary, embedded RocksDB. Runs on any laptop or mini-PC. |

---

## Event Network Architecture

```
[Portable WiFi AP]  ←  Local LAN (no internet)
        |
 [Laptop / Mini-PC]
   ├─ Continuwuity homeserver (~150 MB RAM)
   ├─ Element Web (served locally over HTTP)
   ├─ Optional: ntfy server (local push notifications)
   └─ Optional: Maubot (timing data bots)
        |
   ┌────┼────────────┬──────────────┐
   │    │            │              │
[Officials]   [Officials]   [Competitors]   [Race Control]
 Element X     Element X     Element X       Element Web
  (mobile)     (mobile)      (mobile)        (desktop)
```

The portable WiFi access point creates a local network. The laptop runs the
homeserver and optional services. All phones connect to this WiFi and
communicate through the homeserver. No internet uplink is required.

When phone coverage is available at the venue, the homeserver can optionally
be reached over the internet as well, allowing remote officials or spectators
to participate.

---

## Data Streams Over Matrix

| Stream | Direction | Volume | Priority | Implementation |
|--------|-----------|--------|----------|----------------|
| **Timing data** | Officials → Timekeeper → All | Low | High | Structured messages via bot or direct room messages |
| **Results** | Timekeeper → All | Low | High | Dedicated results room, pinned/finalised messages |
| **Location tracking** | Competitors → Safety Official | High (compacted) | Medium | Bot receives GPS updates, compacts to last-known per competitor |
| **Safety messages** | Anyone → Safety Official | Very low | Critical | Dedicated safety room with @room mentions |
| **Chat / Voice** | All ↔ All | Medium | Low | Native Element chat and voice messages |

Each stream maps to a Matrix room. Room permissions control who can post and
who can read. Encrypted rooms ensure only authorised participants see the data.

---

## Phases of Capability Development

### Phase 1 — Foundation (Event Comms Only)

**Goal:** Replace walkie-talkies and ad-hoc group chats with a reliable,
self-hosted messaging system at events.

**What gets built:**
- Continuwuity homeserver deployment script (one-command startup on the
  timekeeper's laptop)
- Pre-configured Element Web served locally from the same laptop
- WiFi access point setup guide for organisers
- Room structure created automatically on event setup:
  - `#general` — open chat for all participants
  - `#timing` — officials submit timing data to timekeeper
  - `#results` — timekeeper publishes finalised results
  - `#safety` — safety-critical messages with @room alerts
  - `#location` — competitor GPS pings (compacted by bot)
- Voice messaging coaching — officials record voice notes instead of using
  radios

**User experience at this phase:**
- Officials install Element X from the app store before or on arrival
- On-site QR code scan joins them to the event WiFi and auto-configures
  their Element client (homeserver address, room invites)
- Officials send timing data as structured text messages or voice notes
- Timekeeper reviews submissions in `#timing`, adjusts, and publishes
  to `#results`
- Safety official monitors `#safety` and `#location`
- Competitors join `#results` read-only to see published results

**What organisers must do:**
1. Bring a laptop (any modern laptop works) and a portable WiFi access point
2. Run the setup script before the event — it starts the homeserver, creates
   rooms, and generates a QR code for on-site joining
3. Place the WiFi access point in a central location (HQ area)
4. Optionally: test the setup the night before at the venue

---

### Phase 2 — Structured Timing Data

**Goal:** Replace freeform text messages with structured, machine-parseable
timing submissions that the timekeeper's tool can ingest directly.

**What gets added:**
- A Matrix bot (via Maubot or custom appservice) that listens in `#timing`
  and parses structured timing submissions:
  ```
  T 05 car23 01:12.34
  ```
  (Test 05, Car 23, Time 1 minute 12.34 seconds)
- Penalty submissions:
  ```
  P 05 car23 flag_hit
  ```
- The bot validates format, acknowledges receipt, and forwards parsed data to
  the timekeeper's admin panel
- Duplicate detection — if two officials submit the same test/car, the bot
  flags the discrepancy for the timekeeper
- The timekeeper's admin interface (PWA or sideloaded app) pulls structured
  data from the bot

**User experience at this phase:**
- Officials type or speak a short structured command (or use a simple PWA form
  that generates the message)
- The bot immediately responds with confirmation or error
- Timekeeper sees a live feed of parsed, validated timing data in their admin
  tool — not raw chat messages
- Competitors still see finalised results in `#results`

**What organisers must do:**
- Same as Phase 1, plus ensure the Maubot plugin is installed before the event
  (bundled with the setup script)

---

### Phase 3 — Automated Results Publishing

**Goal:** Timekeeper finalises results in their admin tool and they are
automatically published to Matrix and to a public results page.

**What gets added:**
- Timekeeper's admin tool pushes finalised results to the Matrix bot
- Bot publishes formatted results to `#results` (as HTML-formatted Matrix
  messages)
- Bot also updates a results PWA page served locally from the same laptop
  (competitors open `http://192.168.x.x/results` in their browser)
- Results are versioned — each publication is a new message, older versions
  remain in the room history for audit

**User experience at this phase:**
- Timekeeper works in their admin tool, reviews discrepancies, applies
  adjustments, hits "Publish"
- Results appear instantly in `#results` and on the web results page
- Competitors check results on the PWA page or in the Element room
- No manual copy-pasting between tools

**What organisers must do:**
- Same as Phase 2 — the results PWA is served from the same laptop

---

### Phase 4 — Location Tracking and Safety

**Goal:** Competitors' phones report GPS positions to a safety official in
real time. Safety messages are escalated immediately.

**What gets added:**
- Competitor PWA (or native app) captures GPS position periodically and sends
  it as a structured Matrix message to `#location`
- A bot in `#location` compacts updates — it maintains a map of last-known
  position per competitor, old data is pruned from the sync payload
- Safety official sees a live map (PWA) with competitor positions
- Safety messages sent to `#safety` trigger @room alerts and appear on the
  safety map with incident markers
- SOS button in the competitor app sends a high-priority, location-tagged
  safety message

**User experience at this phase:**
- Competitors opt in to location sharing (simple toggle in the app)
- Their phone quietly reports GPS in the background while the app is open
- Safety official monitors a map at HQ showing all competitor positions
- If a competitor crashes or needs help, they tap SOS — the safety official
  sees their exact location immediately
- Stage officials log cars passing through their checkpoint for safety
  tracking (locating a car that may have gone off-road)

**What organisers must do:**
- Same as Phase 3
- Brief competitors at sign-on that location sharing is available and
  explain how to enable it
- Ensure the safety official has a device with a larger screen (tablet or
  laptop) for the map view

---

### Phase 5 — Multihop Store-and-Forward

**Goal:** When there is no WiFi or phone coverage between checkpoints, data
hops through competitors' phones as they physically travel between locations.

**What gets added:**
- Bluetooth / WiFi Direct sync between devices using the Wesh Protocol
  (investigated separately) or a compatible library
- When a competitor arrives at a checkpoint, their phone automatically syncs
  all pending data with the official's phone via Bluetooth
- Data propagates: Start Official → Competitor car → Mid-stage Official →
  Finish Official → HQ
- Matrix remains the primary transport when connectivity exists; Bluetooth
  sync is the fallback for disconnected segments

**User experience at this phase:**
- No change for users — sync happens automatically when phones are near each
  other
- Officials at remote checkpoints see data arriving as competitors pass through
  (even with no phone coverage)
- Timekeeper at HQ receives data from remote officials via the competitor relay
  network

**What organisers must do:**
- Same as Phase 4
- Brief officials that data will sync automatically when competitors arrive —
  no action required from them
- Ensure Bluetooth is enabled on all official devices (default on most phones)

---

## User Experience Summary

### Officials (Stage / Checkpoint)

| Before Matrix | With Matrix (Phase 1+) |
|--------------|----------------------|
| Paper stopwatch, manual entry | Phone-based timing via Element message or PWA form |
| Walkie-talkie to relay times | Voice message or structured text in `#timing` |
| Drive to HQ to deliver results | Data syncs automatically over WiFi or via competitor relay |
| Multiple radios, batteries, frequencies | Single app on existing phone |

**Setup at event:** Scan QR code → join WiFi → Element opens in the right
rooms. Takes under 2 minutes.

### Timekeeper

| Before Matrix | With Matrix (Phase 2+) |
|--------------|----------------------|
| Collects paper slips or verbal reports | Sees live feed of parsed, validated timing data |
| Manually enters data into spreadsheet | Bot pre-fills structured data, timekeeper reviews and adjusts |
| Announces results verbally or via email | Publishes once, results appear everywhere automatically |
| No audit trail | Full Matrix room history serves as an immutable audit log |

**Setup at event:** Run the laptop setup script, open admin tool. Takes under
5 minutes.

### Competitors

| Before Matrix | With Matrix (Phase 1+) |
|--------------|----------------------|
| Walk to noticeboard or ask officials | Open results page in browser or Element room |
| No visibility during event | Live results as they are published |
| No safety tracking | GPS location shared with safety official (opt-in) |
| No way to call for help remotely | SOS button with automatic location |

**Setup at event:** Scan QR code → Element opens showing results room. Takes
under 1 minute.

### Safety Official

| Before Matrix | With Matrix (Phase 4+) |
|--------------|----------------------|
| Relies on radio calls to locate crashed cars | Live map of all competitor positions |
| Manually notes passing cars | Automatic car-pass logging at checkpoints |
| No centralised incident view | Safety messages with location markers on a map |

**Setup at event:** Open the safety map PWA on a laptop or tablet. Takes
under 2 minutes.

---

## What Organisers Need to Host the Service

### Equipment (all items are common and reusable)

| Item | Purpose | Cost Estimate |
|------|---------|---------------|
| Laptop (any modern laptop) | Runs homeserver, bot, results PWA | Already owned by most clubs |
| Portable WiFi access point | Creates the local event network | $30–80, one-time purchase |
| Ethernet cable | Connects laptop to access point | $5 |
| Power board + extension lead | Powers laptop and access point | Already owned |
| Optional: UPS / battery pack | Keeps laptop running during setup/teardown | $50–100 |

### Software Setup (done once, reused at every event)

1. **Install Continuwuity** — single binary, no dependencies. Download from
   the project releases page.
2. **Install Maubot** — optional, for structured data bots. Single binary with
   plugin support.
3. **Download Element Web** — static files served by the homeserver or a
   lightweight HTTP server.
4. **Run the Khanatime26 setup script** — configures the homeserver, creates
   event rooms, generates the on-boarding QR code.

### At Each Event (day-of checklist)

1. Set up WiFi access point in the venue (central location, near HQ)
2. Connect laptop to access point via Ethernet
3. Run the event setup script — this:
   - Starts the homeserver with event-specific configuration
   - Creates the room structure
   - Generates a QR code that configures Element on any phone
   - Starts the timing bot and results PWA
4. Print or display the QR code at sign-on
5. Officials and competitors scan the QR code to join
6. When the event is over, stop the server. Export the room history for archival.

### Network Considerations

- **No internet required.** The entire system runs on the local LAN.
- **WiFi range** is the main constraint. A single access point covers ~50–100 m
  indoors, ~100–300 m outdoors. For larger venues, use multiple access points
  with the same SSID (roaming) or mesh extenders.
- **Phone coverage as backup.** If the venue has mobile data, the homeserver
  can be reached over the internet, extending range beyond the WiFi footprint.
  This is optional — the system is designed to work without it.
- **Battery life.** Element X with periodic sync is not a heavy battery drain.
  Officials should bring a power bank for full-day events.

### Staffing

- **One technical person** to set up and monitor the laptop/server during the
  event. This is typically the timekeeper or a dedicated tech volunteer.
- No additional IT staff is needed. The system is designed to be managed by
  a single person with minimal technical background after initial setup.

---

## Risks and Mitigations

| Risk | Mitigation |
|------|-----------|
| WiFi access point fails | Backup: officials can use phone hotspot or direct Bluetooth sync |
| Laptop fails | Backup: homeserver can be moved to any other computer; state is in a single directory |
| Officials don't install Element before event | QR code at sign-on; Element X is ~30 MB download; setup takes < 2 minutes |
| Bluetooth sync (Phase 5) is unreliable | Matrix remains primary; Bluetooth is a fallback, not a hard dependency |
| Matrix protocol changes | Continuwuity and Element are actively maintained with long-term support |
| E2EE causes key verification overhead | For event use, unencrypted rooms are acceptable (local network, short-lived). E2EE is optional. |

---

## Relationship to Other Transport Options

Matrix/Element is the **primary communication layer** for Khanatime26. Other
transport options (Berty, BLE mesh, WiFi Direct) are investigated as
**complementary fallback layers** for truly disconnected scenarios:

- **Matrix over local WiFi** — primary, works today, well-tested
- **Bluetooth / WiFi Direct** — Phase 5 fallback for disconnected checkpoints
- **Cellular data** — automatic when available, extends range beyond WiFi
- **SMS** — last resort for critical safety messages, ~100 chars encoded
- **Manual file transfer** — emergency fallback (USB, AirDrop)

Matrix provides the reliable foundation. Mesh and P2P technologies fill the
gaps where WiFi cannot reach.

---

## References

- [Matrix specification](https://spec.matrix.org/)
- [Element](https://element.io/)
- [Continuwuity homeserver](https://gitlab.com/continuwuity/continuwuity)
- [Maubot](https://mau.bot/)
- [PLAN.md](../../PLAN.md) — full project planning document
- [Berty.md](Berty.md) — Berty P2P proof of concept
