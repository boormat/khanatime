# Remote setup — outline (organiser guide)

> To be written up. This is a stub outline for two modes: **offline venue** and
> **hotspotted relay**.

## The one idea
All phones load the app from the event laptop; the laptop runs the homeserver
(+ printer). No internet needed. Scanning a QR joins the event.

## Networking — three options
- **A. Travel router (recommended, ~$40, reusable):** DHCP reservation → laptop
  IP never changes. Works offline and for the relay.
- **B. Laptop-as-hotspot (free, Linux):** AP + DHCP; laptop IP fixed.
- **C. Venue wifi:** join as client; ask for a DHCP reservation, else rely on
  sticky lease. Don't force a static IP. (Mesh only if venue already has it.)

## Mode 1 — Offline venue
No internet. Laptop AP/router + local HS + app over LAN http. Publish → join QR
→ phones scan → sync via room history. QR parcel handoff = fallback if wifi dies.

## Mode 2 — Hotspot relay (partial online)
Travel router with a cellular/venue uplink. HQ dual-connected (local HS +
public HS); field→public, LAN→local, HQ relays. If internet drops, fall back to
local room + QR parcels.

## Troubleshooting
- Phones can't reach HS: wrong wifi, or IP changed → regenerate join QR.
- Mixed-content: serve app over LAN http, not the hosted Pages site.
- Hotspot-owner-leaves: put the hotspot on a fixed device.

## Checklist
One-page organiser printout.

## Open question
Relay (dual-HS) is still unimplemented in `multi-transport.md` — write Mode 2
as current single-local-HS reality or as the planned dual-HS?
