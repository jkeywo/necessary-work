//! Projected outcome previews. A preview is computed by realising the project
//! hypothetically against a copy of the current state and rederiving — the
//! exact code path a real completion takes — so a preview equals the realised
//! effect under unchanged conditions by construction.
//!
//! The preview separates guaranteed direct effects (costs, reductions at
//! current scaling, icon totals) from conditional consequences (unlocks,
//! unmet breakpoints). There is no combined efficiency score and no
//! recommendation.

use nw_content::schema::{Breakpoint, EffectOp};
use nw_content::world::Continent;

use crate::calc::{self, CalcTrace, Modifier};
use crate::sim::{derive_state, Sim};
use crate::state::{CompletedProject, RealizedEffect};

#[derive(Debug, Clone)]
pub struct Preview {
    pub project: String,
    pub lead: Continent,

    // Costs and timing.
    pub finance_cost_milli: i64,
    pub mandate_cost_milli: i64,
    pub duration_ticks: u64,
    pub cost_trace: CalcTrace,
    /// Ticks until both Finance and Mandate cover the costs at current
    /// deltas: 0 if affordable now, `None` if a delta is non-positive.
    pub ticks_until_affordable: Option<u64>,

    // Guaranteed direct effects, at current scaling.
    pub emissions_change_milli: [i64; 3],
    pub global_emissions_change_milli: i64,
    pub finance_delta_change_milli: i64,
    pub mandate_delta_change_milli: i64,
    pub mandate_max_change_milli: i64,
    pub icons_after: [[i64; 4]; 3],
    /// The active scaling calculation: base reduction, ordered modifiers,
    /// final value (for the project's first reduction effect).
    pub reduction_modifiers: Vec<Modifier>,
    pub reduction_bonus_permille: i64,

    // Conditional consequences.
    pub breakpoints_met: Vec<String>,
    pub next_breakpoint: Option<Breakpoint>,
    pub unlocks: Vec<String>,
}

/// Preview queueing `project` with `lead`. Returns `None` for unknown ids.
pub fn preview(sim: &Sim, project: &str, lead: Continent) -> Option<Preview> {
    let catalogue = sim.catalogue();
    let index = catalogue.project_index(project)?;
    let def = &catalogue.projects[index];

    let (finance_cost_milli, cost_trace) = sim.current_cost_milli(index, lead);
    let mandate_cost_milli = def.mandate_cost * 1000;
    let duration_ticks = sim.current_duration(index);

    // Realise hypothetically — the same realisation a completion performs.
    let bonus = calc::reduction_bonus(def, lead, &sim.state.icons);
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

    let mut hypothetical = sim.state.clone();
    hypothetical.completed.push(CompletedProject {
        project: index as u32,
        lead,
        tick: hypothetical.tick,
        realized,
        bonus_permille: bonus.bonus_permille,
    });
    derive_state(catalogue, &mut hypothetical);

    let mut emissions_change = [0i64; 3];
    for continent in Continent::ALL {
        emissions_change[continent.index()] = hypothetical.continent_emissions_milli(continent)
            - sim.state.continent_emissions_milli(continent);
    }

    let unlocks = def
        .effects
        .iter()
        .filter_map(|effect| match &effect.op {
            EffectOp::UnlockProject { project } => {
                let target = catalogue.project_index(project)?;
                (!sim.state.unlocked[target]).then(|| project.clone())
            }
            _ => None,
        })
        .collect();

    let ticks_until_affordable = affordable_in(
        sim.state.finance_milli,
        finance_cost_milli,
        sim.state.finance_delta_milli,
    )
    .and_then(|finance_wait| {
        affordable_in(
            sim.state.mandate_milli,
            mandate_cost_milli,
            sim.state.mandate_delta_milli,
        )
        .map(|mandate_wait| finance_wait.max(mandate_wait))
    });

    Some(Preview {
        project: def.id.clone(),
        lead,
        finance_cost_milli,
        mandate_cost_milli,
        duration_ticks,
        cost_trace,
        ticks_until_affordable,
        emissions_change_milli: emissions_change,
        global_emissions_change_milli: emissions_change.iter().sum(),
        finance_delta_change_milli: hypothetical.finance_delta_milli
            - sim.state.finance_delta_milli,
        mandate_delta_change_milli: hypothetical.mandate_delta_milli
            - sim.state.mandate_delta_milli,
        mandate_max_change_milli: hypothetical.mandate_max_milli - sim.state.mandate_max_milli,
        icons_after: hypothetical.icons,
        reduction_modifiers: bonus.modifiers,
        reduction_bonus_permille: bonus.bonus_permille,
        breakpoints_met: bonus.breakpoints_met,
        next_breakpoint: bonus.next_breakpoint,
        unlocks,
    })
}

fn affordable_in(have_milli: i64, need_milli: i64, delta_milli: i64) -> Option<u64> {
    let shortfall = need_milli - have_milli;
    if shortfall <= 0 {
        Some(0)
    } else if delta_milli <= 0 {
        None
    } else {
        Some(((shortfall + delta_milli - 1) / delta_milli) as u64)
    }
}
