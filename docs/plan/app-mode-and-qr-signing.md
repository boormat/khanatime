# Plan: App Mode System + QR Parcel Signing

**Date:** 2026-08-18
**Status:** Part 1 (App Mode) is **obsolete** — the mode system was removed.
The user's role is now derived from the open event + identity (`app::Role`,
`refresh_role`), not picked (`docs/plan/identity-login.md`). Part 2 (QR parcel
signing) landed as per-observation signing (see `docs/research/Cryptography.md`).
**Note:** Other code changes (Entry trim, homeserver picker fix) are landing simultaneously.

---

## Part 1: App Mode System

### Problem
The navbar shows all 13 screens to everyone. Different users need different views:
- Competitors just want results and self-entry
- Officials need timing screens
- Organisers need event admin
- Spectators just want results
- Testing shows everything for debugging

### Design

**5 modes**, stored in localStorage (`kt_mode`), default = Competitor:

| Mode | Visible Screens |
|------|----------------|
| Testing | All 13 screens |
| Organiser | Home, Events, Accounts, Event, Start, Finish, Stage, Stopwatch, Results, Entries, Chat, Help, KhanaRules |
| Spectator | Home, Results |
| Official | Home, Events, Start, Finish, Stage, Stopwatch, Results, Entries, Chat, Help, KhanaRules |
| Competitor | Home, Events, Results, Entries, Help, KhanaRules |

**Mode picker:** Bulma `navbar-item has-dropdown is-hoverable` at the end of the navbar brand. Shows all 5 modes, current highlighted with `is-active`. Click calls `Msg::SetMode(m)`.

**Flat icon bar:** Kept. Icons are filtered by mode — only visible screens appear. Icons stay in fixed canonical order (no rearranging on mode switch).

### Implementation

**Single file change:** `src/app.rs`

1. Add `Mode` enum with `visible_screens()`, `label()`, `all()`, `from_storage()`, `save()` methods
2. Add `pub mode: Signal<Mode>` to `Model` + init in `Model::init()`
3. Add `Msg::SetMode(Mode)` variant + handler (saves to localStorage, sets signal, redirects to Home if current screen invisible)
4. Rewrite `view_navbar`: filter icons by `model.mode.get().visible_screens()`, add mode picker dropdown
5. Add `storage()` helper (private, same pattern as `event.rs`)

**No other files change.** The mode is purely a UI filter — screen content, routing, and permissions are unaffected.

### Edge cases
- Mode switch redirects to Home if current screen is invisible
- Testing mode is always available (no security through obscurity)
- `kt_role` / `local_role()` / `set_local_role()` were removed (unused; a role-picker was never built — the event role now lives on `Official.role`)
- `view_content()` unchanged — it still renders all screens; visibility is navbar-level only

---

## Part 2: QR Parcel Signing

> **Status: DONE (implemented differently from the plan below).** Signing landed as
> a **per-observation** Ed25519 signature on `TimingEvent` / `EventInfo` (the
> `signature` / `signing_key` fields), verified in `replay` and
> `sync::handle_incoming` via `signing::verdict_with` with default-deny. The
> parcel-envelope `sig`/`key` fields described below were NOT added to the
> `Parcel` struct; per-message signing is preferred because it survives relay and
> QR import intact. See `docs/research/Cryptography.md` (Signing Model / Trust
> Model) for the current policy.

### Problem
QR parcels (`khanatime_parcel:{json}`) have zero authentication. Any device that scans the QR can import the parcel. While this is acceptable for the trust model (you trust the person whose QR you scan at an event), there's no way to detect accidental corruption or confirm the sender's identity.

### Research: Matrix E2EE Keys

The matrix-sdk is configured with **E2EE disabled** (`default-features = false, features = ["js", "indexeddb"]`). Enabling `e2e-encryption` would:
- Pull in the full `matrix-sdk-crypto` stack (vodozemac, olm, megolm)
- Significantly increase WASM binary size
- Require OlmMachine initialization before signing
- Tie signing capability to having an active Matrix session

**Verdict:** Matrix device keys are not practical for this use case.

### Recommended: Standalone Ed25519 Signing

Add `ed25519-dalek` (already a transitive dependency via vodozemac) as a direct dependency. Generate a dedicated signing keypair on first use, persist in localStorage, sign parcels independently.

**Flow:**

```
EXPORT (sender):
  1. Generate/load device signing keypair (localStorage)
  2. Serialize parcel JSON
  3. Sign the JSON payload with Ed25519 private key
  4. Include signature + public key in the parcel envelope
  5. Compress + frame + render QR

IMPORT (receiver):
  1. Decode QR frames → decompress → parse envelope
  2. Verify Ed25519 signature against embedded public key
  3. If valid: import messages, store sender's public key (trust-on-first-use)
  4. If invalid: reject with error message
```

**Parcel envelope v2:**
```json
{
  "v": 2,
  "event_uid": "...",
  "messages": [...],
  "sig": "<base64 Ed25519 signature of messages JSON>",
  "key": "<base64 Ed25519 public key>"
}
```

**Size impact:** ~150 bytes added to parcel JSON (88-char signature + 44-char key + JSON framing). After DEFLATE compression: negligible (<0.1% of typical event log). Per-frame overhead if signature goes in Frame struct: ~95 chars, adding 1-2 extra frames for a typical parcel.

**Trust model:** Trust-on-first-use (TOFU). First scan from a device stores its public key. Subsequent parcels from the same key are trusted automatically. Different key = unknown sender = warning prompt.

### Implementation (separate from Part 1)

**New file:** `src/signing.rs`
- `DeviceKey` struct (Ed25519 keypair, persisted in localStorage as `kt_signing_key`)
- `DeviceKey::generate()` — creates new keypair
- `DeviceKey::load()` — loads from localStorage or generates
- `DeviceKey::sign(payload: &[u8]) -> Signature`
- `DeviceKey::public_key() -> [u8; 32]`
- `verify_signature(payload: &[u8], sig: &[u8; 64], key: &[u8; 32]) -> bool`

**Modified files:**
- `src/services/qr.rs` — `Parcel` struct gains `sig` and `key` fields (v2 envelope); `pack_parcel` signs; `unpack_parcel` verifies
- `src/signing.rs` — new module

**Dependencies:**
- `ed25519-dalek` — direct dependency (already transitive via vodozemac)

### Sequencing

Part 1 (App Mode) and Part 2 (QR Signing) are independent and can be developed in parallel. Part 2 is a larger change that benefits from the entry trim (smaller parcels after removing fields).

---

## Part 3: AGENTS.md Update

Add post-plan checklist to AGENTS.md Conventions section:

```markdown
## Post-plan checklist

After completing a multi-file change or feature:

1. **Dead code audit**: `cargo clippy --all-targets -- -D warnings` and
   `cargo clippy --target wasm32-unknown-unknown -- -D warnings`. Remove or
   `#[allow(dead_code)]` (with justification) any new dead code.
2. **Redundancy check**: Search for duplicate functions, orphaned helpers,
   and test helpers that no longer test anything.
3. **Stale docs**: Grep for removed concepts in comments and doc strings.
   Update or remove.
4. **Stale plans**: Check if `docs/plan/` files reference work that is now
   complete. Mark them done or remove.
5. **Test coverage**: Verify `cargo test` passes. Check that new code paths
   have test coverage and broken tests are fixed or removed.
6. **Formatting**: Run `cargo fmt` before committing.
```

Also update the stale navigation/layout rework reference at the bottom of AGENTS.md.

---

## Execution Order

1. **AGENTS.md** — add post-plan checklist (standalone, no code deps)
2. **Part 1: App Mode** — `src/app.rs` only (Mode enum, Model signal, Msg handler, navbar rewrite)
3. **Verify Part 1** — `cargo fmt && ./scripts/check.sh`
4. **Part 2: QR Signing** — `src/signing.rs` (new), `src/services/qr.rs` (modified), `Cargo.toml` (add ed25519-dalek)
5. **Verify Part 2** — `cargo fmt && ./scripts/check.sh`
6. **Commit** — two commits (one per part)
