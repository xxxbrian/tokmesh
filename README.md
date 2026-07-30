# Tokmesh

Local analytics for AI coding token usage: scan session logs, price models, run reports, and explore everything in a full TUI.

Tokmesh is a hard fork of [tokscale](https://github.com/junhoyeo/tokscale). It keeps the local scanner, pricing system, reports, and terminal UI, and evolves independently.

## Build

```sh
cargo build --release -p tokmesh-cli
./target/release/tokmesh --help
```

## Usage

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

## TODO

- Optional self-hosted private cloud for detailed personal history (not started)

## License

MIT. Based on tokscale (Copyright (c) 2025 Junho Yeo).
