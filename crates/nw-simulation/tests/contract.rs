//! The simulation contract's test obligations: determinism, RNG isolation,
//! lifecycle cleanup, queue FIFO with blocked-head explanations, slot
//! milestones, scaling order, preview-equals-realised, pause timing, and
//! recoverability. Replay and build-order obligations live in nw-headless.

use nw_content::world::{Continent, Icon};
use nw_content::Catalogue;
use nw_simulation::{preview, BlockReason, Command, Sim};

fn sim(seed: u64) -> Sim {
    Sim::new(Catalogue::embedded(), seed)
}

fn run(sim: &mut Sim, ticks: u64) {
    for _ in 0..ticks {
        sim.tick();
    }
}

fn queue(sim: &mut Sim, project: &str, lead: Continent) {
    sim.execute(Command::QueueProject {
        project: project.into(),
        lead,
    })
    .expect("queueing a known project");
}

// ------------------------------------------------------------- determinism

#[test]
fn same_seed_and_commands_reach_identical_state() {
    let script = |sim: &mut Sim| {
        queue(sim, "wind-solar-deployment", Continent::Europe);
        run(sim, 400);
        queue(sim, "transit-and-rail", Continent::MajorityWorld);
        run(sim, 400);
    };
    let mut a = sim(42);
    let mut b = sim(42);
    script(&mut a);
    script(&mut b);
    assert_eq!(a.state, b.state);
    assert_eq!(a.log, b.log);
}

#[test]
fn different_seeds_vary_starting_conditions_within_bounds() {
    let a = sim(1);
    let b = sim(2);
    assert_ne!(
        a.state.baseline_emissions_milli,
        b.state.baseline_emissions_milli
    );
    // Variation stays within the authored permille band.
    let catalogue = Catalogue::embedded();
    for (continent, sector, base) in &catalogue.scenario.sector_baselines_milli_gt {
        let varied = a.state.baseline_emissions_milli[continent.index()][sector.index()];
        let bound = base * catalogue.scenario.starting_variation_permille / 1000 + 1;
        assert!((varied - base).abs() <= bound);
    }
}

// ---------------------------------------------------------- recoverability

#[test]
fn baseline_deltas_are_strictly_positive() {
    let s = sim(7);
    assert!(s.state.finance_delta_milli > 0);
    assert!(s.state.mandate_delta_milli > 0);
}

#[test]
fn rejected_commands_change_nothing() {
    let mut s = sim(7);
    let before = s.state.clone();
    assert!(s.execute(Command::ClaimOpportunity { index: 0 }).is_err());
    assert!(s
        .execute(Command::QueueProject {
            project: "no-such-project".into(),
            lead: Continent::Europe,
        })
        .is_err());
    assert_eq!(s.state, before);
    // Both rejections are in the log with reasons.
    assert_eq!(s.log.len(), 2);
    assert!(s.log.iter().all(|entry| !entry.accepted()));
}

// ------------------------------------------------------------ pause timing

#[test]
fn paused_ticks_advance_nothing_but_planning_stays_available() {
    let mut s = sim(7);
    run(&mut s, 50);
    s.execute(Command::Pause).unwrap();
    let frozen = s.state.clone();
    run(&mut s, 200);
    // Ranked time, stocks, and construction are all frozen.
    assert_eq!(s.state.tick, frozen.tick);
    assert_eq!(s.state.finance_milli, frozen.finance_milli);
    // Planning while paused is allowed.
    queue(&mut s, "wind-solar-deployment", Continent::Europe);
    assert_eq!(s.state.queue.len(), 1);
    s.execute(Command::Resume).unwrap();
    run(&mut s, 1);
    assert_eq!(s.state.tick, frozen.tick + 1);
}

#[test]
fn pause_commands_reject_when_redundant() {
    let mut s = sim(7);
    assert!(s.execute(Command::Resume).is_err());
    s.execute(Command::Pause).unwrap();
    assert!(s.execute(Command::Pause).is_err());
}

// ------------------------------------------------- queue FIFO and blocking

#[test]
fn blocked_head_stalls_fifo_and_explains_why() {
    let mut s = sim(7);
    // Nuclear costs 1500 units; the run starts with 800.
    queue(&mut s, "nuclear-fleet", Continent::Europe);
    queue(&mut s, "methane-and-land-programme", Continent::Europe);
    run(&mut s, 1);
    assert!(matches!(
        s.state.last_block,
        Some(BlockReason::InsufficientFinance { .. })
    ));
    // Strict FIFO: the affordable second entry does not jump the queue.
    assert!(s.state.active.is_empty());
    assert_eq!(s.state.queue.len(), 2);
    // Positive income means the head eventually starts — recoverability.
    run(&mut s, 200);
    assert_eq!(s.state.active.len(), 1);
    assert_eq!(s.state.queue.len(), 1);
    // The new head now stalls on the occupied slot, and says so.
    assert_eq!(s.state.last_block, Some(BlockReason::NoFreeSlot));
}

#[test]
fn locked_projects_block_until_unlocked() {
    let mut s = sim(7);
    queue(&mut s, "industrial-deep-decarbonisation", Continent::Europe);
    run(&mut s, 1);
    assert_eq!(s.state.last_block, Some(BlockReason::NotUnlocked));
}

#[test]
fn unique_projects_cannot_repeat() {
    let mut s = sim(7);
    queue(&mut s, "industrial-efficiency-standards", Continent::Europe);
    run(&mut s, 200);
    assert_eq!(s.state.completed.len(), 1);
    queue(&mut s, "industrial-efficiency-standards", Continent::Europe);
    run(&mut s, 1);
    assert_eq!(s.state.last_block, Some(BlockReason::RepeatLimitReached));
}

// ------------------------------------------------------ lifecycle cleanup

#[test]
fn cancellation_destroys_progress_gives_no_refund_frees_the_slot() {
    let mut s = sim(7);
    queue(&mut s, "wind-solar-deployment", Continent::Europe);
    run(&mut s, 50);
    assert_eq!(s.state.active.len(), 1);
    let finance_before_cancel = s.state.finance_milli;
    s.execute(Command::CancelActiveProject { index: 0 })
        .unwrap();
    assert!(s.state.active.is_empty());
    // No refund.
    assert_eq!(s.state.finance_milli, finance_before_cancel);
    // No lingering effects.
    assert_eq!(
        s.state.sector_emissions_milli,
        s.state.baseline_emissions_milli
    );
}

#[test]
fn cancellation_removes_partial_rollout_effects() {
    let mut s = sim(7);
    // Building retrofits roll out linearly during construction.
    queue(&mut s, "building-retrofit-programme", Continent::Europe);
    run(&mut s, 100);
    assert!(
        s.state.total_emissions_milli() < s.state.baseline_emissions_milli.iter().flatten().sum()
    );
    s.execute(Command::CancelActiveProject { index: 0 })
        .unwrap();
    assert_eq!(
        s.state.sector_emissions_milli,
        s.state.baseline_emissions_milli
    );
}

#[test]
fn decommission_removes_benefits_icons_and_upkeep_without_refund() {
    let mut s = sim(7);
    queue(&mut s, "wind-solar-deployment", Continent::Europe);
    run(&mut s, 200);
    assert_eq!(s.state.completed.len(), 1);
    assert!(
        s.state.total_emissions_milli() < s.state.baseline_emissions_milli.iter().flatten().sum()
    );
    let finance_before = s.state.finance_milli;
    s.execute(Command::DecommissionProject { index: 0 })
        .unwrap();
    // Benefits, icons, spillovers gone; no refund.
    assert_eq!(
        s.state.sector_emissions_milli,
        s.state.baseline_emissions_milli
    );
    assert_eq!(s.state.icons, [[0; 4]; 3]);
    assert_eq!(s.state.finance_milli, finance_before);
    // The cost curve still counts the completion: rebuilding is repriced.
    assert_eq!(
        s.state.completions_ever[Catalogue::embedded()
            .project_index("wind-solar-deployment")
            .unwrap()],
        1
    );
}

// -------------------------------------------------------- slot milestones

#[test]
fn institutions_milestones_unlock_programme_slots_and_ratchet() {
    let mut s = sim(7);
    assert_eq!(s.state.slots_total, 1);
    s.state.bonus_icons[Continent::MajorityWorld.index()][Icon::Institutions.index()] = 2;
    s.rederive();
    assert_eq!(s.state.slots_total, 2);
    s.state.bonus_icons[Continent::MajorityWorld.index()][Icon::Institutions.index()] = 6;
    s.rederive();
    assert_eq!(s.state.slots_total, 3);
    // Slots ratchet: losing the icons does not remove the slot.
    s.state.bonus_icons[Continent::MajorityWorld.index()][Icon::Institutions.index()] = 0;
    s.rederive();
    assert_eq!(s.state.slots_total, 3);
}

// ------------------------------------------- scaling, breakpoints, preview

#[test]
fn scaling_applies_before_breakpoints_and_respects_caps() {
    let mut s = sim(7);
    // 10 Infrastructure would scale 800 permille but caps at 600; 4 Workforce
    // meets the breakpoint for +150.
    s.state.bonus_icons[Continent::Europe.index()][Icon::Infrastructure.index()] = 10;
    s.state.bonus_icons[Continent::Europe.index()][Icon::Workforce.index()] = 4;
    s.rederive();
    let p = preview(&s, "wind-solar-deployment", Continent::Europe).unwrap();
    assert_eq!(p.reduction_bonus_permille, 750);
    // Documented order: scaling rules first, then breakpoints.
    assert_eq!(p.reduction_modifiers.len(), 2);
    assert!(p.reduction_modifiers[0].name.starts_with("scaling:"));
    assert!(p.reduction_modifiers[1].name.starts_with("breakpoint:"));
    assert_eq!(
        p.breakpoints_met,
        vec!["skilled-installation-crews".to_string()]
    );
    // Direct effect: 600 base * 1.75, on the lead continent only.
    assert_eq!(p.emissions_change_milli[Continent::Europe.index()], -1050);
    assert_eq!(p.emissions_change_milli[Continent::NorthAmerica.index()], 0);
}

#[test]
fn preview_equals_realised_effect_under_unchanged_conditions() {
    let mut s = sim(11);
    let p = preview(&s, "wind-solar-deployment", Continent::MajorityWorld).unwrap();
    let emissions_before = s.state.sector_emissions_milli;
    let finance_before = s.state.finance_milli;

    queue(&mut s, "wind-solar-deployment", Continent::MajorityWorld);
    run(&mut s, 1); // head starts, cost paid
    let paid = finance_before + s.state.finance_delta_milli - s.state.finance_milli;
    assert_eq!(paid, p.finance_cost_milli);

    run(&mut s, p.duration_ticks);
    assert_eq!(s.state.completed.len(), 1, "project should have completed");
    for continent in Continent::ALL {
        let realised: i64 = s.state.sector_emissions_milli[continent.index()]
            .iter()
            .sum::<i64>()
            - emissions_before[continent.index()].iter().sum::<i64>();
        assert_eq!(realised, p.emissions_change_milli[continent.index()]);
    }
    assert_eq!(s.state.icons, p.icons_after);
}

#[test]
fn preview_reports_time_until_affordable() {
    let s = sim(7);
    // Nuclear costs more than starting finance; the wait must match income.
    let p = preview(&s, "nuclear-fleet", Continent::Europe).unwrap();
    let wait = p.ticks_until_affordable.unwrap();
    assert!(wait > 0);
    let shortfall = p.finance_cost_milli - s.state.finance_milli;
    let delta = s.state.finance_delta_milli;
    assert_eq!(wait, ((shortfall + delta - 1) / delta) as u64);
    // Methane is affordable immediately.
    let p = preview(&s, "methane-and-land-programme", Continent::Europe).unwrap();
    assert_eq!(p.ticks_until_affordable, Some(0));
}

#[test]
fn shared_scope_effects_reach_every_continent() {
    let mut s = sim(7);
    let p = preview(&s, "industrial-efficiency-standards", Continent::Europe).unwrap();
    for continent in Continent::ALL {
        assert_eq!(p.emissions_change_milli[continent.index()], -300);
    }
    assert_eq!(
        p.unlocks,
        vec!["industrial-deep-decarbonisation".to_string()]
    );
    // And realised: completing it unlocks the locked project everywhere.
    queue(&mut s, "industrial-efficiency-standards", Continent::Europe);
    run(&mut s, 200);
    let deep = Catalogue::embedded()
        .project_index("industrial-deep-decarbonisation")
        .unwrap();
    assert!(s.state.unlocked[deep]);
}

// ------------------------------------------------------------ opportunities

#[test]
fn opportunities_spawn_deterministically_and_claims_apply() {
    let mut a = sim(21);
    let mut b = sim(21);
    run(&mut a, 1000);
    run(&mut b, 1000);
    assert_eq!(a.state.opportunities, b.state.opportunities);
    if !a.state.opportunities.is_empty() {
        let finance = a.state.finance_milli;
        let mandate = a.state.mandate_milli;
        let icons = a.state.icons;
        a.execute(Command::ClaimOpportunity { index: 0 }).unwrap();
        let changed = a.state.finance_milli != finance
            || a.state.mandate_milli != mandate
            || a.state.icons != icons;
        assert!(changed, "a claim must apply its one-shot effect");
    }
}
