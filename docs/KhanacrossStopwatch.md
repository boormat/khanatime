# Khanacross — Stopwatch Mode Data Entry

> Start and finish are **separate event records**. Elapsed time is computed
> when both exist. Officials only record what they observe at their location.
> One phone can operate Both mode (start + finish buttons → two records).

---

## Context

A khanacross test is typically:
- Under 1 km, times 30–90 seconds
- Start and finish at the same spot or separate garages
- 2+ officials for redundancy (regs: average ≥2 stopwatches when manual)
- Penalties observed at the relevant end (jump start at start; flags/NFG at finish)

The app replaces paper sheets. Officials tap start or finish; data syncs to
the timekeeper over WiFi (and later BLE).

---

## Position Modes

On joining a test, the official chooses:

| Mode | Creates | Typical use |
|---|---|---|
| **Start** | `start_events` only | Separate start garage; or shared stopwatch “clicker” |
| **Finish** | `finish_events` only | Separate finish garage; or shared stopwatch “stopper” |
| **Both** | start then finish on one phone | Co-located single official; still two DB records |

**Shared stopwatch (two officials, same spot):** Official A on Start mode,
Official B on Finish mode. A gets a clean start click; B gets a clean stop
click. Pairing key links them into one elapsed time.

---

## Pairing Key

```
(event_id, test_number, car_number, run_number)
```

- **Start official** assigns `run_number` (auto-increment per car per test)
- **Finish official** selects the matching pending start (or types car / picks list)
- Elapsed = finish.timestamp − start.timestamp
- Multiple start or finish observations for the same key are averaged by the timekeeper

---

## Start Official Workflow

1. Select test, position **Start**
2. Enter car number (pad or quick-select)
3. Run number shown (auto next for that car)
4. Tap **▶ START** → `start_event` with status `clean`
5. Or **DNS** → no usable start time, status DNS
6. Or **Jump Start** → timestamp + status `jump_start` (+5s flat on that run)
7. Screen ready for next car

### Start screen

```
┌─────────────────────────────────┐
│  Test 1 — START    Official: Mat│
│                                 │
│         Car #23                │
│         Run 1                  │
│                                 │
│    ┌────────────────────────┐   │
│    │     ▶ START            │   │
│    └────────────────────────┘   │
│    [DNS]  [Jump Start]          │
│                                 │
│  Last: Car 17 R1 started ✓    │
└─────────────────────────────────┘
```

### Start penalties

| Action | Status | Time effect |
|---|---|---|
| Clean start | `clean` | — |
| Failed to present / not start in 1 min | `DNS` | Slowest + 10s (at test resolve) |
| Jump start | `jump_start` | **+5s** on this run |

---

## Finish Official Workflow

1. Select test, position **Finish**
2. See **live feed** of cars started but not finished (start time, age, start penalties)
3. Select car: tap pending row, **or** type car number, **or** pick from entrant list
4. Tap **⏹ FINISH** — **always allowed**, even with no car selected (orphan finish; timekeeper assigns car later)
5. Post-finish summary: start info (if paired) + elapsed + finish penalties
6. Set **flag count** and other finish penalties
7. **Submit** → `finish_event`

### Finish screen — waiting

```
┌─────────────────────────────────┐
│  Test 1 — FINISH  Official: Sar │
│                                 │
│  ── Waiting ────────────────── │
│  Car 17 R1  11:05:23  12s ago  │
│  Car 7  R1  11:05:45   8s ago  │
│                                 │
│  Selected: Car 17               │
│  [type #] [entrant list]        │
│                                 │
│    ┌────────────────────────┐   │
│    │     ⏹ FINISH           │   │
│    └────────────────────────┘   │
│  (works without selection)      │
│                                 │
│  Last: Car 23 01:12.34 ✓      │
└─────────────────────────────────┘
```

### Finish screen — after stop (summary)

```
┌─────────────────────────────────┐
│  Car #17 R1 Williams            │
│                                 │
│  Start:  11:05:23.456 (Mat)    │
│  Finish: 11:06:35.796 (Sarah)  │
│  Elapsed: 01:12.34             │
│                                 │
│  Start: Clean ✓                │
│  Flags: [−] 1 [+]  (+5s each)  │
│  [NFG] [Wrong Dir] [Missed]    │
│  [Reversed] [DNF] [Order] [DSQ]│
│                                 │
│  Net: 01:17.34  (1F)           │
│  [✓ SUBMIT]  [Discard]         │
└─────────────────────────────────┘
```

### Finish penalties

| Control | Meaning | Time effect |
|---|---|---|
| **Flags** (marker_hits) | Number of flags/markers hit | **+5.00s per flag** |
| **NFG** | Not Finished Garage — any part of car outside finish garage | **+5.00s plus flags** |
| Wrong direction | Incorrectly completed test | Slowest + 5 |
| Missed mid-course stop | Failed to stop in mid-course garage | Slowest + 5 |
| Reversed | Reversed after exceeding garage limits | Slowest + 5 |
| DNF | Did not finish | Slowest + 5 |
| Wrong order | Running out of order | Slowest + 10 |
| DSQ | Speed / other disqualification | DSQ |

**NFG** = regs abbreviation for finish outside garage (Rule 12.1).  
Flags and NFG both apply when both are set (e.g. NFG + 2 flags = +5 + 10 = +15s).

Slowest-based penalties are stored as status flags and resolved when the
test is complete (slowest clean time known).

---

## Both Mode (single official, co-located)

```
┌─────────────────────────────────┐
│  Test 1 — BOTH     Official: Mat│
│                                 │
│         Car #23  Run 1         │
│                                 │
│  ┌──────────┐  ┌──────────┐    │
│  │ ▶ START  │  │ ⏹ FINISH │    │
│  └──────────┘  └──────────┘    │
│                                 │
│  Start: 10:23:45.12 ✓          │
│  Elapsed: 00:23.47 running…    │
└─────────────────────────────────┘
```

1. Enter car → ▶ START → start_event  
2. ⏹ FINISH → finish_event + penalty UI  
3. Same pairing key; same official_id on both records  

---

## Multi-Stopwatch Averaging

Regs require ≥2 manual stopwatches, averaged.

| Scenario | How |
|---|---|
| Two full stopwatches (co-located) | Two phones in Both mode, or two Start+Finish pairs → timekeeper averages start times and finish times separately |
| Shared start/finish clickers | One Start phone + one Finish phone → one pair, no average needed |
| Multiple finish observers | Multiple finish_events same key → average finish timestamps |

Timekeeper can discard outlier start or finish submissions independently.

---

## Test Overview (runs per test)

Shows pairing status per car:

```
Car  Driver     Start   Finish   Net     Status
 7   Smith      ✓ R1    ✓ R1    45.23   ✓ Done
12   Jones      ✓ R1    —       —       ○ In progress
17   Williams   ✓ R1    ✓ R1    46.89+5 ⚠ 1F
23   Brown      —       —       —       ○ Open
31   Davis      DNS     —       S+10    ✗ DNS
```

Best X of Y still applies to completed paired runs (see below).

---

## Best X of Y

Event config: `best_x_of_y` (standard khanacross = 1 of 1).

For each car/test:
1. List completed run_results (paired start+finish)
2. Exclude DNS/DSQ from counting pool as appropriate
3. Sort by net_ms; best `x` count toward aggregate
4. Status: Done / Short / Open / None (same as before)

---

## Categories (not classes)

- Drivers have **zero or more categories** (Outright, Junior, Female, Buggy, …)
- Outright auto-created; default membership on entry
- Results published Overall + per category
- Driver with no categories: timed, Overall only

Entry management still supports late entry, withdraw, scratch (unchanged
behaviour; assign categories on add/edit).

---

## Data Submission Formats

### Start event

```json
{
  "type": "start_event",
  "event_id": "khanacross_2025_001",
  "test": 1,
  "car": 23,
  "run": 1,
  "official": "did:key:z6Mk...",
  "timestamp": "2025-09-14T10:23:45.12Z",
  "status": "clean",
  "signature": "base64..."
}
```

### Finish event

```json
{
  "type": "finish_event",
  "event_id": "khanacross_2025_001",
  "test": 1,
  "car": 23,
  "run": 1,
  "official": "did:key:z6Mk...",
  "timestamp": "2025-09-14T10:24:57.46Z",
  "marker_hits": 1,
  "status": "clean",
  "signature": "base64..."
}
```

Orphan finish (no car selected yet):

```json
{
  "type": "finish_event",
  "car": null,
  "run": null,
  "timestamp": "...",
  "status": "unassigned"
}
```

---

## Edge Cases

| Case | Handling |
|---|---|
| Car starts, never finishes | Start exists; finish official or timekeeper marks DNF |
| Finish with no start (comms fail) | Orphan finish or typed car; timekeeper pairs/creates start or uses manual time |
| Finish before start arrives on finish phone | Select car by number; pair when start syncs |
| Two starts same car/run | Timekeeper averages or discards one |
| Official misses start | Late Start flag or re-run (Clerk of Course) |
| Official misses finish | Late Finish or orphan timestamp |
| Jump start + flags | +5 jump + 5×flags on net |

---

## Sync

1. Save locally first  
2. WiFi → Matrix #timing  
3. Later: BLE multihop  
4. Timekeeper live feed shows starts, finishes, and in-progress separately  

See [PLAN.md](../PLAN.md) for schema and sprints.
