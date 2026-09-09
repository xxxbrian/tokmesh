# Upstream Sync: 2026-09-08

This update extends PR #2 as one integration PR. It does not release tokmesh or merge the PR.

## Reviewed Tips

- tokscale: `15516420f2b106750760f6e182559899f814e2dc`, reviewing the changes after `e80db478af11408c6dd3436e40dee9e7c0d84a23` plus local prerequisites missing from the earlier port.
- tokens: `f114057f33c64b3e86539aa56d5dfde9964ae8e7`, after `75aba190695c9de5f2695d9baba6afd3c8cb63f8`.
- Leaderboard protocol versions remain `4.7.0` and `27.0.1`. A reviewed commit is not a claim that every upstream file was copied.

## Adopted

- Retain the original PR's LM Studio, Unsloth and Command Code v3 parsers, Cursor conversation grouping, OpenCode incremental scans, Senpi discovery, Windows Copilot credentials and xAI long-context pricing.
- OpenClaw per-agent SQLite stores, WAL invalidation, compressed and import archives, checkpoint exclusion, and per-turn Codex app-server ownership and mirror deduplication. Both cached and direct local parsing are covered.
- Antigravity CLI model recovery and gen9 timestamp decoding, including competing-endianness rejection. Antigravity quota discovery uses a bounded concurrent round and keeps a slow language server eligible. RPC execution is isolated from callers' Tokio runtimes, larger trajectory responses are bounded, and cached message timestamps survive enrichment failure.
- Single-pass Copilot CLI export parsing.
- Kimi Code workspace indexes, cross-client Projects tab, disambiguated workspace labels, worktree path resolution, CJK-width-safe table borders and model truncation, and persisted light-theme selection.
- GPT-6 Astra/Pro request-wide 272k pricing and OpenRouter author endpoint selection by Standard/Flex/Fast/Batch tier. Stealth preview aliases and first-party archived Zhipu/Xiaomi/Tencent rates are included without adopting the upstream submission-evidence protocol.
- Shared parser versions derive from their format owners. Codex incremental coverage changes invalidate old bincode shards. MiMo and fx continue to work in direct reports; provider-reported costs survive the ParsedMessage path.

## Integration Fixes

- Keep the old Cursor credential file if migration to the new location fails.
- Use an exact Fast model tariff before falling back to the base model; report/submit identity normalization is unchanged.
- Re-read OpenCode rows at the inclusive millisecond boundary even when their timestamp equals the cached marker.
- Recover LM Studio response IDs despite nested tool-call or output-item IDs.
- Avoid duplicate fx parsing when integrating an upstream entry point tokmesh already had.
- Update tests to distinguish a missing price from a provider-reported zero price.

## Intentionally Not Adopted

- Hindsight remote synchronization and its mirror-dependent client registration.
- Upstream website, moderation, database migrations, publication workflows and leaderboard server changes. In particular, the latest Droid snapshot reconciliation commit changes only the upstream web service.
- Native TLS, remote parser high-water, costIsComplete, and streaming submission folding. Tokmesh keeps rustls and its independent dual-leaderboard contracts.
- Grok unified.jsonl dual-source parsing, MiniMax report summarization, and upstream branding.
- The upstream Rust 1.98 pin: this port is verified on Rust 1.94.0 and CI uses that same compiler.

## Verification

Run `cargo fmt --all --check`, `cargo test --workspace --locked`, and `cargo doc --workspace --lib --locked --no-deps`. Documentation targets libraries because the workspace contains two binaries named `tokmesh`.

The added CI tests Linux/macOS, builds Windows, and runs focused Windows credential and process-liveness tests. Local tests include temporary databases and local HTTP servers; no real leaderboard upload or release is performed. Tests explicitly ignored by the repository remain opt-in.
