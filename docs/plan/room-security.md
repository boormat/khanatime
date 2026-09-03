# Timing-room security lockdowns

> Status: **design for first official release.** Today rooms are created with
> `history_visibility: world_readable` and effectively open write once joined
> (`docs/plan/event-admin-accounts.md`). Signing/TOFU rejects forged *bodies*,
> but Matrix still allows anyone who can join to attempt sends.
>
> Related: `docs/plan/qr-join.md`, `docs/plan/release-versioning.md`,
> `docs/plan/event-admin-accounts.md`, `docs/plan/identity-login.md`.

## Goal

**Timing room is not public-write.** Capability to write is gated by how you
got into the room. The invite QR is a **secret** — treat a printed QR like a
physical key to the timing desk.

App-level signature checks stay; they are defence-in-depth, not the only lock.

---

## Matrix baseline (all levels)

On create / publish:

| Setting | Value | Why |
|---------|--------|-----|
| `join_rules` | `invite` | No walk-in joins from a room id / directory |
| `history_visibility` | `shared` (or `invited`) for writers; see spectators | Avoid anonymous world peek of live timing if undesired |
| `power_levels.events_default` | elevated (e.g. 50) | Joined-but-not-writer cannot send `m.room.message` |
| Officials | PL ≥ `events_default` | Can post timing |
| Spectators (if members) | PL 0 | Read / sync only |
| Room creator / owner | admin PL | Invite, PL changes, close |

Signing gate (`docs/bugs.md` B6) remains: unsigned / bad-sig observations
never enter runs/scores even if Matrix accepted the send.

**No separate public results room for v1.** If spectators can read the timing
room (membership read-only, or a deliberate world-readable peek policy),
finalisation is “event status → Finished” + Results UI — not a second room.
Revisit only if we need a sanitised public feed stripped of official chatter.

---

## Spectator access

Two options (pick per event, default A for practice):

- **A. Discover / peek** — keep timing history world_readable *or* publish a
  read-only view later; no membership. Simple; anyone with room id can read
  history in Element. Write still invite+PL.
- **B. Read-only invite** — spectator QR / link invites with PL 0. Clearer
  audit of who watched; more invite traffic.

Default recommendation: **A for public results curiosity**, **B for club
events that want a known gallery list**. Neither gets write PL.

---

## Level 1 — Standard (printed official QR)

**Threat model:** QR is secret. Only officials may scan it. Leaked QR ≈ leaked
write access until revoked.

Flow:

1. Publish creates invite-only timing (+ space) rooms.
2. Official invite QR embeds enough for the app to:
   - open the **pinned app release** URL (`release-versioning.md`),
   - land the join query (`homeserver/event/sid/tid/reg`),
   - obtain a Matrix **invite** into the timing room with **write PL**
     (embed signed invite / use publisher session to invite after register —
     exact mechanism is an implementation task; must not leave the room
     `public` join).
3. Organiser prints QR for the weekend (or shows laminated copy at HQ).
4. Revocation: kick / PL demote + rotate invite (new QR) if one leaks.

Spectators use A or B above — **not** the official write QR.

---

## Level 2 — Screen-to-screen only

Same Matrix model as Level 1, operational discipline:

- **Do not print** the write QR.
- Key official / HQ laptop shows the invite QR only when an official is
  standing there; dismiss after scan.
- Optional: short-lived invite tokens (regen QR every N minutes) — nice-to-have,
  not required for first cut.

Use when the paddock is open but you do not want a photocopied key walking
around.

---

## Level 3 — Enrolment by key official

No ambient write QR at all.

1. Official opens **Accounts** / identity and shows **their** device/account QR
   (identity only — not a room invite).
2. Key official scans it on an already-authorised device.
3. App on the key-official device: register/link that identity, **invite** them
   to the timing room, grant write PL, optionally add to Organisers list.
4. Official’s phone resumes / joins via membership (no shared secret poster).

This is the strongest operational control and the most UI work. Target after
Level 1 works; practice day can stay on Level 1–2.

---

## Results finalisation

- Clerk marks event **Finished** (existing lifecycle).
- Officials keep write until PL revoked or room closed; spectators keep read.
- Printed/PDF results stay client-side (`print` stylesheet).
- **No** mandatory second “public results” Matrix room for v1.

If a fully public live board is needed without exposing official traffic,
revisit a derived read-only room or a static export — out of scope until an
event asks for it.

---

## Mapping to current code

| Today | Target |
|-------|--------|
| `history_visibility: world_readable` on create | Revisit: writers `shared`/`invited`; spectator policy explicit |
| Join via room id after register (open HS) | Must **invite** before join; QR carries invite capability |
| Signing/TOFU on bodies | Keep |
| Official invite URL query | Add app pin base URL; add invite/PL semantics |
| Auth backlog “ignore non-admins” | Align with PL + organiser list, not instead of room ACL |

---

## Checklist (first release)

- [ ] Publish: `join_rules: invite`, elevated `events_default`
- [ ] Official join path always results in invite + write PL (Level 1)
- [ ] Document QR-as-secret; practice guide: do not post write QR publicly
- [ ] Spectator policy A or B selectable (or fixed default A)
- [ ] Level 2: UI affordance “show invite QR” without implying print
- [ ] Kick / rotate invite documented for leak response
- [ ] Decide history_visibility for timing vs space (chat may differ)

## Checklist (after Level 1)

- [ ] Level 3 enrolment: scan official identity QR → invite + PL + organisers
- [ ] Optional short-lived invite tokens (Level 2 hardening)
- [ ] Public derived results feed only if an event requires it
