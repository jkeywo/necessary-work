# The Necessary Work

A single-player, real-time incremental strategy game about coordinating global
decarbonisation through the fictional **Common Future Authority (CFA)**. You
reduce global gross greenhouse-gas emissions to zero by financing and
sequencing programmes across three continents — balancing immediate emissions
cuts against the knowledge, infrastructure, workforce, and institutions that
make later cuts possible.

The first prototype runs a **counterfactual 1990** scenario. It has no loss
state: the run is always structurally recoverable, so the design question it
exists to answer is narrow and honest — *is the core economy satisfying over a
20–30 minute run?* The politically and educationally demanding 2020 scenario
(loss conditions, crises, legitimacy, carbon removal) is deliberately deferred
until that economy is proven.

> The 1990 counterfactual omits historical obstruction, crises, and political
> conflict to isolate the economy. It is **not** a claim that rapid transition
> would have been simple, and completion time is a replay metric, not a moral
> grade.

## Status

The Stage 1 deterministic core is implemented and tested: the full tick loop,
semantic command boundary, controlled effect vocabulary, project lifecycle,
planning queue, programme-slot milestones, opportunities, calculation traces,
projected previews, and command-log replay validation, driving a 13-project
authored catalogue. Two seeded bot strategies (deploy-first and capacity-first)
both reach victory in ~20–23 minutes of simulated time at the authored speed,
and every run's record replay-validates. What remains of Stage 1 is the Bevy
dashboard spike and structured human playtests — see
[`pasm/spec/roadmap`](pasm/spec/roadmap). The recorded design lives in
[`pasm/spec/core`](pasm/spec/core); the source design pack in
[`docs/design`](docs/design).

## Repository layout

```
crates/
  nw-content/       Authored content: schemas, loading, validation (RON)
  nw-simulation/    Deterministic, pure-Rust simulation core (no Bevy)
  nw-persistence/   Snapshots, command logs, records, replay validation
  nw-headless/      `nw` — replay validation, seeded bots, batch/balance runs
  nw-tools/         `nw-tools` — content linting and balance reports
  nw-client/        Presentation client (Bevy; added at the UI spike)
docs/design/        The prototype design pack (source of the PASM spec)
pasm/spec/          The living PASM model of this codebase's design/architecture
```

The dependency direction keeps **Bevy out of the simulation**: `nw-simulation`
and `nw-content` never depend on the client, and Bevy is absent from the
workspace until the client spike, so a rendering dependency cannot leak into the
deterministic core by accident.

## Design records (PASM)

Design and architecture are recorded with [PASM](https://github.com/jkeywo/pasm)
— a specification-and-scanner model of what the codebase is *supposed* to be. The
spec under [`pasm/spec`](pasm/spec) is the living record; it is validated in CI
and kept in step with the implementation.

```bash
pip install -e .
pasm validate pasm/spec
```

## Development

Requires Rust 1.95 (pinned via `rust-toolchain.toml`) and Python 3.11+ for the
PASM tooling.

```bash
cargo test --workspace
cargo run --release -p nw-headless -- run --seed 1 --strategy deploy-first
cargo run --release -p nw-headless -- batch --count 4   # both build orders must win
cargo run -p nw-tools -- lint          # lint the authored content catalogue
```

## CI

Every push and pull request validates the PASM spec, checks formatting, runs
clippy as an error gate, and runs the workspace test suite (see
[`.github/workflows/ci.yml`](.github/workflows/ci.yml)). The WASM build and a
GitHub Pages deploy are added once the browser client exists.

## License

MIT — see [LICENSE](LICENSE).
