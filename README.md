# Tokmesh

Local analytics for AI coding token usage: scan session logs, price models, run reports, and explore everything in a full TUI.

Tokmesh is a hard fork of [tokscale](https://github.com/junhoyeo/tokscale). It keeps the local scanner, pricing system, reports, and terminal UI, and can submit daily aggregates to either public leaderboard:

- [tokscale.ai](https://tokscale.ai)
- [tokens.ci](https://tokens.ci)

## Install

### From source

```sh
cargo install --git https://github.com/xxxbrian/tokmesh --locked tokmesh
# or, in a checkout:
cargo build --release -p tokmesh
./target/release/tokmesh --help
```

The installable package name is `tokmesh` (short crates.io / cargo name). Implementation lives in `tokmesh-cli`; the `tokmesh` crate is a thin wrapper that produces the same `tokmesh` binary.

### mise (GitHub Releases)

After a `v*` tag release exists:

```sh
mise use -g github:xxxbrian/tokmesh
tokmesh --help
```

Release assets are named `tokmesh-{version}-{rust-target}.tar.gz` (`.zip` on Windows) with a `tokmesh` binary inside, so mise can auto-select your platform.

### GitHub Releases

Download the archive for your target from the [Releases](https://github.com/xxxbrian/tokmesh/releases) page, extract, and put `tokmesh` on your `PATH`.

## Local usage

```sh
tokmesh              # interactive TUI (default)
tokmesh tui
tokmesh models
tokmesh monthly
tokmesh hourly
tokmesh clients
tokmesh pricing <model>
tokmesh graph --json
tokmesh time-metrics
tokmesh wrapped
```

Config and caches live under `~/.config/tokmesh/` (override with `TOKMESH_CONFIG_DIR` for an isolated profile).

Usage is derived from local client logs and databases (Claude Code, Codex, OpenCode, Grok Build, Cursor cache, and many others). The TUI may also fetch public leaderboard statistics.

### Model identity

Local reports and leaderboard submit share one model-id path. Two narrow suffix rewrites:

| Source | Rule |
|--------|------|
| OpenCode | `gpt-…-fast` → base model (e.g. `gpt-5.6-sol-fast` → `gpt-5.6-sol`) |
| Grok | `grok-…-build` → base model (e.g. `grok-4.5-build` → `grok-4.5`) |

## Leaderboards

```sh
tokmesh tokscale login
tokmesh tokscale submit
tokmesh tokscale submit --dry-run
tokmesh tokscale whoami
tokmesh tokscale logout

tokmesh tokensci login
tokmesh tokensci submit
tokmesh tokensci whoami
tokmesh tokensci logout
```

Credentials are stored separately and never mixed:

- `~/.config/tokmesh/tokscale/credentials.json`
- `~/.config/tokmesh/tokensci/credentials.json`

Environment overrides (optional):

| Variable | Purpose |
|----------|---------|
| `TOKMESH_TOKSCALE_API_TOKEN` | tokscale.ai API token |
| `TOKMESH_TOKENSCI_API_TOKEN` | tokens.ci API token |
| `TOKMESH_TOKSCALE_API_URL` | Override tokscale API base (tests/mocks) |
| `TOKMESH_TOKENSCI_API_URL` | Override tokens.ci API base (tests/mocks) |

Submit sends daily usage aggregates and a stable device identifier. It never sends prompts, completions, file contents, or local paths.

```sh
tokmesh tokscale submit --dry-run          # human summary
tokmesh tokensci submit --dry-run --json   # full JSON body (no upload)
```

`tokmesh tokensci submit --replace --client <id> --since YYYY-MM-DD --until YYYY-MM-DD`
authoritatively replaces that client's bounded date range; missing local days are removed remotely.

## TODO

- Optional self-hosted private cloud for detailed personal history (not started)

## License

MIT. Includes work from [tokscale](https://github.com/junhoyeo/tokscale) (Copyright (c) 2025 Junho Yeo) and [tokens.ci](https://github.com/missuo/tokens) (Copyright (c) 2026 Vincent Yang). See [LICENSE](LICENSE).
