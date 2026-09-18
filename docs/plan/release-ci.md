# Release CI / deployment + in-app updates

> Status: **live.** Cumulative site on the **`gh-pages` branch**; GitHub Pages
> must be set to **Deploy from a branch → `gh-pages` / (root)**.

## Semver (event policy)

| Part | Meaning | Change during a live event? |
|------|---------|------------------------------|
| **major** | Breaking changes, major reorgs | **Never** |
| **minor** | New features, screen changes, wire-format changes | **Never** |
| **patch** | Hotfix for a problem being experienced | **Yes** |

## Operator flow

### Dev / prerelease

```
feature → PR → main
              ↓
         test.yml + preview.yml
              ↓
         gh-pages: /main/ (dev-<sha>) + catalog index
```

Testers: `https://boormat.github.io/khanatime/main/`

### Stable release

```
1. main green (test + preview)
2. Bump Cargo.toml + CHANGELOG on main
3. git tag vX.Y.Z && git push origin vX.Y.Z
4. release.yml → gh-pages: /vX.Y.Z/ + /latest/ + releases.json + catalog
5. Verify live URLs, then regenerate invite QR from /vX.Y.Z/
```

Re-publish without retagging: Actions → **release** → Run workflow → tag `v0.2.1`.

**Always verify** after a green run (do not trust the checkmark alone):

- Catalog lists the version: `https://boormat.github.io/khanatime/`
- App: `…/vX.Y.Z/`
- Manifest: `…/releases.json`

## Channels

| Channel | URL | Notes |
|---------|-----|--------|
| Catalog | `/khanatime/` | HTML index (not the WASM app) |
| Stable | `/khanatime/vX.Y.Z/` | Invite QR base |
| Latest alias | `/khanatime/latest/` | Same bits as newest tag |
| Preview | `/khanatime/main/` | `dev-<sha>` |

Invite QRs: **`/vX.Y.Z/` only**.

## Pages settings (required)

**Settings → Pages → Build and deployment → Source:**  
**Deploy from a branch** → **`gh-pages`** → **/ (root)**.

Do **not** use “GitHub Actions” as the Pages source with these workflows.
Publishing only via `peaceiris` → `gh-pages`. Using both `deploy-pages` and
`peaceiris` caused a split brain: branch had v0.2.1 while the live Actions
artifact stayed on preview.

## Workflows

- `test.yml` — PR/push → `check.sh`
- `preview.yml` — `main` → update `/main/` on `gh-pages`
- `release.yml` — tag `v*` or workflow_dispatch → `/vX.Y.Z/` + `/latest/`
- `deploy.yml` — retired notice only
