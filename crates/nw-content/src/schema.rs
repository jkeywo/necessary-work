//! The authored-content schema: the controlled effect vocabulary, project
//! definitions, scenario data, and opportunities.
//!
//! Projects act only through this vocabulary. Every effect declares an explicit
//! scope — no effect silently becomes global. A project using these mechanics
//! is content extension; a new effect type is mechanic extension and requires
//! architecture review (see `pasm/spec/core/decisions.yaml`).

use serde::{Deserialize, Serialize};

use crate::world::{Continent, Icon, Sector};

/// Where an effect applies. Spatial effects (icons, emissions) use the spatial
/// scopes; economy-wide effects (finance, mandate, cost/duration modifiers,
/// unlocks) use `Global`. `Spillover` is an authored transfer to a named other
/// continent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Scope {
    Global,
    LeadContinent,
    Continent(Continent),
    AllContinents,
    Spillover(Continent),
}

/// The controlled effect vocabulary. Magnitudes are integers: emissions in
/// milli-GtCO2e/year, money in milli-units per tick or flat milli-units.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectOp {
    AddIcon { icon: Icon, amount: i64 },
    RemoveIcon { icon: Icon, amount: i64 },
    ReduceSectorEmissions { sector: Sector, milli_gt: i64 },
    AddFinanceIncome { milli_per_tick: i64 },
    AddMandateIncome { milli_per_tick: i64 },
    AddMandateMaximum { milli: i64 },
    AddMaintenance { milli_per_tick: i64 },
    ModifyProjectCost { project: String, permille: i64 },
    ModifyProjectDuration { project: String, permille: i64 },
    UnlockProject { project: String },
}

impl EffectOp {
    /// Spatial ops apply per continent; the rest are economy-wide.
    pub fn is_spatial(&self) -> bool {
        matches!(
            self,
            EffectOp::AddIcon { .. }
                | EffectOp::RemoveIcon { .. }
                | EffectOp::ReduceSectorEmissions { .. }
        )
    }
}

/// An effect with its explicit scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopedEffect {
    pub op: EffectOp,
    pub scope: Scope,
}

/// How often a project can be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Repeat {
    /// Once, ever.
    Unique,
    /// Unlimited repeats; the cost curve prices each one.
    Repeatable,
    /// A fixed number of escalating completions, globally.
    Tiered { count: u32 },
    /// At most `per_continent` standing completions on each continent.
    /// Decommissioning frees the cap slot, but the cost curve still counts
    /// every completion ever, so toggling is never profitable.
    CappedRollout { per_continent: u32 },
}

/// Controlled cost curves. The multiplier is permille of the base cost as a
/// function of how many times the project has ever completed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CostCurve {
    Flat,
    Linear {
        increment_permille: i64,
    },
    Exponential {
        growth_permille: i64,
    },
    /// Economies of scale followed by depletion: the first `floor_count`
    /// completions each discount the cost, later ones grow it.
    ScaleThenDeplete {
        discount_permille: i64,
        floor_count: u32,
        growth_permille: i64,
    },
}

/// How a project's effects arrive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Activation {
    /// All effects at completion.
    Completion,
    /// Emissions effects roll out linearly during construction; everything
    /// else at completion. Cancelling removes the partial rollout.
    LinearRollout,
}

/// What must hold before the queue head can start. Icon prerequisites read the
/// lead continent's stocks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Prerequisite {
    IconAtLeast { icon: Icon, amount: i64 },
    CompletedOnLead { project: String },
    CompletedAnywhere { project: String },
}

/// Continuous scaling: a permille bonus per icon on the lead continent, capped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScalingRule {
    pub icon: Icon,
    pub permille_per_icon: i64,
    pub cap_permille: i64,
}

/// A discrete authored breakpoint: a flat permille bonus once the lead
/// continent holds at least `at_least` of the icon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Breakpoint {
    pub id: String,
    pub icon: Icon,
    pub at_least: i64,
    pub bonus_permille: i64,
}

/// Real-world context and source metadata, per the research plan's in-game
/// citation structure. Values are illustrative and balance-driven, never
/// forecasts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectContext {
    pub what: String,
    pub why: String,
    pub depends_on: String,
    pub limits: String,
    pub in_this_scenario: String,
    pub abstraction: String,
    pub sources: Vec<String>,
}

/// A data-defined project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectDef {
    pub id: String,
    pub title: String,
    pub summary: String,
    /// Whole finance units (stored as milli internally: x1000).
    pub finance_cost: i64,
    /// Whole mandate points, spent upfront.
    pub mandate_cost: i64,
    pub duration_ticks: u64,
    pub repeat: Repeat,
    pub cost_curve: CostCurve,
    pub activation: Activation,
    /// Locked projects need an `UnlockProject` effect before they can start.
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub prerequisites: Vec<Prerequisite>,
    #[serde(default)]
    pub scaling: Vec<ScalingRule>,
    #[serde(default)]
    pub breakpoints: Vec<Breakpoint>,
    /// Persistent effects while the project stands; all removed on
    /// decommission. Scaling and breakpoints modify only the
    /// `ReduceSectorEmissions` magnitudes.
    #[serde(default)]
    pub effects: Vec<ScopedEffect>,
    pub context: ProjectContext,
}

/// A simple deterministic beneficial opportunity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpportunityDef {
    pub id: String,
    pub title: String,
    pub effect: OpportunityEffect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpportunityEffect {
    FinanceGrant {
        units: i64,
    },
    MandateBoost {
        milli: i64,
    },
    /// Grants one icon to a continent drawn at spawn time.
    IconGrant {
        icon: Icon,
    },
}

/// The authored scenario: starting stocks, baselines, incomes, milestones.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub id: String,
    /// Client pacing hint; the simulation itself only counts ticks.
    pub authored_speed_ticks_per_second: u32,
    /// Whole finance units.
    pub finance_start: i64,
    /// Whole mandate points.
    pub mandate_start: i64,
    pub mandate_max_start: i64,
    /// Milli-units per tick, per continent — visibly positive baselines
    /// guarantee recoverability.
    pub finance_income_milli: Vec<(Continent, i64)>,
    pub mandate_income_milli: Vec<(Continent, i64)>,
    /// Milli-GtCO2e/year per (continent, sector); fixed unless projects act.
    pub sector_baselines_milli_gt: Vec<(Continent, Sector, i64)>,
    /// ± permille applied per sector by the `starting_variation` stream.
    pub starting_variation_permille: i64,
    /// Each milestone grants one further global programme slot when any
    /// continent's Institutions stock reaches the threshold. Slots ratchet.
    pub slot_milestones: Vec<SlotMilestone>,
    pub opportunity_gap_ticks: (u64, u64),
    pub opportunity_lifetime_ticks: u64,
    /// Periodic state-hash cadence for validation records.
    pub hash_every_ticks: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlotMilestone {
    pub institutions: i64,
}

/// Top level of `content/scenario.ron`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioFile {
    pub scenario: Scenario,
    pub opportunities: Vec<OpportunityDef>,
}

/// Top level of `content/projects.ron`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectsFile {
    pub projects: Vec<ProjectDef>,
}
