# Tokmesh

Local analytics for AI coding token usage: scan session logs, price models, run reports, and explore everything in a full TUI.

Tokmesh is a hard fork of [tokscale](https://github.com/junhoyeo/tokscale). It keeps the local scanner, pricing system, reports, and terminal UI, and can submit daily aggregates to either public leaderboard:

- [tokscale.ai](https://tokscale.ai)
- [tokens.ci](https://tokens.ci)

## Build

```sh
cargo build --release -p tokmesh-cli
./target/release/tokmesh --help
```

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

Data is read **only** from local client logs and databases (Claude Code, Codex, OpenCode, Grok Build, Cursor cache, and many others).

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

Only aggregated usage leaves the machine on submit (same privacy model as upstream). Use `--dry-run` to print what would be uploaded without contacting a server.

## TODO

- Optional self-hosted private cloud for detailed personal history (not started)

## License

MIT. Includes work from [tokscale](https://github.com/junhoyeo/tokscale) (Copyright (c) 2025 Junho Yeo) and [tokens.ci](https://github.com/missuo/tokens) (Copyright (c) 2026 Vincent Yang). See [LICENSE](LICENSE).
