//! Two deterministic bot strategies. Both must reach victory for the economy
//! to pass its Stage 1 gate: `DeployFirst` chases direct reductions and only
//! builds capacity opportunistically; `CapacityFirst` builds enabling projects,
//! institutions, and programme slots before mass deployment. They pick
//! candidates through the same projected-outcome previews a player sees.

use nw_content::world::{Continent, Sector};
use nw_persistence::Runner;
use nw_simulation::{preview, Command, Sim};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strategy {
    DeployFirst,
    CapacityFirst,
}

impl Strategy {
    pub fn parse(text: &str) -> Option<Strategy> {
        match text {
            "deploy-first" => Some(Strategy::DeployFirst),
            "capacity-first" => Some(Strategy::CapacityFirst),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Strategy::DeployFirst => "deploy-first",
            Strategy::CapacityFirst => "capacity-first",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Outcome {
    pub victory_tick: Option<u64>,
    pub final_tick: u64,
}

/// Play a full run: claim every opportunity, keep one project queued, tick.
pub fn play(runner: &mut Runner, strategy: Strategy, max_ticks: u64) -> Outcome {
    loop {
        if runner.sim.state.victory_tick.is_some() || runner.sim.state.tick >= max_ticks {
            break;
        }
        while !runner.sim.state.opportunities.is_empty() {
            runner
                .sim
                .execute(Command::ClaimOpportunity { index: 0 })
                .expect("claiming the first open opportunity");
        }
        if runner.sim.state.queue.is_empty() {
            if let Some((project, lead)) = next_pick(&runner.sim, strategy) {
                runner
                    .sim
                    .execute(Command::QueueProject { project, lead })
                    .expect("queueing a known project");
            }
        }
        runner.tick();
    }
    Outcome {
        victory_tick: runner.sim.state.victory_tick,
        final_tick: runner.sim.state.tick,
    }
}

fn completed_on(sim: &Sim, id: &str, lead: Continent) -> u32 {
    sim.state
        .completed
        .iter()
        .filter(|done| {
            sim.catalogue().projects[done.project as usize].id == id && done.lead == lead
        })
        .count() as u32
}

fn completed_anywhere(sim: &Sim, id: &str) -> u32 {
    sim.state
        .completed
        .iter()
        .filter(|done| sim.catalogue().projects[done.project as usize].id == id)
        .count() as u32
}

fn in_flight(sim: &Sim, id: &str, lead: Option<Continent>) -> bool {
    let matches = |project: u32, at: Continent| {
        sim.catalogue().projects[project as usize].id == id && lead.map(|l| l == at).unwrap_or(true)
    };
    sim.state.active.iter().any(|b| matches(b.project, b.lead))
        || sim.state.queue.iter().any(|q| matches(q.project, q.lead))
}

fn ever(sim: &Sim, id: &str) -> u32 {
    sim.catalogue()
        .project_index(id)
        .map(|index| sim.state.completions_ever[index])
        .unwrap_or(0)
}

fn icons(sim: &Sim, lead: Continent, icon: nw_content::world::Icon) -> i64 {
    sim.state.icons[lead.index()][icon.index()]
}

/// The continent with the most remaining emissions.
fn worst_continent(sim: &Sim) -> Continent {
    *Continent::ALL
        .iter()
        .max_by_key(|c| sim.state.continent_emissions_milli(**c))
        .expect("three continents")
}

/// Choose what to queue next, or `None` when nothing remains worth queueing.
fn next_pick(sim: &Sim, strategy: Strategy) -> Option<(String, Continent)> {
    use nw_content::world::Icon;

    // Capacity-first front-loads enablers, institutions, and slots.
    if strategy == Strategy::CapacityFirst {
        for continent in Continent::ALL {
            if completed_on(sim, "grid-modernisation", continent) == 0
                && !in_flight(sim, "grid-modernisation", Some(continent))
            {
                return Some(("grid-modernisation".into(), continent));
            }
        }
        for continent in Continent::ALL {
            if completed_on(sim, "clean-manufacturing-scaleup", continent) == 0
                && !in_flight(sim, "clean-manufacturing-scaleup", Some(continent))
            {
                return Some(("clean-manufacturing-scaleup".into(), continent));
            }
        }
        if ever(sim, "cfa-regional-secretariats") < 3
            && !in_flight(sim, "cfa-regional-secretariats", None)
        {
            return Some(("cfa-regional-secretariats".into(), worst_continent(sim)));
        }
        if completed_anywhere(sim, "industrial-efficiency-standards") == 0
            && !in_flight(sim, "industrial-efficiency-standards", None)
        {
            return Some((
                "industrial-efficiency-standards".into(),
                worst_continent(sim),
            ));
        }
    }

    // Deploy-first builds coordination capacity only out of surplus cash.
    if strategy == Strategy::DeployFirst
        && sim.state.finance_milli > 6_000_000
        && ever(sim, "cfa-regional-secretariats") < 3
        && !in_flight(sim, "cfa-regional-secretariats", None)
    {
        return Some(("cfa-regional-secretariats".into(), worst_continent(sim)));
    }

    // Deploy phase: target the worst remaining (continent, sector).
    let mut targets: Vec<(Continent, Sector, i64)> = Vec::new();
    for continent in Continent::ALL {
        for sector in Sector::ALL {
            let remaining = sim.state.sector_emissions_milli[continent.index()][sector.index()];
            if remaining > 0 {
                targets.push((continent, sector, remaining));
            }
        }
    }
    let (continent, sector, _) = *targets.iter().max_by_key(|(_, _, left)| *left)?;

    match sector {
        Sector::Power => Some(("wind-solar-deployment".into(), continent)),
        Sector::TransportAndBuildings => {
            // Compare the always-available options through their previews and
            // take the cheapest reduction actually startable.
            let mut candidates = vec!["transit-and-rail", "building-retrofit-programme"];
            if completed_on(sim, "grid-modernisation", continent) > 0
                && icons(sim, continent, Icon::Infrastructure) >= 3
                && sim.state.mandate_milli >= 6000
            {
                candidates.push("electrification-programme");
            }
            let best = candidates.into_iter().min_by_key(|id| {
                preview(sim, id, continent)
                    .map(|p| {
                        let reduction = (-p.global_emissions_change_milli).max(1);
                        p.finance_cost_milli * 1000 / reduction
                    })
                    .unwrap_or(i64::MAX)
            })?;
            Some((best.into(), continent))
        }
        Sector::IndustryAndLand => {
            if completed_on(sim, "methane-and-land-programme", continent) < 2
                && !in_flight(sim, "methane-and-land-programme", Some(continent))
            {
                return Some(("methane-and-land-programme".into(), continent));
            }
            if completed_anywhere(sim, "industrial-efficiency-standards") == 0 {
                if in_flight(sim, "industrial-efficiency-standards", None) {
                    // Standards are on their way; deploy elsewhere meanwhile.
                    return Some(("wind-solar-deployment".into(), worst_continent(sim)));
                }
                return Some(("industrial-efficiency-standards".into(), continent));
            }
            if icons(sim, continent, Icon::Knowledge) < 3 {
                if completed_on(sim, "clean-manufacturing-scaleup", continent) == 0
                    && !in_flight(sim, "clean-manufacturing-scaleup", Some(continent))
                {
                    return Some(("clean-manufacturing-scaleup".into(), continent));
                }
                // Knowledge is building; deploy elsewhere meanwhile.
                return Some(("wind-solar-deployment".into(), worst_continent(sim)));
            }
            Some(("industrial-deep-decarbonisation".into(), continent))
        }
    }
}
