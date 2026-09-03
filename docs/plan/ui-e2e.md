# UI end-to-end / regression testing

> Status: **pre-release must** — unit + `wasm-bindgen-test` do not cover
> click flows, presentation, or cross-page state. Regressions keep landing on
> officials (focus steal, hidden timing buttons, TBA gates, nav/state
> hangover). Add a real browser E2E layer before practice/prod harden.
>
> Related: `AGENTS.md` (test layers), `scripts/check.sh`, `scripts/wasm-test.sh`,
> `PLAN.md` Things to sort.

## What we already have (and what it misses)

| Layer | Tool | Covers | Misses |
|-------|------|--------|--------|
| Native unit | `cargo test` | Pure domain (`event`/`batch`/`replay`/`qr`/…) | DOM, clicks, CSS, signals |
| Wasm unit | `./scripts/wasm-test.sh` (`wasm-bindgen-test`) | localStorage / DOM helpers / Matrix constructors in headless FF | Multi-step user journeys, layout, “button gone”, focus |
| Manual | `test-me-please` + `serve.sh` | Human judgement | Not repeatable; doesn’t gate CI |

**Gap:** regressions in **user click flows** and **presentation/state** across
Timing → Results → Event → Home. That needs driving a real browser against a
built Trunk app.

---

## Recommendation: Playwright against Trunk

**Playwright** (TypeScript test package in-repo) is the practical choice:

- Click / type / navigate with auto-wait; mobile + desktop projects
- Traces + screenshots on failure (debug “what did the official see?”)
- Works with any static/WASM app — no Sycamore-specific harness required
- CI-standard; can gate `check` / deploy the same way wasm tests do

Alternatives considered:

| Option | Verdict |
|--------|---------|
| Expand `wasm-bindgen-test` only | Good for helpers; painful for multi-page flows and visuals |
| Cypress | Fine, but heavier; Playwright is enough |
| `playwright-rs` / Probar | Keep stack in Rust, but TS Playwright is more mature for locators/CI today — revisit if we want zero Node |
| Manual-only | Already failing us |

**Node is a test-only dep** (not a runtime dep of the WASM app). Pin via
`package.json` at repo root or under `e2e/`.

---

## Architecture

```
e2e/
  package.json          # @playwright/test
  playwright.config.ts  # baseURL, projects (chromium desktop + pixel-ish mobile)
  tests/
    demo-timing.spec.ts
    results-state.spec.ts
    nav-event-switch.spec.ts
    …
  fixtures/             # optional seeded localStorage JSON
scripts/e2e.sh          # build (or reuse dist) + playwright test
```

**Serve target:** `trunk build` (or `trunk serve` with fixed port) of the
worktree under test. Prefer **release-shaped build** for CI; debug build OK
locally for speed.

**Isolation:** each test starts with cleared `localStorage` / `sessionStorage`
(Playwright `context.addInitScript` or `page.goto` + clear), then seeds Demo
or a fixture event so runs don’t depend on leftover HQ state.

**Selectors:** add stable hooks in the Rust UI — prefer
`data-testid="…"` (or `aria-label` where it’s real a11y) on critical controls
(Start/Stop/Manual, car chips, Confirm, Results tabs, burger items). Do **not**
rely on Bulma class soup or visible copy that changes often.

**Matrix / homeserver:** first slice is **offline / Demo / local-only** flows
(no Synapse). Publish/join/QR paths come next with podman Synapse or mocked
responses — don’t block the framework on full Matrix.

---

## Priority flows (write these first)

Order = highest past regression density:

1. **Demo stopwatch happy path** — open Demo → Timing → pick stage → select car
   → Start → Stop → Confirm → Log shows finish; Results updates.
2. **Manual / TBA** — `?` car requires comment; Manual disabled until comment;
   confirm clears staging (B11/B14 class bugs).
3. **Provisional / edit focus** — open edit; 1s tick must not steal focus or
   hide Start/Stop (B7/B12).
4. **Event switch state** — time in event A, switch/open event B; car/comment/
   stage must not hang over (B13).
5. **Nav shell** — burger + Timing hub stage picker; mobile viewport.
6. **Results modes** — live vs official toggle still renders rows.
7. **Later:** publish + QR join against local Synapse; handoff parcel import
   (camera optional / paste path).

Each spec asserts **visible state** (buttons present/absent, text in log,
results row count), not only “no console error”.

---

## CI / gates

- `scripts/e2e.sh` — install browsers once, run Playwright.
- Wire into `scripts/check.sh` **or** a dedicated CI job that still **gates
  deploy** (same bar as check). Start as non-blocking if flaky, then harden.
- Artefacts: Playwright HTML report + trace zip on failure.
- Mobile project: one Chromium device descriptor (e.g. Pixel 5) mandatory for
  Timing/Results specs.

---

## App changes needed for testability

- [ ] `data-testid` (or equivalent) on critical controls listed above
- [ ] Optional `?e2e=1` or build flag to disable animations / confirm slow
      paths if they flake (only if needed — prefer fixing waits)
- [ ] Document “how to run E2E” in `AGENTS.md` Commands

Avoid a wide test-only API on `window` unless a flow is otherwise unobservable
(e.g. reading signed outbox). Prefer DOM assertions.

---

## Checklist

### Scaffold (do early — unblocks writing specs while features land)

- [ ] `e2e/` Playwright project + `playwright.config.ts` (desktop + mobile)
- [ ] `scripts/e2e.sh` (build/serve + test)
- [ ] CI job / `check.sh` hook (gate deploy once green)
- [ ] First `data-testid` pass on Timing + Results + navbar
- [ ] Specs 1–4 above green locally
- [ ] AGENTS.md: document `./scripts/e2e.sh` next to wasm-test

### Harden (before / during first official release)

- [ ] Specs 5–6; screenshot assertions on key Timing/Results screens (optional
      but useful for presentation regressions)
- [ ] Synapse-backed publish/join smoke (Level 1 invite path)
- [ ] Failures upload traces; flake budget ~0 on the core four flows
- [ ] Practice-day checklist: run E2E on the **pinned** `/vX.Y.Z/` build

---

## Non-goals (for now)

- Full visual regression suite across every Bulma theme breakpoint
- Replacing `cargo test` / wasm-bindgen-test
- Driving real device cameras for QR (use paste / injected barcode fixtures)
