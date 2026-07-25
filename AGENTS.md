@RTK.md

# The Necessary Work — Agent Guide

A single-player real-time incremental strategy game about coordinating global
decarbonisation through the fictional Common Future Authority. The first
prototype is a counterfactual 1990 with no loss state, built to answer one
question: is the core economy satisfying over a 20–30 minute run? (It is not a
claim that rapid transition would have been simple — the README carries the
full framing.)

## Tech stack

| Layer | Technology |
|---|---|
| Simulation core | Rust, `crates/nw-simulation` — pure, deterministic, no Bevy |
| Content | RON under `content/`, `include_str!`-embedded; schema validation + content hash in `crates/nw-content` |
| Persistence | `crates/nw-persistence` — RON records, periodic state digests, replay validation |
| Headless tools | `crates/nw-headless` (bin `nw`) — bot runs, replay validation, corpus batches |
| Client | Bevy 0.16, `crates/nw-client` — non-authoritative view; every number it shows comes from sim state, calc traces, and previews |
| Architecture model | PASM — YAML spec under `pasm/spec/`, tool pinned from vellum |
| Shared crates | vellum-digest, vellum-rng, vellum-corpus (see `vellum-adoption` in decisions.yaml — vellum-replay deliberately not adopted) |
| CI | fleet-ci caller (`.github/workflows/ci.yml`) → pasm gates, clippy `-D warnings`, tests, Trunk build, Pages deploy |

## Project rules

- The simulation is pure Rust: content and simulation never depend on the
  client, and Bevy never reaches the simulation.
- Every player decision enters the sim as a semantic command that can be
  rejected; pause/resume go through the same path. The UI never re-derives a
  number — calc traces and previews exist so it doesn't have to.
- Content is embedded at compile time so native, wasm, and CI ship
  byte-identical data; `content_version` hashes the authored bytes.
- Read and update `pasm/spec/` before or alongside every structural change;
  record accepted choices in `pasm/spec/core/decisions.yaml`.

## PASM — keep it up to date

1. Model first, then build — spec entities before Rust for a new system.
2. Record decisions in `pasm/spec/core/decisions.yaml` as you make them.
3. `uv run pasm validate pasm/spec` after any model change; fix before commit.
4. `uv run pasm scan pasm/spec --json` gates CI — keep implementation
   mappings current.
5. Never leave dead spec — removing a system updates its declarations.

## Common commands

```bash
# CI gates — run all of these before calling work done
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
uv run pasm validate pasm/spec

# Run the game (NW_SEED fixes the seed; NW_SMOKE_SECS / NW_SHOT for CI boots)
cargo run -p nw-client

# Headless tools
cargo run -p nw-headless -- run --seed 1 --strategy deploy-first
cargo run -p nw-headless -- batch --count 4      # corpus: both bots must win
cargo run -p nw-headless -- validate <record.ron>

# Web build
trunk serve                        # http://localhost:8080
trunk build --release              # CI ships this with --public-url /necessary-work/
```

## Vellum — the shared foundation

This repo pins vellum by rev in Cargo.toml (digest/rng/corpus),
pyproject.toml (pasm), and the `uses:` line of `.github/workflows/ci.yml`. A
vellum bump PR aligns all of them and touches nothing else. Local override
etiquette: vellum `docs/handbook/local-dev.md` — never committed active.
