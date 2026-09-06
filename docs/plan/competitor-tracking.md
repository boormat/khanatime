# Competitor tracking & safety — BLE tags, trackside beacons, LoRa

## Summary

Track every competitor's position for safety in forest events with **no phone
coverage**: a cheap **car tag** advertises the car's id over BLE; fixed
**trackside beacons** record passings and broadcast a summary of recent
cars seen; phones (competitors' and officials') passively harvest that data
and carry it to HQ as couriers, relaying over Matrix the moment they reach
connectivity. A G-sensor on the tag turns a crash into an immediate beaconed
alert. **LoRa (Meshtastic) is the candidate long-range layer** for official↔
official links across the forest — a field test decides whether it earns a
place.

**Context (from event organisers):** events are held in forest, in deep
valleys. Officials are 10s of km apart, competitors km apart, no mobile
coverage. BLE links only devices within ~10–100 m, so the system is a
**data-mule network**: each phone physically carries what it learned until
the next rendezvous (a car passing a crashed car within metres, an official
arriving at a location, two vehicles meeting on track).

**Related:**
- `docs/research/Bluetooth.md` — BLE multihop relay research for timing data
  (checkpoint → roving phones → HQ via Matrix). The phone-courier layer here
  extends that model; see also its `StoreAndForward.md`.
- `docs/plan/multi-transport.md` / `identity-amendments.md` — wire v2
  observations (uids, `amend`/`void`) and the `khanatime_parcel` handoff
  machinery. Sightings should ride these existing observation formats.

---

## Platform capability matrix (the ground truth everything sits on)

| Capability | WASM (Web Bluetooth) | Native iOS (CoreBluetooth) | Native Android |
|---|---|---|---|
| Scan (listen to adverts) | Yes, **only while page open & screen on**. Chrome-family only (Android/ChromeOS/desktop Chrome; Linux usually). **Not iOS Safari, not Firefox** | Yes. **Background scanning works**: `bluetooth-central` mode + scan filtered to declared service UUIDs, OS-batched | Yes. Background throttled without a persistent **foreground service** (mandatory notification) |
| Broadcast (advertise a beacon) | **No — Web Bluetooth has no advertising/peripheral mode** | Foreground only. **Custom advert payload is never transmitted in background** | Foreground only; OS stops advertising when app backgrounds |
| Connect as GATT client | Yes (same platforms as scan; needs user gesture) | Yes, also in background (opportunistic, app wakes on data) | Yes; background via foreground service |
| Be a GATT server | **No** | Yes, foreground; background limited to existing connections | Yes, foreground; background via foreground service |
| Always-on / background | No — browser suspends BLE | Yes for central scan/connect | Only via foreground service |

**Conclusions that fix the architecture:**
- Phones will **never** be the identity device in the background — the car
  **tag** carries identity.
- Phones **can** listen in the background, but only as a **native app on
  iOS** (service-UUID-filtered scan) or with a foreground service on Android.
  WASM is a foreground/HQ tool only.
- Phone-to-phone BLE works while one phone is **foreground and active**
  (the official's phone); new background relationships are not possible on
  iOS. Established connections persist in background.

---

## Components

### Car tag — XIAO nRF52840 Sense (programmable, G-sensor)

- Advertises a short opaque **car id** (matches `ids.rs` style) on a
  configurable interval (tens–hundreds of ms).
- **Sense variant's onboard IMU** (the reason programmable tags are
  required — fixed-format iBeacon fobs can't log G):
  - **Wake-on-impact**: threshold crossing wakes the nRF from deep sleep
    (µA-class idle); also motion-wake so the tag sleeps until the car moves.
  - **Crash flag in the advert payload**: car id + impact flag + magnitude +
    seconds-since-impact (~5–7 bytes) — every listener sees it from the
    advertisement itself, no connection needed.
  - Freebies worth including: **free-fall** (tag broken loose) and **6D
    orientation** (car on its roof).
  - Detail (waveform snippet) stays behind a **GATT** pull; a small
    last-N-seconds ring buffer persisted to the nRF52840's internal flash is
    a later "black-box" phase, not v1.
- **Battery via ADC** exposed as a GATT characteristic for pre-event fleet
  checks; months of runtime advertising, event-day trivial.
- Provisioning at flash time (USB-C on the XIAO) or OTA via **Nordic DFU**
  over BLE; car id writable via GATT.
- The tag is mounted **securely to the car**, not the phone — the phone is
  loose/decoupled/possibly dead after a crash. The tag is the sensor.

### Trackside beacon (station) — XIAO nRF52840 (or ESP32S3 if WiFi push)

- Listens (scans) for car tags; records sightings `(car id, station id,
  tick)` ~8–10 bytes each into a ring buffer (64–128 entries), persisted to
  internal flash so a powered-off station keeps its memory.
- **Broadcasts the last-N list in its own advert payload** (extended
  advertising, 255 bytes ≈ 25 sightings; cyclic rotation across adverts for
  more). The advert payload is the **primary data channel** — phones get
  data one advert interval after entering range, passively, iOS-background-
  friendly.
- A **GATT service** for on-demand pulls (and a writable char for future
  phone→station deposits / gossip).
- **Half-duplex radio**: scanning and advertising are time-shared — duty
  cycle ~80% scan / 20% advertise, sized so a car passing in 5–10 s is
  certainly caught and a phone in range certainly gets the list. This is the
  main firmware tunable; verify the ambient SoftDevice scan+advertise
  permutation in the prototype (worst case: alternate windows).
- **Clock**: RTC provisioned by the official's phone at placement (station
  has no GPS/clock); before provisioning, relative ticks.

### Phone apps (native + WASM)

- **Harvester** (native on iOS for background; WASM foreground on
  Android/desktop): passively scans station adverts and tag adverts.
- **Courier**: summarises harvested sightings and relays to HQ over Matrix
  when connectivity appears. Summary = most recent seen location of each
  competitor.
- **Peer sightings**: a phone that hears another car's tag logs "car B heard
  by car A's phone at A's GPS fix, T" — localises a stopped/crashed car
  without any station nearby. **Enrichment, not ground truth**: pass-bys in
  opposite directions also trigger; HQ fuses with station sightings and
  time-gaps.
- **Officials' tools**: at station placement, reads/sets the station id +
  RTC and records the phone's GPS fix paired with the station id (the
  beacon has no GPS — this pairing is how HQ maps sightings to coordinates).
  Also: check stations are alive, hear car tags while roving, collect
  stations after the event.
- **Phone-to-phone D2D** (no coverage): connection (GATT, full exchange) or
  connectionless (short message in advert payload — the receiver can be
  fully backgrounded). Works with the official's phone foreground/active;
  only for opportunistic rendezvous, never as an always-on background relay.

### LoRa / Meshtastic (candidate long-range layer)

- **Why it's on the table:** BLE physically cannot bridge km. LoRa is
  licence-exempt in AU (915–928 MHz, LIPD class licence, ≤1 W EIRP, duty
  cycle limits fine for sighting-sized payloads). It is *not strictly*
  line-of-sight (sub-GHz diffraction + deep-sleep sensitivity), but deep
  valleys and mountain ridges genuinely break NLOS links.
- **The mesh is the cars**: if per-hop range through the actual forest > car
  spacing in the running order, the field itself forms the relay chain.
  Decided by one measured number — see Phase 0.
- **What Meshtastic gives:** GPS position broadcast per node (independent of
  phones), mesh text, Store & Forward mailboxes on router nodes, phone
  pairing over BLE (own app can integrate via the open protobuf interface),
  MQTT bridge at any connected node, encrypted channels.
- **What it does not give:** bulk transport (message-sized payloads only —
  fine for sightings), escape from the physics, unbounded mesh diameter
  (hop limits ~3–7).
- **Costs (AU, rough):** bare module (SX1262/RFM95) $6–16; Heltec WiFi LoRa
  32 V3 (ESP32-S3 + SX1262 + OLED) $25–35; LilyGO T3-S3/T-Beam S3 (+GPS)
  $50–65; prebuilt Meshtastic node (SenseCAP T1000-E etc.) $100–130; add
  ~$10–20 per unit battery + IP65 box and $10–25 for a proper 915 MHz whip
  antenna (antenna = where range is won/lost).
- **Safety fallback, separate from all of the above:** a satellite messenger
  (Garmin inReach-class, ~$300 + subscription) for the one message that must
  reach HQ no matter what.

---

## The safety loop (end to end)

1. Impact → tag's advert carries flag + magnitude + recency.
2. Any phone or station in range hears it; a passing car's phone also
   records its own GPS fix (peer sighting).
3. The information rides as a courier until connectivity.
4. HQ sees "car 12, impact ~8 g, near fix (coord), 2 min ago" and dispatches.
5. Stopped-car detection is derived from *absence* of sightings + course
   topology + peer sightings — hearing a beacon proves the beacon is alive
   and the car is at a place; it does not prove the competitor is OK.

---

## Phased delivery

### Phase 0 — Field test (decides LoRa, measures BLE)

- 3× Heltec V3 on Mesh firmware (region AU), driven at realistic spacing
  through the actual event forest: measure max hop. **If hop ≥ car spacing,
  LoRa earns the realtime layer; if not, courier design is unchanged.**
- Measure BLE tag↔station detection range and the passing window (phone in
  range for ~5–12 s at 30–80 km/h over ~100 m footprint).
- Measure a phone→station→phone data pull end to end.

### Phase 1 — Prototype firmware

- Tag: blink-blink beacon + motion wake + impact flag on XIAO nRF52840 Sense
  (embassy + `nrf-softdevice`).
- Station: scan + duty-cycled extended-advert summary + GATT pull, ring
  buffer to flash.
- WASM courier app (foreground): harvest station adverts, show summary.

### Phase 2 — App layer

- **Freeze the station advert payload format first** (it's the one wire
  format that has to be right before hardware ships).
- Native iOS harvester (background scan, declared service UUID).
- Sightings ride wire v2 observation format; merging reuses the parcel /
  content-id dedup machinery.
- Provisioning flow at station placement (station id + RTC + GPS pairing).

### Phase 3 — Integration & HQ

- Relay summaries to HQ over Matrix (existing sync path), dedup by
  content-id, "who's where" view, stopped-car alerts (absence + topology).

### Phase 4 — Production

- DFU fleet updates, battery/persistence hardening, station collection
  workflow, LoRa layer if Phase 0 passes, satellite-messenger alert path.

---

## Open questions

- [ ] LoRa hop range in the actual event forest — the field test number.
  Cars spaced within hop range of each other in the running order?
- [ ] Does the SoftDevice permutation scan + extended-advertise concurrently
  work on the XIAO, or do we alternate windows (and what does that cost)?
- [ ] Car tag mounting position that survives a crash and keeps the antenna
  clear of body metal (windscreen base / roll cage / plastic bumper?).
- [ ] Station advert payload layout: block header, car id width, tick
  encoding, impact bit. Freeze before Phase 2.
- [ ] iOS native scope: harvester-only app, or the whole app native?
- [ ] Does the passing window reliably allow a GATT pull, or must the advert
  payload carry everything (assumed: primary = advert)?
- [ ] Meshtastic store & forward: current firmware maturity for the
  mailbox-on-router role?
- [ ] LIPD class licence transmission limits: confirm current duty-cycle
  terms for 915–928 MHz before the LoRa gateway/backhaul design.
- [ ] Impact ring buffer (black-box waveform) — v1 or later?