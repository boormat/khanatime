# Khana Timing project

[![test](https://github.com/boormat/khanatime/actions/workflows/test.yml/badge.svg)](https://github.com/boormat/khanatime/actions/workflows/test.yml)

Live App/Web Install: https://boormat.github.io/khanatime/

Rust WASM client-side app:
- [Sycamore](https://sycamore-rs.github.io/) for the reactive UI
- [Trunk](https://trunk.dev) for the WASM build
- Serverless — offline-first via localStorage, with Matrix sync planned
  (see `PLAN.md`)

# Tech decisions

Rust + Sycamore
Sycamore is a fine-grained reactive framework. Big maintenance saving is no
JavaScript errors to deal with. When you change something the compiler will fail
or warn you of unhandled events/messages etc. Way faster than tracing state in
console.

Trunk for WASM build. It just seems to work, unlike wasm-pack that had
dependency problems. Styling with Bulma library. Fontawesome for icons.

Write it as a single WASM app.

Pages:
- home menu/event picker
- Results view. Just render
- Event/Scorer. Central data entry for classes, entrants, stage times
- Stage. Time entry for stage officials

Data Store:
Use local storage to operate offline. Offline sharing of documents as primary
option (Matrix room as store-and-forward, see `PLAN.md`).
 - Stage times as a doc. List of times for that stage. Basically the primary
   (maybe consider a stage commander to resolve)
 - Event as a doc. Owned by Scorer. They import stage times and/or manually
   enter from paper.
 - Published results? Publish to web? PDF + in the json form.

The data flow from time entry to scorer to published results is a 3 step
process, not trying to publish results before they are approved.

Note: trunk needs inline mode for stylesheets. data-inline

## Install / check required tools

1. Make sure you have basic tools installed:

  - [Rust](https://www.rust-lang.org)
  - `rustup target add wasm32-unknown-unknown`
  - cargo install --locked trunk

Once you've installed Trunk, simply execute `trunk serve --open` from this
example's directory, and you should see the web application rendered in your
browser.

## Development

```bash
# Dev server with hot reload -> http://localhost:8080
trunk serve --open

# Release build (outputs to dist/)
trunk build

# Unit tests (native target, no wasm needed)
cargo test
```

Notes for devs:

- This is a browser-only WASM app. The supported build is `trunk build`/`trunk serve`.
- `cargo build` / `cargo test` (native) also work and are pure Rust — no C
  compiler required. `matrix-sdk` (planned Matrix sync) is gated to the wasm
  target only, so it doesn't drag `rustls`/`aws-lc-sys` (a C crate) into native
  builds. If you ever move it back to a shared dependency, native builds need a
  working C compiler (note: `zig cc`'s bundled clang rejects the
  `x86_64-unknown-linux-gnu` target triple used by `aws-lc-sys` — use real `gcc`).

## Testing and Deploy

Currently manual triggered release build in github workers.
https://github.com/boormat/khanatime/actions
Run the deploy workflow to update  https://boormat.github.io/khanatime/

When testing locally with trunk and chrome, the WebWorkers seem to mess
up when the reload comes from trunk, and you get a blank screen.
A workaround is developer tools -> Service Workers -> Update on Reload On
