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
| `Bluetooth.md` | BLE phone-to-phone options | Background only — BLE may supplement Matrix offline, not a primary transport |
| `Berty.md` | Berty PWA deep-link idea | Background only — not adopted |
| `StoreAndForward.md` | Mesh / store-and-forward landscape | Background only — room history is our store-and-forward layer |
| `Cryptography.md` | Identity & signatures | Background — Matrix E2EE + per-official DID concepts inform identity design |
| `Architecture.md` | Original architecture notes + open questions | Background — several open questions remain open in `PLAN.md` |

## Current direction (see PLAN.md)

- **Transport:** single shared Matrix room per event; room history replays as the
  offline store-and-forward sync. Serverless clients, no dedicated server.
- **Storage:** local (localStorage / IndexedDB) as source of truth, synced via
  the room.
- **Timing:** start and finish are separate event records; elapsed computed from
  pairing key `(event_id, test_number, car_number, run_number)`.
