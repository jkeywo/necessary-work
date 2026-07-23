# The Necessary Work — Simulation Contract

## 1. Architectural boundary

The simulation is a pure Rust library independent of Bevy, rendering, OS, save UI, and network code. Clients use a narrow command/state interface.

Required uses:

- native Bevy client;
- WASM Bevy client;
- headless tests;
- deterministic replay;
- batch balance simulations;
- debug inspection.

## 2. Determinism

The simulation is deterministic given ruleset version, content version, scenario identifier, scenario seed, and ordered timestamped commands.

Wall-clock time, frame rate, rendering state, input polling order, locale, and platform APIs may not affect authoritative results.

## 3. Canonical units

### Emissions

Simplified, rounded GtCO2e/year. Use “global emissions” or “gross emissions,” not “net emissions,” while removal is absent.

### Finance

Abstract global CFA units with visible continental contribution breakdown.

### Mandate

Abstract global points with current, maximum, income rate, and visible continental contributions.

### Icons

Non-negative continental integer stocks: Knowledge, Infrastructure, Workforce, Institutions.

### Time

Canonical integer simulation ticks. All durations and accrual derive from ticks. Ranked time is active, unpaused ticks from start to valid victory.

Authoritative state should use integers or fixed-point values, not floating-point outcomes.

## 4. Authoritative state

At minimum:

- current tick and pause state;
- ruleset/content versions;
- scenario seed;
- named RNG stream states;
- Finance;
- Mandate current/max;
- continental contribution rates;
- nine sector emissions rates;
- four icon stocks per continent;
- project availability and lifecycle state;
- active programme-slot count;
- milestone progress;
- opportunity state;
- event and explanation trace;
- victory state.

## 5. Command interface

Semantic commands, not UI events. Likely commands:

- QueueProject
- RemoveQueuedProject
- ReorderQueue
- CancelActiveProject
- DecommissionProject
- SelectProjectLeadContinent
- Pause
- Resume
- ClaimOpportunity

Every command returns a structured accepted/rejected result. Rejected commands do not consume resources, alter state, or advance time, and include machine- and human-readable reasons.

## 6. Tick order

Recommended fixed order:

1. Apply commands scheduled for the tick in logged order.
2. Resolve pause/resume.
3. If paused, emit explanations and stop.
4. Accrue Finance and Mandate.
5. Advance project construction.
6. Recalculate partial rollout effects.
7. Complete eligible projects.
8. Apply completion effects and icons.
9. Recalculate prerequisites, scaling, breakpoints, and unlocks.
10. Resolve programme-slot milestones.
11. Advance opportunity timing.
12. Create or expire opportunities.
13. Attempt to start the FIFO queue head.
14. Recalculate derived totals and deltas.
15. Test victory.
16. Emit hashes and explanation events as required.

The implementation may refine this order, but it must remain singular, documented, and tested.

## 7. Controlled effect vocabulary

Candidate families:

- AddIcon
- RemoveIcon
- ReduceSectorEmissions
- AddFinanceIncome
- AddMandateIncome
- AddMandateMaximum
- AddMaintenance
- ModifyProjectCost
- ModifyProjectDuration
- AddProgrammeSlotMilestoneProgress
- UnlockProject
- AddLocalEffect
- AddSharedEffect
- AddTransferredEffect
- AddBreakpoint
- AddScalingRule

Every effect has explicit scope: global, lead continent, named continent, all continents, authored subset, named sector, or authored spillover target. No effect silently becomes global.

## 8. Scaling and breakpoints

Projects may define prerequisites, continuous scaling, and discrete authored breakpoints. Scaling functions come from a controlled set.

Every calculation exposes base value, contributing icons, modifier names, modifier order, final value, and next breakpoint. Project previews and debug traces use this same output.

## 9. Project lifecycle

### Queued

No resources reserved; no slot occupied; freely reordered or removed.

### Active

Full costs paid upfront; one slot occupied; progress advances only while unpaused.

### Cancelled while active

Progress destroyed; partial effects removed; no refund; slot released.

### Completed

Completion effects applied; recurring effects active; may be decommissioned.

### Decommissioned

Benefits, icons, spillovers, and upkeep removed; no refund. Rebuild rules must prevent resource creation or cost bypass.

## 10. Recoverability and victory

Invariants:

- baseline Finance income is positive;
- baseline Mandate income is positive;
- no random event creates an unrecoverable penalty;
- no loss state exists;
- every valid start has a path to victory.

Victory requires gross global emissions of zero and non-negative Finance and Mandate income deltas.

## 11. Randomness

Use independent named deterministic streams:

- `starting_variation`
- `opportunity_timing`
- `opportunity_selection`

Each derives from the scenario seed plus a stable identifier. Systems may not share streams. Adding a draw to one system must not perturb another.

## 12. Saves, logs, and validation

Authoritative validation record:

- ruleset version;
- content version;
- scenario identifier;
- scenario seed;
- all accepted and rejected commands;
- command tick;
- pause/resume transitions;
- periodic state hashes;
- final hash;
- completion tick;
- validation result.

Validation recreates the initial state, replays commands, checks periodic hashes, and confirms final hash and victory tick.

A save may cache a snapshot for fast loading, but the command log is authoritative. Compatibility is version-specific; old records are archived, not converted.

## 13. Benchmark timing

- Advances only when simulation advances.
- Pause stops it.
- Planning while paused is allowed.
- Panels do not pause automatically.
- Hardware frame rate cannot affect it.
- Debug acceleration invalidates ranked records unless explicitly supported.

## 14. Explanation contract

Every visible state change should have a structured trace containing tick, affected value, previous and new values, direct cause, modifiers, source, scope, and direct/conditional/derived status.

This powers previews, formulas, notifications, causal recap, and debug inspection.

## 15. Test obligations

- deterministic replay across supported builds;
- RNG stream isolation;
- cancellation and decommission cleanup;
- queue FIFO and blocked-head explanations;
- slot milestone triggers;
- scaling and breakpoint order;
- victory validation;
- recoverability;
- preview equals realised effect under unchanged conditions;
- version-specific replay rejection;
- pause timing;
- seeded batch runs showing at least two plausible winning build orders.

## 16. Deferred

Rhai scripting; network validation; online leaderboards; cross-version migration; offline progress; crises; local legitimacy; degradation and failures.
