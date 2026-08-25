# Passwordless matrix.org login — OIDC (MAS) SSO flow

## Summary

Let users sign in with matrix.org accounts that have no password — e.g. those
created via **"Continue with Google"** on matrix.org's login page. matrix.org
now runs **MAS** (Matrix Authentication Service), an OIDC-native auth layer:
`account.matrix.org/login` offers Google / Apple / GitHub / Facebook / GitLab
plus username-or-email. Password-only login (`m.login.password`) and
self-registration don't exist for such accounts, so today they can't use the
app at all.

The fix is a client-side **OAuth 2.0 authorization-code + PKCE** flow against
the homeserver's OIDC provider (MAS), which matrix-sdk 0.18 already implements
(`client.oauth()`). Any OIDC-native homeserver gets the SSO button; legacy
homeservers (local dev Synapse) keep username/password + register.

**Related:** `docs/plan/qr-join.md` (same startup-hash hook pattern), `PLAN.md`
Comms section. Pre-release: no back-compat; localStorage is disposable.

## Why this approach

- matrix-sdk 0.18 `OAuth` (client-side): `server_metadata()`, `register_client()`
  (dynamic client registration — MAS supports it, public client, no secret),
  `login(redirect_uri, …)` → authorization URL, `finish_login(url_or_query)`
  → code exchange + Matrix grant, `full_session()`/`user_session()` for
  persistence, `refresh_access_token()` for token expiry.
- All pure HTTP to the homeserver → works in WASM with the pinned `getrandom`
  wasm backends. No extra Cargo features needed.
- `MatrixAuth::login_username`/`register` stay untouched for legacy HSs.
- **Not used:** the `sso-login` feature — it spawns a local HTTP server to
  catch the redirect, which can't exist on static hosting. Likewise the legacy
  `m.login.sso` webview dance and email magic-link are not offered by MAS.

## Flow (new-tab + BroadcastChannel handoff)

1. **Detect** (`page/home.rs`): for the entered homeserver,
   `oauth.server_metadata().await`; if `error.is_not_supported()` the server
   is password-only and the SSO button stays hidden/disabled. matrix.org
   qualifies; `http://localhost:8008` (dev Synapse) doesn't.
2. **Start** (`sync.rs` + `services/matrix.rs`):
   `oauth.register_client(metadata)` (cache `client_id` per homeserver in
   localStorage for the next login), then
   `oauth.login(redirect_uri, None, Some(reg_data), None).build()` where
   `redirect_uri` = `location.origin + location.pathname` (exact match —
   MAS validates it). `window.open(auth_url)` from the click handler (user
   gesture → no popup blocker). The new tab (B) shows MAS's own login page;
   the user picks Google / Apple / …
3. **Callback** (`main.rs`): MAS redirects tab B to
   `redirect_uri?code=…&state=…`. The app boots in tab B, and — *before*
   `resume_on_load` — detects the OAuth callback in the URL query (mirroring
   the QR-join hook), posts `{code, state}` to tab A over a `BroadcastChannel`
   (channel name derived from `state`), then closes itself / shows
   "return to the original tab". The callback tab must short-circuit
   `resume_on_load`.
4. **Finish** (tab A): receives the callback, calls
   `oauth.finish_login(url_or_query)`, persists the session, then proceeds
   exactly like today's successful `Connect` (identity, join event room, start
   sync, backfill).

Same-tab redirect is an alternative if matrix-sdk is later confirmed to persist
in-progress OAuth state across reload — the new-tab handoff doesn't depend on
that and is deterministic, so it's the default.

## Session storage (`services/matrix.rs`)

`StoredSession` today is password-auth only (`AuthSession::Matrix` at
`matrix.rs:110`). Add an OAuth kind:

```rust
pub struct StoredSession {
    pub homeserver: String,
    pub user_id: String,
    pub kind: StoredAuth,          // Matrix { device_id, access_token, refresh_token }
                                   // | OAuth(OAuthSessionJson)
}
```

- `save_session` branches on `client.session()`:
  `AuthSession::Matrix(ms)` → as today; `AuthSession::OAuth(os)` → persist the
  serialized `OAuthSession` (`oauth.full_session()`).
- `restore_session`/`resume_on_load` branch on the stored kind:
  `oauth.restore_session(session, RoomLoadSettings::default())` vs
  `matrix_auth().restore_session(…)`.
- Token refresh: build the client with `ClientBuilder::handle_refresh_tokens()`
  so OAuth tokens refresh automatically, or call `oauth.refresh_access_token()`.

## UI (`page/home.rs`)

A **"Sign in with SSO (Google…)"** button beside the password form, shown when
the entered homeserver advertises OIDC. `ConnState` gains an SSO-in-progress
state so the button shows "Waiting for browser tab…" and stays disabled until
`finish_login` lands. Password/Register unchanged and always available.

## Files

- `services/matrix.rs` — `is_oidc(hs)`, `start_oauth_login(hs)` (detect →
  register → build URL → persist client_id), `finish_oauth_login(url)`,
  session save/load branching, client builder refresh-token option.
- `sync.rs` — `Msg::SsoStart` / `Msg::SsoFinish` handlers + the BroadcastChannel
  subscribe/relay plumbing (channel per `state`, storage-event or
  BroadcastChannel listener on tab A).
- `main.rs` — `oauth_callback()` startup hook (parse `?code&state`, publish to
  channel, close tab; return before `resume_on_load`).
- `app.rs` — `Msg` variants + `ConnState::SsoPending`; `page/home.rs` — SSO
  button + status text.
- Docs: this file; AGENTS.md src tree (already stale — services/matrix.rs is
  missing; fix while here).

## Task list

- [ ] `services/matrix.rs`: `is_oidc` / `start_oauth_login` / `finish_oauth_login`;
      `StoredSession` auth-kind branching (save/load/restore); client builder
      refresh-token flag.
- [ ] `sync.rs`: SSO start (register + build + open tab + subscribe channel) and
      finish (relay → `finish_login` → identity/join/sync, same tail as
      `connect()`).
- [ ] `main.rs`: `oauth_callback()` before `resume_on_load`; consume the query
      (replaceState) so a refresh doesn't re-broadcast.
- [ ] `app.rs` / `page/home.rs`: `Msg::Conn(…SsoStart/SsoFinish)` + SSO button +
      `ConnState::SsoPending`.
- [ ] Session kind round-trip test (pure, native test of save/load JSON).
- [ ] `./scripts/check.sh` green (fmt + clippy `--all-targets` + tests).
- [ ] Manual matrix.org test (below).
- [ ] Docs: AGENTS.md tree + PLAN.md Related docs.

## Gotchas

- **Popup blockers**: `window.open` must come from the button click handler,
  not a later async continuation (build the URL first, then open).
- **redirect_uri exact match**: origin + pathname only (no query, no hash);
  must equal the metadata registered with MAS.
- **Callback tab**: must not run `resume_on_load` (short-circuit on the query
  params); consume the query via `history.replaceState` so a reload is clean.
- **Channel keyed by `state`**: only the originating tab matches, so a stale or
  foreign callback can't hijack an unrelated login.
- **Mixed content**: matrix.org is HTTPS → fine from GitHub Pages. LAN dev stays
  password-based (dev Synapse has no OIDC).
- **MAS registration policy**: matrix.org's MAS refuses dynamic client
  registration whose `client_uri`/`redirect_uris` are **http or on localhost**
  (its `client_registration.rego` enforces https + non-localhost hosts).  SSO
  therefore only works from a real https origin — the deployed GitHub Pages
  app, the local `scripts/serve.sh` (trunk over TLS on
  `dev.localhost`), or a https tunnel.  `sso_login` pre-empts this
  with a clear message instead of surfacing the raw 400.
- **Concurrent edits**: another instance edits this repo; re-read files before
  touching.

## Manual matrix.org test

1. Serve over https: `scripts/serve.sh` (auto-generated cert, accept the browser
   warning once) and open `https://dev.localhost:8080`, or deploy to GitHub Pages
   (SSO needs a real https origin — matrix.org rejects http/localhost redirects).
   Sign in form:
   enter `https://matrix.org` (the "Use Matrix.org" button prefills it), tap
   "SSO sign-in".
2. New tab opens at MAS; pick **Google**, grant access.
3. Tab returns to the app URL with `?code&state`; original tab completes,
   shows the user logged in; Results/sync work.
4. Refresh → session resumes via `restore_session` (OAuth kind).
5. Same flow against a self-hosted MAS server if available; `localhost:8008`
   still shows password/register only.

## Follow-ups (not in scope)

- QR join links can carry an `hs` whose SSO button then pre-fills the
  homeserver (trivial once this lands).
- Element-style "sign in with QR code" (matrix-sdk `login_with_qr_code`,
  needs `e2e-encryption` feature) as a phone-to-phone bootstrap.