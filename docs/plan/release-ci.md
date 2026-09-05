# Release CI / deployment + in-app updates

> Status: **implementing.** Operator flow, Pages layout, and update UX for
> pinned app releases. Product rules live with `docs/plan/release-versioning.md`.

## Semver (event policy)

| Part | Meaning | Change during a live event? |
|------|---------|------------------------------|
| **major** | Breaking changes, major reorgs | **Never** |
| **minor** | New features, screen changes, wire-format changes | **Never** |
| **patch** | Hotfix for a problem being experienced | **Yes** — organiser/timekeeper may move HQ (and re-issue QR if bumping the event pin) |

Officials talk **major.minor** (“we’re on 0.2”). Patch notes must be short and
operational so a timekeeper can decide whether to open `/v0.2.1/`.

## Operator flow

1. PR → `main` (`test.yml` / `check.sh` green).
2. Bump `Cargo.toml` version + `CHANGELOG.md` entry on `main`.
3. Tag matching Cargo: `git tag vX.Y.Z && git push origin vX.Y.Z`.
4. `release.yml`: assert tag == Cargo.toml → check → build → publish
   `/khanatime/vX.Y.Z/` + update latest root + `releases.json` + GitHub Release.

No retagging the same `vX.Y.Z`. Hotfix = new patch.

## Channels

| Channel | Trigger | URL | `app_version` |
|---------|---------|-----|---------------|
| Stable | tag `vX.Y.Z` | `/khanatime/vX.Y.Z/` | `X.Y.Z` |
| Latest stable | after tag | `/khanatime/` | newest tag |
| Preview | push `main` / dispatch | `/khanatime/main/` | `dev-<sha>` |
| Local | `serve.sh` | localhost | `dev-<sha>` or Cargo |

Invite QRs use the **pinned** `/vX.Y.Z/` URL only — never floating `/` or `/main/`.

## `releases.json`

Published at `https://boormat.github.io/khanatime/releases.json` on each
stable release. The app fetches it to prompt about newer versions and to warn
before publish/create on an older build.

## Workflows

- `test.yml` — PR/push → `check.sh`
- `release.yml` — tag `v*` → version assert + check + versioned Pages + Release
- `preview.yml` — `main` → `/khanatime/main/` only (does not overwrite `/v*`)
- `deploy.yml` — retired as main-overwrite; superseded by release/preview

Pages site is assembled cumulatively (existing `gh-pages` tree + new `vX.Y.Z/`
+ root latest) then uploaded via `deploy-pages`, and the tree is pushed back to
`gh-pages` for the next run.
