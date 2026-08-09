# Agents Guide

> Khanatime — grassroots khanacross timing app (Rust WASM + Sycamore + Matrix).
> Successor to the archived `khanatime26` Flutter prototype; design history in
> `docs/research/` and the current plan in `PLAN.md`.

## Commands

```bash
# Dev server (Trunk, auto-reload on save)
trunk serve

# Release build
trunk build --release

# Tests
cargo test

# Format
cargo fmt

# Lint (CI has this commented out, run locally)
cargo clippy --all-features
```

Deploy is a manual GitHub Actions workflow (`deploy.yml`) that updates
https://boormat.github.io/khanatime/ — run it from the Actions tab.

## Framework notes

- **Sycamore 0.9** reactive framework.
  State via `Signal<T>`; views via `view! { ... }` macro.
- **Trunk** for the WASM build (see `Trunk.toml`); styling is SASS/Bulma in
  `styles/`, FontAwesome icons.
- **Elm-style architecture:** central `Model` + `Msg` enum + `update()` in
  `src/main.rs`. Each page owns a sub-model and sub-msg; page updates are
  dispatched from `main.rs` via `Msg::XxxMsg(...)`.
- **No server.** Client is WASM only; sync is Matrix (single shared room per
  event). `matrix-sdk` 0.18 (features `js`, `indexeddb`) is already a
  dependency. `getrandom` 0.3/0.4 `wasm_js` backends are pinned for
  wasm32-unknown-unknown — keep those if you touch `Cargo.toml`.

## Architecture

```
src/
├── main.rs             # Page enum, Model, Msg, update(), navbar, render
├── lib.rs              # log! macro + web_log (console.log)
├── view.rs             # small view helpers
├── event.rs            # EventInfo, Entry, ScoreData, KTime, KTimeTime
│                       # + localStorage load/save (event + stage times)
├── input.rs            # keyboard/input helpers
└── page/
    ├── home.rs         # menu / event picker
    ├── event.rs        # event setup (entries, classes, stages)
    ├── stage.rs        # TIMER — command-line stopwatch entry
    │                   #   parse_command()/parse_car(), CmdParse, TimeCmd
    ├── results.rs      # results + score computation (ResultRow/ResultScore/Pos)
    ├── help.rs         # usage help
    └── khana_rule.rs   # rendered rules reference
```

### Domain model (see PLAN.md + docs/KhanacrossRules.md)

- `EventInfo { name, stages_count, classes, entries }`
- `Entry { car, name, vehicle, classes }` — car number is a string ("00 0B 24TBC")
- `ScoreData { stage, car, time }` — one record per stage per car
- `KTime` enum + `KTimeTime { time_ds, flags, garage }` — time stored in
  **deciseconds** (`time_ds`), plus flag penalties (count) and garage flag.

## Sync model (current direction)

- Single shared Matrix room per event named "timing"; room history replays as
  store-and-forward offline sync. See `docs/research/MessagingSpike.md` and the
  Comms section of `PLAN.md`.

## Conventions

- `cargo fmt` before finishing; match existing code style (short fns, terse
  comments).
- Keep domain logic in `event.rs` / `results.rs` (pure, testable), UI in
  `page/`.
- Don't add new server-side components; Matrix-only.
