# Architecture

> Extracted from: PLAN.md (original planning document)
> Status: Research phase

---

## Project Goals

Develop applications for Rally and Khanacross result collection, merging, and
publishing event results. Possibly two applications sharing a common deployment
and communications mechanism.

Key constraints:
- Low budget motorsport events with unpaid volunteer officials
- Officials are often non-technical, reluctant to install apps
- Small downloads, works on slow data connections
- Must work with intermittent or no phone coverage
- Phone-only (no extra equipment like WiFi routers)
- Low battery usage desired
- Maximum resilience through data duplication across all devices

---

## Distribution Tiers

### Tier 1: App Store — Existing Open Source App (Comms Layer)
Use an app officials can install from the store for chat, voice, and
store-and-forward messaging.

**Primary candidate: Matrix/Element** (see [Matrix.md](Matrix.md))

### Tier 2: PWA — Results Viewing
Lightweight PWA for competitors/spectators to view results when at HQ over
WiFi or from timekeeper's device. Also serves as UI layer for the native app.

**PWA limitations (confirmed blockers for primary transport):**
- No background sync on iOS
- No Bluetooth peer-to-peer on iOS
- No WiFi Direct access
- No background location on either platform
- No SMS access
- 7-day storage eviction on iOS
- Unreliable push notifications on iOS

**PWA is suitable for:**
- Results display (when connected to local WiFi)
- UI layer inside native wrapper (Capacitor/WebView)
- Data entry when app is in foreground

### Tier 3: Sideload — Timekeeper/Delegate Power Tool
Native app sideloaded onto key officials' devices:
- Runs local web server to serve PWA
- Acts as central results database
- Admin UI for reviewing, adjusting, finalizing results
- Bridge data into Matrix bot for publishing
- Full audit log management

---

## Data Streams

### Stream 1: Timing Data
- Officials → Timekeeper → All
- Small volume, high priority
- Signed by official, reviewed/adjusted by timekeeper
- Timekeeper signs final derived results

### Stream 2: Results
- Timekeeper → All
- Small volume, high priority
- Signed by timekeeper
- Published via Matrix room

### Stream 3: Location Tracking (High Volume — Compacted)
- Competitors → Safety Official
- GPS captures every N seconds
- **Network sync:** Only last known location per competitor (~80 bytes/update)
- **Full logs:** Retained locally on device, transferred directly to timekeeper only
- Compaction: older location data pruned from network sync payload

### Stream 4: Safety Messages
- Any → Safety Official
- Small volume, **critical** priority
- Location-tagged, signed
- Types: crash, medical, road_condition, sos, info
- SMS-capable encoding: ~100-150 chars

### Stream 5: Chat / Voice
- All ↔ All
- Medium volume, low priority
- Handled by Element/Matrix

---

## Multihop Store-and-Forward

### Architecture
Data hops through multiple devices to reach HQ. No single device needs direct
connectivity to the timekeeper. Each official and competitor carries all data
they've seen and syncs with anyone nearby.

### Human Relay Network
Competitors physically travel between isolated officials. Their phones carry
all data seen so far and sync via Bluetooth/WiFi Direct when arriving at
checkpoints.

```
Start Official → (BT) → Competitor car → (BT) → Mid-stage Official
                              |                        |
                              v                        v
                       Carry all data           Forward to next
                       seen so far              competitor/official
                              |                        |
                              v                        v
                       Finish Official → (WiFi) → HQ / Timekeeper
```

### Transport Priority
1. **Local WiFi / Matrix** — Primary when connected
2. **Bluetooth / WiFi Direct** — Nearby sync (stage start↔finish)
3. **Cellular data** — Store-and-forward when signal returns
4. **SMS** — Last resort (satellite SMS on newer phones)
5. **Manual file** — Emergency (USB/airdrop)

### SMS Encoding
```
KT:<event_id>:<test>:<car>:<time_ms>:<penalties>:<sig_prefix>
```
~60-80 chars per timing submission. ~100-150 chars for safety messages.

---

## Roles

| Role | Device | Responsibilities |
|------|--------|-----------------|
| Timekeeper | Sideload + PWA | Signs results, manages keys, full audit log, can be transferred mid-event |
| Safety Official | Sideload or PWA | Real-time location map, receives safety messages |
| Stage Official | PWA | Records timing data, relays data via multihop |
| Competitor | PWA | Views results, carries data, reports location, can swap to official |

---

## Existing Motorsport Software Survey

### What Exists (Open Source)
| Project | Value | Status |
|---------|-------|--------|
| `race-time` (boormat) | Khanacross data model study | Archived, Meteor.js |
| `timing71/livetiming-core` | CTD format, plugin architecture, relay | Active, Python |
| `skoky/OpenRaceTiming` | Modular architecture concept | Concept only |
| `cpvalente/ontime` | Live results display patterns | Active, 913 stars |
| `LateralGs/rallyx_timing_scoring` | RallyCross scoring logic | Club-tested |
| `Kiasersosa/rally-commander` | Rally team management, Next.js | Pre-alpha |
| `RotorHazard` | FPV drone timing, plugin system | Active, 258 stars |

### What Doesn't Exist
- No open-source khanacross timing system (stopwatch, multiple officials)
- No open-source stage rally timing (wall-clock, start/finish separation)
- No combined khanacross + rally system

### Proprietary References
- **RallySafe** — Gold standard for ANZ rally timing (GPS tracking, live results, satellite comms)
- **Rally Scoring** — Dominant North American rally scoring
- **MSNZ POSSUM** — Event management (not timing)

---

## Open Questions / TODO

- [ ] Decision: Matrix primary + mesh later, or invest in mesh from start
- [ ] Deeper investigation of Berty's Wesh Protocol for offline transport layer
- [ ] How to handle the timekeeper role transfer mid-event technically?
- [ ] How does the safety official's location map UI work in practice?
- [ ] Integration with MSNZ POSSUM for event entries/results submission?
- [ ] What happens when multiple timekeepers exist (assistant timekeeper)?
- [ ] How to handle results disputes and audit trail?
- [ ] Competitor self-registration flow?
