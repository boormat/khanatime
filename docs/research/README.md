# Research Notes

> These files were carried over verbatim from `khanatime26` (Flutter prototype,
> now archived) so the design history survives. They are research/background
> notes, not a description of the current system. Where the current direction
> differs, this file flags it and the decision is recorded in `PLAN.md`.

## Files

| File | Topic | Status |
|------|-------|--------|
| `Matrix.md` | Matrix transport architecture | **Partly superseded** — multi-room (general/timing/results/safety/location) design; current plan uses a **single shared room per event** with store-and-forward via room history |
| `MessagingSpike.md` | Matrix spike conclusions / decision record | Current — landed on one room per event named "timing"; local echo + broadcast is the sync model |
| `Cryptography.md` | Identity & signatures | Background — Matrix E2EE + per-official DID concepts inform identity design |
| `Architecture.md` | Original architecture notes + open questions | Background — several open questions remain open in `PLAN.md` |

The offline-comms research (`Bluetooth.md`, `Berty.md`, `StoreAndForward.md`)
moved to the **boomtrack** repo (private, `boormat/boomtrack`) with the
competitor-tracking project (BLE tags, trackside stations, courier phones,
LoRa/GPS) on 2026-09-06. This repo is khana-cross timing only.

## Current direction (see PLAN.md)

- **Transport:** single shared Matrix room per event; room history replays as the
  offline store-and-forward sync. Serverless clients, no dedicated server.
- **Storage:** local (localStorage / IndexedDB) as source of truth, synced via
  the room.
- **Timing:** start and finish are separate event records; elapsed computed from
  pairing key `(event_id, test_number, car_number, run_number)`.
