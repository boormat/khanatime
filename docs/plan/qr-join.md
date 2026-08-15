# QR join links — phone-to-phone bootstrap

## Summary

Let one device display a QR code that another phone scans with its plain camera
app, landing **inside the WASM app** pre-configured: homeserver, event, and
(optionally) credentials. The scanned device then connects, adopts the
published event, joins its timing room, and backfills — zero typing.

A QR is just text; a URL payload means any phone's camera opens the app. The
app reads `window.location.hash` at startup and routes itself. No NFC (Web NFC
is Chrome-Android-only, NDEF-tag-only, no phone-to-phone since Android Beam was
removed; iOS exposes nothing to the web).

**Related:** `docs/plan/multi-transport.md` — the join link here is the
bootstrap for the room transports there (dual homeserver relay + QR parcel
handoff, event/observation ids with `amend`/`void`).

Everything below builds on existing pieces: `sync::connect` (login),
`events::open_result` (adopt published event), `Msg::SetEvent` (replay + join +
backfill). New work is a parser, one `Msg` variant, a startup hook, and the QR
display.

## Link format

```
{app-origin}/#join?hs=http%3A%2F%2F192.168.1.10%3A8008&ev=kt-2026-mydogs&u=timer&p=chase
```

- `#join?` — namespaced so no other hash use can collide.
- `hs` — LAN-reachable homeserver URL (percent-encoded).
- `ev` — event id. The space alias is derivable once connected:
  `#<ev>:<server_name>` (see `alias()` in `services/matrix.rs`). Accept a full
  alias (leading `%23`) as a fallback form.
- `u` / `p` — optional credentials. Only sane on a LAN/dev server where
  `register_or_login` accepts anything. Never for a public HS.
- ~100 chars → tiny, low-density QR that scans instantly. (QR practical
  ceiling is ~1–2KB; we're nowhere near it. Spec max is 2,953 bytes binary at
  V40-L if ever needed for animated data transfer — separate topic.)

## Startup flow

`main.rs` today: `Model::init()` → `setup_effects` → `warm_start()` picks
Screen → `sync::resume_on_load(model)`. New:

```rust
let model = Model::init();
app::setup_effects(model);
#[cfg(target_arch = "wasm32")]
if let Some(link) = join::from_location() {
    join::consume_hash();           // history.replaceState: reload resumes normally
    app::show(model, Screen::Home); // conn status visible while joining
    crate::update(model, Msg::Join(link)); // link OVERRIDES warm_start/resume_on_load
} else {
    // existing warm_start()/resume_on_load path unchanged
}
```

The link wins over the persisted session: it's an explicit "join this event on
this server" instruction. `save_session` overwrites storage during the
pipeline, so subsequent loads resume the joined event normally.

## Components

### 1. `src/join.rs` (new module)

Parser is pure Rust — **not** wasm-gated — so `cargo test` covers it natively.
Only location/history access is gated.

```rust
pub struct JoinLink {
    pub hs: String,
    pub event: String,
    pub user: Option<String>,
    pub pass: Option<String>,
}

/// Parse "#join?hs=..&ev=..[&u=..&p=..]" — hand-rolled pct-decode (keep
/// native-testable; no web_sys::UrlSearchParams). None if not a join hash
/// or hs/ev missing.
pub fn parse_join_hash(hash: &str) -> Option<JoinLink> { ... }

#[cfg(target_arch = "wasm32")]
pub fn from_location() -> Option<JoinLink> {
    let hash = web_sys::window()?.location().hash().ok()?;
    parse_join_hash(&hash)
}

/// Clear the hash without reload so a refresh doesn't re-run the join.
#[cfg(target_arch = "wasm32")]
pub fn consume_hash() { /* history().replace_state_with_url(NULL, "", path_only) */ }
```

### 2. `Msg::Join` in `app.rs`

```rust
pub enum Msg {
    // ...
    Join(crate::join::JoinLink), // arrived via QR/URL: connect + adopt event
}
// update(): Msg::Join(link) => crate::join::go(model, link),
```

Plus one new `AppState` field for the missing-creds path:

```rust
pub pending_join: Signal<Option<crate::join::JoinLink>>,
```

### 3. Join pipeline — `join::go()`, wasm, lives beside `sync::connect`

It shares `sink_for` / `start_sync` / `ConnState` plumbing, so put the async
body in `sync.rs` (e.g. `pub fn join_via_link(model, link)`) and keep
`join.rs` to struct+parse; alternative is making `sink_for` `pub(crate)`.

Chains the login half of `sync::connect` with the adopt half of
`events::open_result`:

```rust
spawn_local(async move {
    let res = async {
        let client = matrix::new_client(&link.hs).await?;
        match (&link.user, &link.pass) {
            (Some(u), Some(p)) => matrix::register_or_login(&client, u, p).await?,
            _ => { /* stored session for this hs, else Err → pending_join */ }
        }
        matrix::save_session(&client, &link.hs);
        matrix::set_client(Some(client.clone()));
        let ev = matrix::open_published_event(&client, &alias).await?;
        crate::event::enqueue_event_setup(&ev);   // BEFORE SetEvent — see note
        matrix::start_sync(client, sink_for(model));
        Ok::<_, String>(ev)
    }).await;
    match res {
        Ok(ev) => {
            // set identity/conn exactly like sync::connect does
            crate::update(model, Msg::SetEvent(ev.id));       // replay + join timing room + backfill
            crate::update(model, Msg::Show(Screen::Results)); // scanned devices are usually spectators
        }
        Err(e) => model.app.conn.set(ConnState::Error(e)),
    }
});
```

- **Alias**: add a `matrix.rs` helper (`space_alias_for(client, ev)` or accept
  an id in `open_published_event`) deriving `#<ev>:<server_name(client)>`.
- **Ordering matters**: `enqueue_event_setup(&ev)` before `SetEvent`, same as
  `open_result` — the seeded manifest is what gives the replayed event its
  `timing_alias`/`timing_id`, so `join_current_event` joins the per-event room
  instead of falling back to `join_or_create_room`.
- **Demo events never appear in links** (local-only, as today). Link join
  requires a *published* event (`open_published_event` reads `io.kt.event`
  space state) — consistent with "publish before timing".

### 4. Missing-creds path

When the link has no `u`/`p` and no stored session matches `hs`:

1. `model.app.pending_join.set(Some(link))`, prefill
   `model.screens.home.homeserver` from the link, land on Home with a
   "log in to join {event}" hint.
2. Tail of `sync::connect()` (success branch):

```rust
if let Some(link) = model.app.pending_join.get_clone() {
    model.app.pending_join.set(None);
    crate::update(model, Msg::Join(link)); // resume, now logged in
}
```

### 5. QR display (timekeeper side)

- "Show join QR" button on Event admin and/or Home comms section.
- Rust `qrcode` crate (pure Rust, wasm-fine, no `getrandom` — Cargo.toml pins
  untouched), render to SVG/canvas in a modal.
- **URL derivation**: base = `window.location.origin`; hs default =
  `http://{location.hostname}:8008` — NOT the stored homeserver, which is
  typically `localhost:8008` on the timekeeper's laptop and wrong everywhere
  else. Editable hs input before showing the code.

## Task list

- [ ] `src/join.rs`: `JoinLink`, `parse_join_hash` (+ pct-decode), unit tests
      (valid full/minimal link, missing hs/ev → None, non-join hash → None,
      full-alias fallback, encoded chars in u/p).
- [ ] `app.rs`: `Msg::Join` variant + dispatch; `pending_join` signal on
      `AppState`.
- [ ] `sync.rs`: `join_via_link` pipeline (login → open published → seed
      manifest → start_sync → SetEvent → Show(Results)); resume `pending_join`
      at the end of `connect()`'s success branch.
- [ ] `services/matrix.rs`: alias-from-event-id helper for the join.
- [ ] `main.rs`: startup wiring (`from_location`, `consume_hash`, override
      warm path).
- [ ] Home: prefill hs + pending-join hint when `pending_join` is set.
- [ ] QR display: `qrcode` crate, modal with join URL + hs override input,
      origin/hostname derivation.
- [ ] Tests: parser units; `./scripts/check.sh` green (fmt + clippy
      `--all-targets` + tests — same as CI gate).
- [ ] Docs: AGENTS.md src tree gains `join.rs` (tree is already stale —
      app.rs/log.rs/replay.rs/services/ missing; fix while there).
- [ ] Manual LAN test (below).

## Gotchas

- **Mixed content**: an app loaded over HTTPS (GitHub Pages) cannot reach a
  plain-`http` LAN Synapse — browser blocks it. LAN QR join ⇒ serve the app
  itself over LAN http (`trunk serve --address 0.0.0.0`), which the
  origin-derivation above handles automatically. Pages deployment pairs only
  with TLS/public homeservers.
- **server_name vs URL**: aliases embed the HS `server_name` (`:localhost`
  for the dev podman stack) but resolve fine through any URL pointing at that
  HS — so `#kt-…:localhost` works from a phone using `http://192.168.x.x:8008`.
- **Idempotent**: a device that already has the session/event just re-logs and
  re-joins; LWW merge keeps state consistent.
- **Hash consumption**: always `consume_hash()` after reading, else a refresh
  force-rejoins and stomps an account switch.

## Manual LAN test

1. `podman start synapse`; serve app on LAN (`trunk serve --address 0.0.0.0`).
2. Laptop: log in, plan event, publish, "Show join QR".
3. Phone (same wifi): scan with camera app → app loads from laptop origin →
   auto-register/login → lands on Results with event adopted.
4. Laptop: add an entry/stage time → phone Results updates live.
5. Refresh phone → resumes via `resume_on_load`, no re-join.

## Follow-ups (not in scope)

- In-app QR **scanning** (`getUserMedia` + `BarcodeDetector`; Firefox needs a
  jsQR fallback; requires HTTPS or localhost) — `events.rs` already has
  `Msg::ScanQr` / `Msg::EnterRoomId` stubs; `EnterRoomId` is nearly free once
  the join pipeline exists (parse alias → same adopt path).
- Animated QR sequences for **data** transfer (offline event handoff without
  a homeserver) — fountain-coded frames, a few KB/s.
- WebRTC DataChannel sync bootstrapped by a QR SDP exchange.
