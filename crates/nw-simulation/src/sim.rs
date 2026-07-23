//! The simulation: command application and the single, documented tick order.
//!
//! Each tick, in order: apply logged commands (done by the caller between
//! ticks, in log order), resolve pause (a paused tick is a pure no-op), accrue
//! Finance and Mandate, advance construction, complete eligible projects and
//! lock in their realised effects, rederive all derived state (partial
//! rollouts, prerequisites, unlocks, slot milestones), advance opportunity
//! timing and expiry, attempt the FIFO queue head, rederive, and test victory.
//! Hashing is sampled externally by the persistence harness.

use nw_content::schema::{
    Activation, EffectOp, OpportunityEffect, Prerequisite, ProjectDef, Repeat, Scope,
};
use nw_content::world::{Continent, Icon};
use nw_content::Catalogue;

use crate::calc::{self, CalcTrace, Modifier};
use crate::command::{Command, LoggedCommand, Rejection};
use crate::rng::RngStreams;
use crate::state::{
    ActiveProject, CompletedProject, OpenOpportunity, QueuedProject, RealizedEffect, RunState,
};
use crate::trace::{BlockReason, TraceEvent, TraceKind};

/// The ruleset version. Replay records are version-specific; a mismatch is
/// rejected outright rather than converted.
pub const RULESET_VERSION: &str = "proto-1";

pub struct Sim {
    catalogue: Catalogue,
    pub state: RunState,
    pub log: Vec<LoggedCommand>,
}

impl Sim {
    pub fn new(catalogue: Catalogue, seed: u64) -> Sim {
        let mut streams = RngStreams::new(seed);
        let scenario = &catalogue.scenario;

        // Starting variation, drawn in authored order from its own stream.
        let variation = scenario.starting_variation_permille;
        let mut baselines = [[0i64; 3]; 3];
        for (continent, sector, base) in &scenario.sector_baselines_milli_gt {
            let draw = streams.starting_variation.range_i64(-variation, variation);
            baselines[continent.index()][sector.index()] = calc::apply_permille(*base, draw);
        }

        let mut finance_income = [0i64; 3];
        for (continent, income) in &scenario.finance_income_milli {
            finance_income[continent.index()] = *income;
        }
        let mut mandate_income = [0i64; 3];
        for (continent, income) in &scenario.mandate_income_milli {
            mandate_income[continent.index()] = *income;
        }

        let first_gap = streams.opportunity_timing.range_u64(
            scenario.opportunity_gap_ticks.0,
            scenario.opportunity_gap_ticks.1,
        );

        let state = RunState {
            tick: 0,
            paused: false,
            seed,
            streams,
            baseline_emissions_milli: baselines,
            baseline_finance_income_milli: finance_income,
            baseline_mandate_income_milli: mandate_income,
            finance_milli: scenario.finance_start * 1000,
            mandate_milli: scenario.mandate_start * 1000,
            bonus_icons: [[0; 4]; 3],
            queue: Vec::new(),
            active: Vec::new(),
            completed: Vec::new(),
            completions_ever: vec![0; catalogue.projects.len()],
            unlocked: catalogue.projects.iter().map(|p| !p.locked).collect(),
            opportunities: Vec::new(),
            next_opportunity_tick: first_gap,
            victory_tick: None,
            last_block: None,
            sector_emissions_milli: baselines,
            icons: [[0; 4]; 3],
            finance_delta_milli: 0,
            mandate_delta_milli: 0,
            mandate_max_milli: scenario.mandate_max_start * 1000,
            slots_total: 1,
            trace: Vec::new(),
        };

        let mut sim = Sim {
            catalogue,
            state,
            log: Vec::new(),
        };
        sim.rederive();
        sim
    }

    pub fn catalogue(&self) -> &Catalogue {
        &self.catalogue
    }

    // ------------------------------------------------------------- commands

    /// Apply one semantic command against the current state and log the
    /// outcome. Rejected commands change nothing.
    pub fn execute(&mut self, command: Command) -> Result<(), Rejection> {
        let result = self.apply(&command);
        self.log.push(LoggedCommand {
            tick: self.state.tick,
            command,
            rejection: result.clone().err(),
        });
        result
    }

    fn apply(&mut self, command: &Command) -> Result<(), Rejection> {
        let state = &mut self.state;
        match command {
            Command::QueueProject { project, lead } => {
                let Some(index) = self.catalogue.project_index(project) else {
                    return Err(Rejection::UnknownProject {
                        id: project.clone(),
                    });
                };
                state.queue.push(QueuedProject {
                    project: index as u32,
                    lead: *lead,
                });
                Ok(())
            }
            Command::RemoveQueuedProject { index } => {
                let index = *index as usize;
                if index >= state.queue.len() {
                    return Err(Rejection::InvalidIndex);
                }
                state.queue.remove(index);
                Ok(())
            }
            Command::ReorderQueue { from, to } => {
                let (from, to) = (*from as usize, *to as usize);
                if from >= state.queue.len() || to >= state.queue.len() {
                    return Err(Rejection::InvalidIndex);
                }
                let entry = state.queue.remove(from);
                state.queue.insert(to, entry);
                Ok(())
            }
            Command::SelectProjectLeadContinent { index, lead } => {
                let index = *index as usize;
                if index >= state.queue.len() {
                    return Err(Rejection::InvalidIndex);
                }
                state.queue[index].lead = *lead;
                Ok(())
            }
            Command::CancelActiveProject { index } => {
                let index = *index as usize;
                if index >= state.active.len() {
                    return Err(Rejection::InvalidIndex);
                }
                // Progress and partial rollout destroyed; no refund; slot freed.
                let build = state.active.remove(index);
                let id = self.catalogue.projects[build.project as usize].id.clone();
                let tick = state.tick;
                state.trace.push(TraceEvent {
                    tick,
                    kind: TraceKind::ProjectCancelled {
                        project: id,
                        lead: build.lead,
                    },
                });
                self.rederive();
                Ok(())
            }
            Command::DecommissionProject { index } => {
                let index = *index as usize;
                if index >= state.completed.len() {
                    return Err(Rejection::InvalidIndex);
                }
                // Benefits, icons, spillovers, and upkeep all disappear with
                // the entry; no refund. The cost curve still counts it.
                let entry = state.completed.remove(index);
                let id = self.catalogue.projects[entry.project as usize].id.clone();
                let tick = state.tick;
                state.trace.push(TraceEvent {
                    tick,
                    kind: TraceKind::ProjectDecommissioned {
                        project: id,
                        lead: entry.lead,
                    },
                });
                self.rederive();
                Ok(())
            }
            Command::Pause => {
                if state.paused {
                    return Err(Rejection::AlreadyPaused);
                }
                state.paused = true;
                Ok(())
            }
            Command::Resume => {
                if !state.paused {
                    return Err(Rejection::NotPaused);
                }
                state.paused = false;
                Ok(())
            }
            Command::ClaimOpportunity { index } => {
                let index = *index as usize;
                if index >= state.opportunities.len() {
                    return Err(Rejection::InvalidIndex);
                }
                let open = state.opportunities.remove(index);
                let def = &self.catalogue.opportunities[open.def as usize];
                match def.effect {
                    OpportunityEffect::FinanceGrant { units } => {
                        state.finance_milli += units * 1000;
                    }
                    OpportunityEffect::MandateBoost { milli } => {
                        state.mandate_milli =
                            (state.mandate_milli + milli).min(state.mandate_max_milli);
                    }
                    OpportunityEffect::IconGrant { icon } => {
                        state.bonus_icons[open.continent.index()][icon.index()] += 1;
                    }
                }
                let tick = state.tick;
                state.trace.push(TraceEvent {
                    tick,
                    kind: TraceKind::OpportunityClaimed { id: def.id.clone() },
                });
                self.rederive();
                Ok(())
            }
        }
    }

    // ----------------------------------------------------------------- tick

    /// Advance one tick. While paused this is a pure no-op: the tick counter
    /// (which is also ranked time) does not advance and nothing accrues, but
    /// planning commands remain available between calls.
    pub fn tick(&mut self) {
        if self.state.paused {
            return;
        }
        self.state.tick += 1;
        let tick = self.state.tick;

        // Accrue Finance and Mandate at the deltas derived last tick.
        self.state.finance_milli += self.state.finance_delta_milli;
        self.state.mandate_milli = (self.state.mandate_milli + self.state.mandate_delta_milli)
            .min(self.state.mandate_max_milli);

        // Advance construction.
        for build in &mut self.state.active {
            build.progress += 1;
        }

        // Complete eligible projects, locking in realised effects with the
        // icons as derived at the end of the previous tick.
        let mut index = 0;
        while index < self.state.active.len() {
            if self.state.active[index].progress >= self.state.active[index].duration {
                let build = self.state.active.remove(index);
                self.complete(build, tick);
            } else {
                index += 1;
            }
        }

        // Rederive: icons, partial rollouts, unlocks, milestones, deltas.
        self.rederive();

        // Opportunities: expiry, then seeded spawning.
        self.opportunity_step(tick);

        // Attempt the FIFO queue head, then rederive if anything started.
        self.try_start_head(tick);

        // Victory.
        if self.state.victory_tick.is_none() && self.state.victory_condition_met() {
            self.state.victory_tick = Some(tick);
            self.state.trace.push(TraceEvent {
                tick,
                kind: TraceKind::VictoryReached,
            });
        }
    }

    fn complete(&mut self, build: ActiveProject, tick: u64) {
        let def = &self.catalogue.projects[build.project as usize];
        let bonus = calc::reduction_bonus(def, build.lead, &self.state.icons);
        let realized = def
            .effects
            .iter()
            .map(|effect| {
                let op = match &effect.op {
                    EffectOp::ReduceSectorEmissions { sector, milli_gt } => {
                        EffectOp::ReduceSectorEmissions {
                            sector: *sector,
                            milli_gt: calc::apply_permille(*milli_gt, bonus.bonus_permille),
                        }
                    }
                    other => other.clone(),
                };
                RealizedEffect {
                    op,
                    scope: effect.scope,
                }
            })
            .collect();

        self.state.completions_ever[build.project as usize] += 1;
        self.state.completed.push(CompletedProject {
            project: build.project,
            lead: build.lead,
            tick,
            realized,
            bonus_permille: bonus.bonus_permille,
        });
        self.state.trace.push(TraceEvent {
            tick,
            kind: TraceKind::ProjectCompleted {
                project: def.id.clone(),
                lead: build.lead,
                bonus_permille: bonus.bonus_permille,
            },
        });
    }

    fn opportunity_step(&mut self, tick: u64) {
        // Expire.
        let mut expired = Vec::new();
        self.state.opportunities.retain(|open| {
            if open.expires <= tick {
                expired.push(open.def);
                false
            } else {
                true
            }
        });
        for def_index in expired {
            let id = self.catalogue.opportunities[def_index as usize].id.clone();
            self.state.trace.push(TraceEvent {
                tick,
                kind: TraceKind::OpportunityExpired { id },
            });
        }

        // Spawn on schedule: selection and timing use their own streams.
        if tick >= self.state.next_opportunity_tick && !self.catalogue.opportunities.is_empty() {
            let selection = &mut self.state.streams.opportunity_selection;
            let def_index = selection.pick(self.catalogue.opportunities.len());
            let continent = Continent::ALL[selection.pick(Continent::ALL.len())];
            let lifetime = self.catalogue.scenario.opportunity_lifetime_ticks;
            self.state.opportunities.push(OpenOpportunity {
                def: def_index as u32,
                continent,
                opened: tick,
                expires: tick + lifetime,
            });
            let gap_bounds = self.catalogue.scenario.opportunity_gap_ticks;
            let gap = self
                .state
                .streams
                .opportunity_timing
                .range_u64(gap_bounds.0, gap_bounds.1);
            self.state.next_opportunity_tick = tick + gap;
            let id = self.catalogue.opportunities[def_index].id.clone();
            self.state.trace.push(TraceEvent {
                tick,
                kind: TraceKind::OpportunityOpened { id, continent },
            });
        }
    }

    fn try_start_head(&mut self, tick: u64) {
        let Some(&head) = self.state.queue.first() else {
            self.state.last_block = None;
            return;
        };
        match self.head_block(head) {
            None => {
                let (cost_milli, _) = self.current_cost_milli(head.project as usize, head.lead);
                let def = &self.catalogue.projects[head.project as usize];
                let duration = self.current_duration(head.project as usize);
                self.state.finance_milli -= cost_milli;
                self.state.mandate_milli -= def.mandate_cost * 1000;
                self.state.queue.remove(0);
                self.state.active.push(ActiveProject {
                    project: head.project,
                    lead: head.lead,
                    started: tick,
                    progress: 0,
                    duration,
                });
                self.state.last_block = None;
                self.state.trace.push(TraceEvent {
                    tick,
                    kind: TraceKind::ProjectStarted {
                        project: def.id.clone(),
                        lead: head.lead,
                    },
                });
                self.rederive();
            }
            Some(reason) => {
                // The queue stalls and explains why — trace only on change.
                if self.state.last_block.as_ref() != Some(&reason) {
                    self.state.trace.push(TraceEvent {
                        tick,
                        kind: TraceKind::QueueBlocked {
                            reason: reason.clone(),
                        },
                    });
                    self.state.last_block = Some(reason);
                }
            }
        }
    }

    /// Why the queue head cannot start right now, or `None` if it can.
    /// Checked in a fixed order: unlock, repeat limit, prerequisites, slot,
    /// finance, mandate.
    pub fn head_block(&self, head: QueuedProject) -> Option<BlockReason> {
        let project = head.project as usize;
        let def = &self.catalogue.projects[project];
        let state = &self.state;

        if !state.unlocked[project] {
            return Some(BlockReason::NotUnlocked);
        }

        let active_count = state
            .active
            .iter()
            .filter(|build| build.project == head.project)
            .count() as u32;
        let repeat_blocked = match def.repeat {
            Repeat::Unique => state.completions_ever[project] + active_count >= 1,
            Repeat::Repeatable => false,
            Repeat::Tiered { count } => state.completions_ever[project] + active_count >= count,
            Repeat::CappedRollout { per_continent } => {
                let standing = state
                    .completed
                    .iter()
                    .filter(|done| done.project == head.project && done.lead == head.lead)
                    .count() as u32;
                let building = state
                    .active
                    .iter()
                    .filter(|build| build.project == head.project && build.lead == head.lead)
                    .count() as u32;
                standing + building >= per_continent
            }
        };
        if repeat_blocked {
            return Some(BlockReason::RepeatLimitReached);
        }

        for prerequisite in &def.prerequisites {
            match prerequisite {
                Prerequisite::IconAtLeast { icon, amount } => {
                    let have = state.icons[head.lead.index()][icon.index()];
                    if have < *amount {
                        return Some(BlockReason::MissingIcon {
                            icon: *icon,
                            needed: *amount,
                            have,
                        });
                    }
                }
                Prerequisite::CompletedOnLead { project: required } => {
                    let met = state.completed.iter().any(|done| {
                        self.catalogue.projects[done.project as usize].id == *required
                            && done.lead == head.lead
                    });
                    if !met {
                        return Some(BlockReason::MissingProject {
                            project: required.clone(),
                        });
                    }
                }
                Prerequisite::CompletedAnywhere { project: required } => {
                    let met = state
                        .completed
                        .iter()
                        .any(|done| self.catalogue.projects[done.project as usize].id == *required);
                    if !met {
                        return Some(BlockReason::MissingProject {
                            project: required.clone(),
                        });
                    }
                }
            }
        }

        if state.active.len() as u32 >= state.slots_total {
            return Some(BlockReason::NoFreeSlot);
        }

        let (cost_milli, _) = self.current_cost_milli(project, head.lead);
        if state.finance_milli < cost_milli {
            return Some(BlockReason::InsufficientFinance {
                needed_milli: cost_milli,
                have_milli: state.finance_milli,
            });
        }
        let mandate_milli = def.mandate_cost * 1000;
        if state.mandate_milli < mandate_milli {
            return Some(BlockReason::InsufficientMandate {
                needed_milli: mandate_milli,
                have_milli: state.mandate_milli,
            });
        }
        None
    }

    // ---------------------------------------------------------- calculations

    /// The current cost of starting a project, with the full calc trace:
    /// base, repeat-curve multiplier, then external cost modifiers, in order.
    pub fn current_cost_milli(&self, project: usize, _lead: Continent) -> (i64, CalcTrace) {
        let def = &self.catalogue.projects[project];
        let base = def.finance_cost * 1000;
        let mut modifiers = Vec::new();

        let curve =
            calc::cost_multiplier_permille(def.cost_curve, self.state.completions_ever[project]);
        modifiers.push(Modifier {
            name: "repeat-curve".into(),
            permille: curve - 1000,
        });
        let mut value = base * curve / 1000;

        let mut external = 0;
        for done in &self.state.completed {
            for effect in &done.realized {
                if let EffectOp::ModifyProjectCost {
                    project: target,
                    permille,
                } = &effect.op
                {
                    if *target == def.id {
                        let source = &self.catalogue.projects[done.project as usize].id;
                        modifiers.push(Modifier {
                            name: format!("modifier:{source}"),
                            permille: *permille,
                        });
                        external += permille;
                    }
                }
            }
        }
        value = calc::apply_permille(value, external);

        let trace = CalcTrace {
            base,
            bonus_permille: (curve - 1000) + external,
            modifiers,
            final_value: value,
        };
        (value, trace)
    }

    /// The current duration of a project, after duration modifiers.
    pub fn current_duration(&self, project: usize) -> u64 {
        let def = &self.catalogue.projects[project];
        let mut external = 0i64;
        for done in &self.state.completed {
            for effect in &done.realized {
                if let EffectOp::ModifyProjectDuration {
                    project: target,
                    permille,
                } = &effect.op
                {
                    if *target == def.id {
                        external += permille;
                    }
                }
            }
        }
        (calc::apply_permille(def.duration_ticks as i64, external)).max(1) as u64
    }

    /// Rederive every derived field from the standing lists. Public so tests
    /// and tools can rebuild derived state after direct state edits.
    pub fn rederive(&mut self) {
        let previous_slots = self.state.slots_total;
        derive_state(&self.catalogue, &mut self.state);
        if self.state.slots_total > previous_slots {
            let tick = self.state.tick;
            let total = self.state.slots_total;
            self.state.trace.push(TraceEvent {
                tick,
                kind: TraceKind::SlotUnlocked { total },
            });
        }
    }
}

/// Which continents a scoped effect touches.
fn scope_continents(scope: Scope, lead: Continent) -> Vec<Continent> {
    match scope {
        Scope::Global | Scope::AllContinents => Continent::ALL.to_vec(),
        Scope::LeadContinent => vec![lead],
        Scope::Continent(continent) | Scope::Spillover(continent) => vec![continent],
    }
}

/// Recompute all derived state from baselines plus the standing completed and
/// active lists. Everything a project contributes lives here, so removing its
/// entry removes every trace of it.
pub fn derive_state(catalogue: &Catalogue, state: &mut RunState) {
    // Icons: opportunity bonuses plus standing completed projects.
    let mut icons = state.bonus_icons;
    for done in &state.completed {
        for effect in &done.realized {
            match &effect.op {
                EffectOp::AddIcon { icon, amount } => {
                    for continent in scope_continents(effect.scope, done.lead) {
                        icons[continent.index()][icon.index()] += amount;
                    }
                }
                EffectOp::RemoveIcon { icon, amount } => {
                    for continent in scope_continents(effect.scope, done.lead) {
                        icons[continent.index()][icon.index()] -= amount;
                    }
                }
                _ => {}
            }
        }
    }
    for row in &mut icons {
        for stock in row {
            *stock = (*stock).max(0);
        }
    }
    state.icons = icons;

    // Unlock ratchet: once granted, an unlock persists.
    for done in &state.completed {
        for effect in &done.realized {
            if let EffectOp::UnlockProject { project } = &effect.op {
                if let Some(index) = catalogue.project_index(project) {
                    state.unlocked[index] = true;
                }
            }
        }
    }

    // Emissions: baselines minus realised reductions minus linear-rollout
    // partials (which use *current* scaling until completion locks them).
    let mut emissions = state.baseline_emissions_milli;
    for done in &state.completed {
        for effect in &done.realized {
            if let EffectOp::ReduceSectorEmissions { sector, milli_gt } = &effect.op {
                for continent in scope_continents(effect.scope, done.lead) {
                    emissions[continent.index()][sector.index()] -= milli_gt;
                }
            }
        }
    }
    for build in &state.active {
        let def = &catalogue.projects[build.project as usize];
        if def.activation != Activation::LinearRollout || build.duration == 0 {
            continue;
        }
        let bonus = calc::reduction_bonus(def, build.lead, &state.icons);
        for effect in &def.effects {
            if let EffectOp::ReduceSectorEmissions { sector, milli_gt } = &effect.op {
                let scaled = calc::apply_permille(*milli_gt, bonus.bonus_permille);
                let partial = scaled * build.progress as i64 / build.duration as i64;
                for continent in scope_continents(effect.scope, build.lead) {
                    emissions[continent.index()][sector.index()] -= partial;
                }
            }
        }
    }
    for row in &mut emissions {
        for rate in row {
            *rate = (*rate).max(0);
        }
    }
    state.sector_emissions_milli = emissions;

    // Deltas and the mandate maximum.
    let mut finance_delta: i64 = state.baseline_finance_income_milli.iter().sum();
    let mut mandate_delta: i64 = state.baseline_mandate_income_milli.iter().sum();
    let mut mandate_max = catalogue.scenario.mandate_max_start * 1000;
    for done in &state.completed {
        for effect in &done.realized {
            match &effect.op {
                EffectOp::AddFinanceIncome { milli_per_tick } => finance_delta += milli_per_tick,
                EffectOp::AddMaintenance { milli_per_tick } => finance_delta -= milli_per_tick,
                EffectOp::AddMandateIncome { milli_per_tick } => mandate_delta += milli_per_tick,
                EffectOp::AddMandateMaximum { milli } => mandate_max += milli,
                _ => {}
            }
        }
    }
    state.finance_delta_milli = finance_delta;
    state.mandate_delta_milli = mandate_delta;
    state.mandate_max_milli = mandate_max;
    state.mandate_milli = state.mandate_milli.min(mandate_max);

    // Programme-slot milestones: driven by the best continental Institutions
    // stock. Slots ratchet — a milestone once passed stays passed.
    let best_institutions = Continent::ALL
        .iter()
        .map(|continent| state.icons[continent.index()][Icon::Institutions.index()])
        .max()
        .unwrap_or(0);
    let reached = catalogue
        .scenario
        .slot_milestones
        .iter()
        .filter(|milestone| best_institutions >= milestone.institutions)
        .count() as u32;
    state.slots_total = state.slots_total.max(1 + reached);
}

/// Look up a project definition by id.
pub fn project_def<'a>(catalogue: &'a Catalogue, id: &str) -> Option<&'a ProjectDef> {
    catalogue.projects.iter().find(|p| p.id == id)
}
