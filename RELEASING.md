# Releasing tokmesh

How to cut a version: bump once, ship binaries on a tag, then manually publish
registries only after the GitHub Release is complete.

## Model

| Channel | What ships | Trigger |
|---------|------------|---------|
| **GitHub Release** | Prebuilt `tokmesh` for 8 targets (mise-friendly names) | Push tag `vX.Y.Z` → workflow **Release** |
| **crates.io** | Source: `tokmesh-core` → `tokmesh-cli` → `tokmesh` | Manual **Publish registries** |
| **npm** | Single package `tokmesh` (downloads binary from GH Release on install) | Manual **Publish registries** |
| **PyPI** | Platform wheels + sdist (binaries embedded from GH Release) | Manual **Publish registries** |

**Unified version:** all crates, npm, and PyPI use the same `X.Y.Z` from
`[workspace.package] version` (and packaging manifests kept in sync by the
bump script).

**Safety:** registries never auto-publish on tag. Tag only builds binaries.
Registry jobs refuse to run unless the matching GitHub Release has all assets.

Auth for registries is **Trusted Publishing (OIDC)** for crates.io, npm, and
PyPI. No long-lived registry tokens are required in CI when publishers are
configured for workflow `publish-registries.yml`.

---

## One-time setup (Trusted Publishing)

Do this once per registry (already done for 0.1.0 if you followed the web
checklist). Values must match exactly:

| Field | Value |
|-------|--------|
| GitHub owner | `xxxbrian` |
| Repository | `tokmesh` |
| Workflow file | `publish-registries.yml` |
| Environment | *(leave empty unless you add `environment:` to the job)* |

### crates.io

For **each** of `tokmesh-core`, `tokmesh-cli`, and `tokmesh`:

1. https://crates.io/crates/&lt;name&gt;/settings  
2. **Trusted Publishing** → add GitHub publisher with the table above.

### npm

1. https://www.npmjs.com/package/tokmesh → **Settings**  
2. **Trusted Publisher** → GitHub Actions → same table.

### PyPI

1. https://pypi.org/manage/project/tokmesh/settings/publishing/  
2. Add GitHub publisher → same table (project already exists after 0.1.0).

You can delete old GitHub secrets (`CARGO_REGISTRY_TOKEN`, `NPM_TOKEN`,
`PYPI_API_TOKEN`) if everything uses OIDC only.

---

## Every release (checklist)

Replace `0.2.0` with the new version.

### 1. Bump version on `main`

```sh
git checkout main
git pull origin main

python3 scripts/bump-version.py 0.2.0
# Updates:
#   - Cargo.toml [workspace.package] version
#   - path dep versions for tokmesh-core / tokmesh-cli
#   - packaging/npm/tokmesh/package.json
#   - packaging/pypi/pyproject.toml + __version__

git diff   # review
git commit -am "chore: release 0.2.0"
git push origin main
```

### 2. Tag and push (starts binary Release)

```sh
git tag -a v0.2.0 -m "v0.2.0"
git push origin v0.2.0
```

### 3. Wait for **Release** workflow

- Actions → **Release** (triggered by tag `v*`).
- Builds 8 targets, attaches:

  - `tokmesh-{version}-{rust-target}.tar.gz` (unix)
  - `tokmesh-{version}-{rust-target}.zip` (windows)
  - matching `.sha256` files

- Archive root is only the `tokmesh` / `tokmesh.exe` binary.
- CI also re-runs `scripts/bump-version.py` from the tag so `--version` matches
  even if the commit was slightly off.

**Do not publish registries until this is green and the GitHub Release page
lists all assets.**

Local check (optional):

```sh
scripts/verify-github-release.sh 0.2.0
```

### 4. Publish registries (manual)

Actions → **Publish registries** → **Run workflow**:

| Input | Typical value | Notes |
|-------|---------------|--------|
| `version` | `0.2.0` | or `v0.2.0` |
| `ref` | *(empty)* | defaults to tag `v0.2.0`; use `main` only if the tag is older than packaging code |
| `dry_run` | **`true` first** | package/verify only, no upload |
| `crates` | `true` | order: core → cli → tokmesh |
| `npm` | `true` if needed | single package `tokmesh` |
| `pypi` | `true` if needed | wheels from Release binaries |

1. Run once with **`dry_run=true`** — all selected jobs should succeed.  
2. Run again with the same inputs and **`dry_run=false`** to upload.

OIDC publishers must already be configured (see above). Failed auth usually
means workflow name / owner / repo / environment mismatch on the registry site.

---

## Workflows (reference)

| File | Name | When | Does |
|------|------|------|------|
| `.github/workflows/release.yml` | Release | `push` tags `v*` | Multi-platform binaries → GitHub Release only |
| `.github/workflows/publish-registries.yml` | Publish registries | `workflow_dispatch` only | Gate on complete Release, then crates / npm / pypi |

### Scripts

| Script | Role |
|--------|------|
| `scripts/bump-version.py` | Single source of truth bump for Cargo + npm + pypi |
| `scripts/verify-github-release.sh` | Assert all expected Release assets exist |
| `scripts/build-pypi-wheels.py` | Build PyPI wheels/sdist from GH Release assets (used by CI) |

---

## Install channels after a release

```sh
# Prebuilt (mise / GitHub)
mise use -g github:xxxbrian/tokmesh

# crates.io (compiles from source)
cargo install tokmesh --locked

# npm (wrapper; fetches matching Release binary)
npm install -g tokmesh

# PyPI
pipx install tokmesh
# or: uv tool install tokmesh
```

---

## Notes and pitfalls

- **Never** publish a new crates/npm/pypi version before the GitHub Release for
  that version is complete. npm and PyPI packages depend on those assets;
  yanking/replacing registry versions is painful.
- **crates.io** cannot reuse a version number. Bump semver for every publish.
- **Leaderboard `meta.version`:** submit reports the *upstream* package version
  pinned in `upstreams.lock` (`tokscale.version` / `tokens.version`), not
  tokmesh's own crate version. Local `graph` / `tokmesh --version` stay tokmesh.
- **crates.io workspace order:** publish `tokmesh-core`, then `tokmesh-cli`, then
  `tokmesh`. CI packages core first; cli/tokmesh only resolve on the index after
  their dependencies of the *same* version are already published.
- **npm / PyPI** same: once `0.2.0` is published, the next cut is `0.2.1`+.
- First-time local bootstrap (0.1.0) used interactive login/tokens; ongoing
  releases should use tag → Release → manual Publish registries + OIDC.
- Private repo would break public npm postinstall and anonymous Release
  downloads; keep the repo **public** for distribution.
- If `Publish registries` is run for a version whose tag predates packaging
  fixes, set `ref=main` (and still require Release assets for that version).

---

## Quick copy-paste (happy path)

```sh
VER=0.2.0
python3 scripts/bump-version.py "$VER"
git commit -am "chore: release $VER"
git push origin main
git tag -a "v$VER" -m "v$VER"
git push origin "v$VER"
# wait for Actions → Release to succeed
# then Actions → Publish registries:
#   version=$VER  dry_run=true  crates+npm+pypi
# then same with dry_run=false
```
