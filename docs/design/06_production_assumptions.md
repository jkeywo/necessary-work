# The Necessary Work — Production Assumptions

## Product

Single-player real-time incremental strategy game about coordinating global decarbonisation through the fictional Common Future Authority.

Initial targets:

- Windows native desktop.
- Browser build through WebAssembly.
- Shared Rust simulation.
- Bevy client.

## Development priorities

1. Satisfying core economy.
2. Reliable simulation and deterministic validation.
3. Comprehensible interface and causal explanations.
4. Educational integrity.
5. Content breadth.
6. Presentation polish.

## Prototype scope

- One 20–30 minute 1990 scenario.
- Approximately 12 projects.
- Three continents.
- Three sectors per continent.
- Four icon types.
- Finance and Mandate.
- One to three global programme slots.
- No loss state.
- No online services.

## Technology

### Suggested workspace

- `simulation` — deterministic pure Rust core;
- `content` — schemas, loading, validation, and project data;
- `client` — Bevy application;
- `persistence` — snapshots, logs, records, archive;
- `headless` — replay, bots, batch runs, benchmarks;
- `tools` — content linting and balance reports.

Dependency direction must keep Bevy out of the simulation.

### Content format

Human-reviewable structured data, likely RON, JSON, or YAML, with schema validation, stable IDs, explicit units/scopes, source metadata, linting, and content hashes.

No arbitrary scripts through Alpha 1. Rhai may be added later only for behaviour the controlled effect vocabulary cannot express cleanly.

### Determinism

Fixed tick; integer/fixed-point arithmetic; named RNG streams; timestamped semantic command log; headless replay; periodic hashes; versioned records.

### Persistence

One autosave, manual save/load, local personal bests, archived version-specific records, no cross-version compatibility promise, no cloud save, no offline progression.

Snapshots may accelerate loading, but command replay is authoritative.

## Interface assumptions

- Mouse-first desktop interface.
- Keyboard shortcuts desirable but not required initially.
- Main dashboard plus contextual side panel.
- Compact information-dense presentation.
- No separate map-navigation layer in the first prototype.

The WASM layout may rearrange panels but must retain global totals, continent summaries, active programmes, queue, projects, and critical notifications.

## Accessibility baseline

- Pause with unrestricted planning.
- Readable scalable text where practical.
- No information conveyed only by colour.
- Persistent notifications.
- Numerical values alongside bars.
- No rapid-click requirement.
- Tooltips or expanded explanations.

Full accessibility review belongs to the vertical slice.

## Presentation assumptions

Prototype may use flat panels, basic icons, placeholder illustrations, simple completion animations, provisional typography, and minimal audio.

It must still support judging completion satisfaction, breakpoint impact, resource acceleration, queue activity, and the final approach to zero.

Normal-speed pacing must stand on its own.

## Playtesting

### Internal

Determinism; seeded bots; economy stress; cancellation/recoverability; dominant-strategy search; preview accuracy; benchmark clock.

### Human

Use fixed reference seeds initially. Collect completion time, action log, queue stalls, project choices, thresholds, cancellations, pause time, panel use, post-run explanation, and pacing/satisfaction feedback.

No participant number is fixed yet. Use enough sessions to detect dominant openings, misunderstood mechanics, dead waiting, enabling-project taxes, and at least two plausible successful build orders.

Do not infer broad educational outcomes from a small convenience sample.

## Benchmarks

Initially use handcrafted, explicitly provisional local brackets based on active unpaused simulation time. Record developer, bot, and consenting playtest runs in the same validated format. Replace handcrafted brackets with an empirical reference distribution when the sample is sufficiently large and representative.

No online leaderboard in prototype or Alpha 1.

## Content production

Every project requires game data, normal summary, expanded mechanical explanation, real-world description, enabling conditions, limits/trade-offs, sources, abstraction note, and tests.

A project using existing mechanics is content extension. A new effect type or causal system is mechanic extension and requires architecture review.

## Team and schedule

No team size or deadline is fixed. Manage by exit criteria rather than dates.

Estimate separately: simulation, Bevy UI, content tooling, persistence/replay, project content, research review, balancing, playtesting, and visual/audio production.

Implement the deterministic simulation, Bevy UI spike, and one complete end-to-end project before bulk content production.

## Risks

### Design

Enabling projects become taxes; one opening dominates; slot milestones encourage exploitative concentration; normal-speed pacing is dull; non-losable play lacks tension; timing discourages reading; previews solve rather than clarify decisions.

### Technical

Simulation leaks into Bevy; floating-point divergence; content effects become ad hoc code; snapshots and logs diverge; WASM constraints arrive late; explanation UI diverges from calculations.

### Educational

Europe appears naturally central; regions become a hierarchy; Finance appears the only constraint; technology is detached from politics/labour/materials/distribution; precise numbers imply certainty; 1990 appears politically easy; speed becomes moral performance.

## Early implementation order

1. Define units and fixed tick.
2. Implement scenario state and one sector.
3. Implement Finance and Mandate flows.
4. Implement one icon family and one project.
5. Implement semantic commands and deterministic replay.
6. Implement project lifecycle and queue.
7. Generalise to controlled data effects.
8. Add calculation trace and projected preview.
9. Add remaining sectors, continents, and icons.
10. Add slot milestones.
11. Add opportunities with named RNG streams.
12. Add save cache and record archive.
13. Build main Bevy dashboard.
14. Complete the 12-project prototype catalogue.
15. Add end-screen timing, bracket, and causal recap.
16. Run structured prototype playtests.
