//! The main dashboard: global totals and deltas, three continental summaries,
//! active programmes, the planning queue with its blocked-head explanation,
//! notifications and opportunities, available projects, and a context side
//! panel showing the selected project's projected preview and sources.
//!
//! The whole dynamic tree is rebuilt whenever the simulation ticks or the
//! player acts — an immediate-mode approach in retained clothing, cheap at
//! four ticks per second and impossible to desynchronise. Wording lives here;
//! every number comes from the simulation's state, calc traces, and previews.

use bevy::ecs::relationship::RelatedSpawnerCommands;
use bevy::prelude::*;
use nw_content::schema::Repeat;
use nw_simulation::{preview, BlockReason, Command, Continent, Icon, Sector, Sim, TraceKind};

use crate::{Game, UiState};

type Spawner<'w> = RelatedSpawnerCommands<'w, ChildOf>;

// Placeholder palette: flat, dark, colour never the only signal.
pub const BACKGROUND: Color = Color::srgb(0.06, 0.07, 0.09);
const PANEL: Color = Color::srgb(0.10, 0.11, 0.14);
const PANEL_SUNKEN: Color = Color::srgb(0.08, 0.09, 0.11);
const BUTTON: Color = Color::srgb(0.17, 0.19, 0.25);
const BUTTON_SELECTED: Color = Color::srgb(0.24, 0.30, 0.42);
const FG: Color = Color::srgb(0.87, 0.88, 0.90);
const DIM: Color = Color::srgb(0.55, 0.57, 0.62);
const EMISSIONS: Color = Color::srgb(0.95, 0.60, 0.40);
const FINANCE: Color = Color::srgb(0.55, 0.85, 0.55);
const MANDATE: Color = Color::srgb(0.55, 0.72, 0.95);
const WARN: Color = Color::srgb(0.95, 0.78, 0.40);
const GOOD: Color = Color::srgb(0.50, 0.95, 0.60);

/// Marker for the rebuildable dashboard root.
#[derive(Component)]
pub struct DynamicUi;

/// Every clickable maps to exactly one semantic intent.
#[derive(Component, Clone)]
pub enum Action {
    TogglePause,
    SelectProject(String),
    SetLead(Continent),
    QueueSelected,
    RemoveQueued(usize),
    MoveQueuedUp(usize),
    CancelActive(usize),
    ClaimOpportunity(usize),
}

// ------------------------------------------------------------- interaction

// Bevy query filters are inherently type-heavy; the standard allowance.
#[allow(clippy::type_complexity)]
pub fn handle_buttons(
    interactions: Query<(&Interaction, &Action), (Changed<Interaction>, With<Button>)>,
    mut game: ResMut<Game>,
    mut ui_state: ResMut<UiState>,
) {
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action.clone() {
            Action::TogglePause => {
                let command = if game.sim.state.paused {
                    Command::Resume
                } else {
                    Command::Pause
                };
                let _ = game.sim.execute(command);
            }
            Action::SelectProject(id) => ui_state.selected = Some(id),
            Action::SetLead(continent) => ui_state.lead = continent,
            Action::QueueSelected => {
                if let Some(project) = ui_state.selected.clone() {
                    let _ = game.sim.execute(Command::QueueProject {
                        project,
                        lead: ui_state.lead,
                    });
                }
            }
            Action::RemoveQueued(index) => {
                let _ = game.sim.execute(Command::RemoveQueuedProject {
                    index: index as u32,
                });
            }
            Action::MoveQueuedUp(index) => {
                let _ = game.sim.execute(Command::ReorderQueue {
                    from: index as u32,
                    to: index as u32 - 1,
                });
            }
            Action::CancelActive(index) => {
                let _ = game.sim.execute(Command::CancelActiveProject {
                    index: index as u32,
                });
            }
            Action::ClaimOpportunity(index) => {
                let _ = game.sim.execute(Command::ClaimOpportunity {
                    index: index as u32,
                });
            }
        }
        ui_state.dirty = true;
    }
}

// ---------------------------------------------------------------- rebuild

pub fn rebuild(
    mut commands: Commands,
    roots: Query<Entity, With<DynamicUi>>,
    game: Res<Game>,
    mut ui_state: ResMut<UiState>,
) {
    if !ui_state.dirty {
        return;
    }
    ui_state.dirty = false;
    for root in &roots {
        commands.entity(root).despawn();
    }

    let sim = &game.sim;
    let selected = ui_state.selected.clone();
    let lead = ui_state.lead;

    commands
        .spawn((
            DynamicUi,
            Node {
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(8.)),
                row_gap: Val::Px(8.),
                ..default()
            },
        ))
        .with_children(|root| {
            top_bar(root, sim);
            root.spawn(Node {
                flex_grow: 1.,
                column_gap: Val::Px(8.),
                overflow: Overflow::clip(),
                ..default()
            })
            .with_children(|middle| {
                continent_column(middle, sim);
                programme_column(middle, sim);
                project_column(middle, sim, selected.as_deref(), lead);
            });
        });
}

// ----------------------------------------------------------------- top bar

fn top_bar(parent: &mut Spawner, sim: &Sim) {
    let state = &sim.state;
    let baseline: i64 = state.baseline_emissions_milli.iter().flatten().sum();
    let now = state.total_emissions_milli();
    let speed = sim
        .catalogue()
        .scenario
        .authored_speed_ticks_per_second
        .max(1);
    let seconds = state.tick / u64::from(speed);

    parent
        .spawn((
            Node {
                width: Val::Percent(100.),
                padding: UiRect::all(Val::Px(10.)),
                column_gap: Val::Px(28.),
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(PANEL),
        ))
        .with_children(|bar| {
            stat(
                bar,
                "GLOBAL GROSS EMISSIONS",
                format!("{} GtCO2e/yr", gt(now)),
                EMISSIONS,
            );
            stat(
                bar,
                "REDUCED SO FAR",
                format!("{} Gt", gt(baseline - now)),
                GOOD,
            );
            stat(
                bar,
                "FINANCE",
                format!(
                    "{}  ({:+}/t)",
                    money(state.finance_milli),
                    money(state.finance_delta_milli)
                ),
                FINANCE,
            );
            stat(
                bar,
                "MANDATE",
                format!(
                    "{} / {}  ({:+}/t)",
                    money(state.mandate_milli),
                    money(state.mandate_max_milli),
                    money(state.mandate_delta_milli)
                ),
                MANDATE,
            );
            stat(
                bar,
                "PROGRAMME SLOTS",
                format!("{} of {}", state.active.len(), state.slots_total),
                FG,
            );
            stat(
                bar,
                "RANKED TIME",
                format!("{}:{:02}", seconds / 60, seconds % 60),
                FG,
            );

            if let Some(victory) = state.victory_tick {
                let at = victory / u64::from(speed);
                stat(
                    bar,
                    "VICTORY",
                    format!("zero at {}:{:02}", at / 60, at % 60),
                    GOOD,
                );
            }

            // Spacer, then pause — the only speed control that exists.
            bar.spawn(Node {
                flex_grow: 1.,
                ..default()
            });
            let label = if state.paused { "RESUME" } else { "PAUSE" };
            button(bar, label, Action::TogglePause, state.paused);
            if state.paused {
                bar.spawn(text("planning while paused", 12., WARN));
            }
        });
}

fn stat(parent: &mut Spawner, title: &str, value: String, color: Color) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.),
            ..default()
        })
        .with_children(|column| {
            column.spawn(text(title, 10., DIM));
            column.spawn(text(value, 16., color));
        });
}

// ----------------------------------------------------- continental column

fn continent_column(parent: &mut Spawner, sim: &Sim) {
    parent
        .spawn(Node {
            width: Val::Percent(24.),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.),
            ..default()
        })
        .with_children(|column| {
            for continent in Continent::ALL {
                continent_card(column, sim, continent);
            }
        });
}

fn continent_card(parent: &mut Spawner, sim: &Sim, continent: Continent) {
    let state = &sim.state;
    let c = continent.index();
    let total = state.continent_emissions_milli(continent);
    let leads_programme = state.active.iter().any(|build| build.lead == continent);

    parent
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.)),
                row_gap: Val::Px(4.),
                ..default()
            },
            BackgroundColor(PANEL),
        ))
        .with_children(|card| {
            card.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            })
            .with_children(|row| {
                row.spawn(text(continent.name(), 15., FG));
                row.spawn(text(format!("{} Gt", gt(total)), 15., EMISSIONS));
            });
            for sector in Sector::ALL {
                let rate = state.sector_emissions_milli[c][sector.index()];
                card.spawn(Node {
                    justify_content: JustifyContent::SpaceBetween,
                    ..default()
                })
                .with_children(|row| {
                    row.spawn(text(sector.name(), 12., DIM));
                    row.spawn(text(gt(rate), 12., if rate == 0 { GOOD } else { FG }));
                });
            }
            let icons = state.icons[c];
            card.spawn(text(
                format!(
                    "Kn {}  In {}  Wf {}  It {}",
                    icons[Icon::Knowledge.index()],
                    icons[Icon::Infrastructure.index()],
                    icons[Icon::Workforce.index()],
                    icons[Icon::Institutions.index()],
                ),
                12.,
                MANDATE,
            ));
            card.spawn(text(
                format!(
                    "+{} F/t   +{} M/t{}",
                    money(state.baseline_finance_income_milli[c]),
                    money(state.baseline_mandate_income_milli[c]),
                    if leads_programme {
                        "   * leading a programme"
                    } else {
                        ""
                    },
                ),
                11.,
                DIM,
            ));
        });
}

// ------------------------------------------------------ programme column

fn programme_column(parent: &mut Spawner, sim: &Sim) {
    parent
        .spawn(Node {
            width: Val::Percent(38.),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.),
            ..default()
        })
        .with_children(|column| {
            active_panel(column, sim);
            queue_panel(column, sim);
            notification_panel(column, sim);
        });
}

fn active_panel(parent: &mut Spawner, sim: &Sim) {
    let state = &sim.state;
    panel(parent, "ACTIVE PROGRAMMES", |body| {
        if state.active.is_empty() {
            body.spawn(text("none - the queue head starts when it can", 12., DIM));
        }
        for (index, build) in state.active.iter().enumerate() {
            let def = &sim.catalogue().projects[build.project as usize];
            let percent = build.progress * 100 / build.duration.max(1);
            body.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.),
                ..default()
            })
            .with_children(|row| {
                row.spawn(text(
                    format!("{} - {}", def.title, build.lead.name()),
                    13.,
                    FG,
                ));
                row.spawn(Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(6.),
                    ..default()
                })
                .with_children(|end| {
                    end.spawn(text(format!("{percent}%"), 13., FINANCE));
                    button(end, "cancel", Action::CancelActive(index), false);
                });
            });
        }
    });
}

fn queue_panel(parent: &mut Spawner, sim: &Sim) {
    let state = &sim.state;
    panel(parent, "PLANNING QUEUE (strict FIFO)", |body| {
        if state.queue.is_empty() {
            body.spawn(text("empty - queue projects from the right", 12., DIM));
        }
        for (index, entry) in state.queue.iter().enumerate() {
            let def = &sim.catalogue().projects[entry.project as usize];
            body.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|row| {
                row.spawn(text(
                    format!("{}. {} - {}", index + 1, def.title, entry.lead.name()),
                    13.,
                    FG,
                ));
                row.spawn(Node {
                    column_gap: Val::Px(4.),
                    ..default()
                })
                .with_children(|end| {
                    if index > 0 {
                        button(end, "up", Action::MoveQueuedUp(index), false);
                    }
                    button(end, "x", Action::RemoveQueued(index), false);
                });
            });
            if index == 0 {
                if let Some(reason) = &state.last_block {
                    body.spawn(text(
                        format!("   waiting: {}", block_text(reason)),
                        12.,
                        WARN,
                    ));
                }
            }
        }
    });
}

fn notification_panel(parent: &mut Spawner, sim: &Sim) {
    let state = &sim.state;
    panel(parent, "OPPORTUNITIES & EVENTS", |body| {
        for (index, open) in state.opportunities.iter().enumerate() {
            let def = &sim.catalogue().opportunities[open.def as usize];
            body.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|row| {
                row.spawn(text(
                    format!(
                        "{} ({}) - expires in {}t",
                        def.title,
                        open.continent.name(),
                        open.expires.saturating_sub(state.tick)
                    ),
                    12.,
                    GOOD,
                ));
                button(row, "claim", Action::ClaimOpportunity(index), false);
            });
        }
        for event in state.trace.iter().rev().take(8) {
            body.spawn(text(
                format!("t{}  {}", event.tick, trace_text(&event.kind)),
                11.,
                DIM,
            ));
        }
    });
}

// -------------------------------------------------------- project column

fn project_column(parent: &mut Spawner, sim: &Sim, selected: Option<&str>, lead: Continent) {
    parent
        .spawn(Node {
            width: Val::Percent(38.),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.),
            ..default()
        })
        .with_children(|column| {
            available_panel(column, sim, selected);
            if let Some(id) = selected {
                detail_panel(column, sim, id, lead);
            } else {
                panel(column, "PROJECT DETAIL", |body| {
                    body.spawn(text("select a project to preview its outcomes", 12., DIM));
                });
            }
        });
}

fn available_panel(parent: &mut Spawner, sim: &Sim, selected: Option<&str>) {
    panel(parent, "AVAILABLE PROJECTS", |body| {
        for (index, def) in sim.catalogue().projects.iter().enumerate() {
            if !sim.state.unlocked[index] {
                continue;
            }
            let (cost, _) = sim.current_cost_milli(index, Continent::Europe);
            let is_selected = selected == Some(def.id.as_str());
            body.spawn((
                Button,
                Node {
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::axes(Val::Px(6.), Val::Px(2.)),
                    ..default()
                },
                BackgroundColor(if is_selected {
                    BUTTON_SELECTED
                } else {
                    PANEL_SUNKEN
                }),
                Action::SelectProject(def.id.clone()),
            ))
            .with_children(|row| {
                row.spawn(text(&def.title, 12., FG));
                row.spawn(text(
                    format!("{} F{}", money(cost), repeat_tag(def.repeat)),
                    12.,
                    DIM,
                ));
            });
        }
    });
}

fn detail_panel(parent: &mut Spawner, sim: &Sim, id: &str, lead: Continent) {
    let Some(projected) = preview(sim, id, lead) else {
        return;
    };
    let Some(def) = sim.catalogue().projects.iter().find(|p| p.id == id) else {
        return;
    };

    panel(parent, "PROJECT DETAIL", |body| {
        body.spawn(text(&def.title, 15., FG));
        body.spawn(text(&def.summary, 12., DIM));

        // Lead continent selection: authored choice, no recommendation.
        body.spawn(Node {
            column_gap: Val::Px(6.),
            align_items: AlignItems::Center,
            margin: UiRect::vertical(Val::Px(4.)),
            ..default()
        })
        .with_children(|row| {
            row.spawn(text("lead:", 12., DIM));
            for continent in Continent::ALL {
                button(
                    row,
                    continent.name(),
                    Action::SetLead(continent),
                    continent == lead,
                );
            }
            button(row, "QUEUE", Action::QueueSelected, false);
        });

        // Guaranteed direct effects, from the same calc the sim will run.
        let wait = match projected.ticks_until_affordable {
            Some(0) => "affordable now".to_string(),
            Some(ticks) => format!("affordable in ~{ticks}t"),
            None => "income cannot currently cover this".to_string(),
        };
        body.spawn(text(
            format!(
                "cost {} F + {} M   duration {}t   {}",
                money(projected.finance_cost_milli),
                money(projected.mandate_cost_milli),
                projected.duration_ticks,
                wait
            ),
            12.,
            FG,
        ));
        for modifier in &projected.cost_trace.modifiers {
            if modifier.permille != 0 {
                body.spawn(text(
                    format!("   cost {} {:+} per 1000", modifier.name, modifier.permille),
                    11.,
                    DIM,
                ));
            }
        }
        if projected.global_emissions_change_milli != 0 {
            body.spawn(text(
                format!(
                    "direct: {} Gt global on completion",
                    gt(projected.global_emissions_change_milli)
                ),
                12.,
                EMISSIONS,
            ));
            for continent in Continent::ALL {
                let change = projected.emissions_change_milli[continent.index()];
                if change != 0 {
                    body.spawn(text(
                        format!("   {} {} Gt", continent.name(), gt(change)),
                        11.,
                        DIM,
                    ));
                }
            }
        }
        for modifier in &projected.reduction_modifiers {
            body.spawn(text(
                format!("   {} {:+} per 1000", modifier.name, modifier.permille),
                11.,
                MANDATE,
            ));
        }
        if projected.finance_delta_change_milli != 0 || projected.mandate_delta_change_milli != 0 {
            body.spawn(text(
                format!(
                    "ongoing: {:+} F/t   {:+} M/t",
                    money(projected.finance_delta_change_milli),
                    money(projected.mandate_delta_change_milli)
                ),
                12.,
                FINANCE,
            ));
        }

        // Conditional consequences, separated per the preview contract.
        if let Some(next) = &projected.next_breakpoint {
            body.spawn(text(
                format!(
                    "next breakpoint: {} at {} {} ({:+} per 1000)",
                    next.id,
                    next.at_least,
                    next.icon.name(),
                    next.bonus_permille
                ),
                11.,
                WARN,
            ));
        }
        for unlock in &projected.unlocks {
            body.spawn(text(format!("unlocks: {unlock}"), 11., WARN));
        }

        // Context: evidence, then abstraction, kept apart per the guardrails.
        body.spawn(text(format!("what: {}", def.context.what), 11., DIM));
        body.spawn(text(
            format!("in this scenario: {}", def.context.in_this_scenario),
            11.,
            DIM,
        ));
        body.spawn(text(
            format!("abstraction: {}", def.context.abstraction),
            11.,
            DIM,
        ));
        body.spawn(text(
            format!("sources: {}", def.context.sources.join("; ")),
            11.,
            DIM,
        ));
    });
}

// ------------------------------------------------------------- primitives

fn panel(parent: &mut Spawner, title: &str, body: impl FnOnce(&mut Spawner)) {
    parent
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.)),
                row_gap: Val::Px(4.),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(PANEL),
        ))
        .with_children(|content| {
            content.spawn(text(title, 11., DIM));
            body(content);
        });
}

fn button(parent: &mut Spawner, label: &str, action: Action, highlighted: bool) {
    parent
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(8.), Val::Px(3.)),
                ..default()
            },
            BackgroundColor(if highlighted { BUTTON_SELECTED } else { BUTTON }),
            action,
        ))
        .with_children(|inner| {
            inner.spawn(text(label, 12., FG));
        });
}

fn text(value: impl Into<String>, size: f32, color: Color) -> (Text, TextFont, TextColor) {
    (
        Text::new(value.into()),
        TextFont {
            font_size: size,
            ..default()
        },
        TextColor(color),
    )
}

// -------------------------------------------------------------- wording

fn gt(milli: i64) -> String {
    format!("{:.1}", milli as f64 / 1000.0)
}

fn money(milli: i64) -> String {
    let value = milli as f64 / 1000.0;
    if value.fract().abs() < f64::EPSILON {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}

fn repeat_tag(repeat: Repeat) -> &'static str {
    match repeat {
        Repeat::Unique => "  (unique)",
        Repeat::Repeatable => "",
        Repeat::Tiered { .. } => "  (tiered)",
        Repeat::CappedRollout { .. } => "  (capped)",
    }
}

fn block_text(reason: &BlockReason) -> String {
    match reason {
        BlockReason::NoFreeSlot => "no free programme slot".into(),
        BlockReason::NotUnlocked => "not yet unlocked".into(),
        BlockReason::MissingIcon { icon, needed, have } => {
            format!(
                "needs {} {} on the lead continent (has {})",
                needed,
                icon.name(),
                have
            )
        }
        BlockReason::MissingProject { project } => {
            format!("requires {project} completed first")
        }
        BlockReason::RepeatLimitReached => "repeat limit reached".into(),
        BlockReason::InsufficientFinance {
            needed_milli,
            have_milli,
        } => {
            format!(
                "needs {} Finance (has {})",
                money(*needed_milli),
                money(*have_milli)
            )
        }
        BlockReason::InsufficientMandate {
            needed_milli,
            have_milli,
        } => {
            format!(
                "needs {} Mandate (has {})",
                money(*needed_milli),
                money(*have_milli)
            )
        }
    }
}

fn trace_text(kind: &TraceKind) -> String {
    match kind {
        TraceKind::ProjectStarted { project, lead } => {
            format!("started {} - {}", project, lead.name())
        }
        TraceKind::ProjectCompleted {
            project,
            lead,
            bonus_permille,
        } => {
            if *bonus_permille > 0 {
                format!(
                    "completed {} - {} ({:+} per 1000)",
                    project,
                    lead.name(),
                    bonus_permille
                )
            } else {
                format!("completed {} - {}", project, lead.name())
            }
        }
        TraceKind::ProjectCancelled { project, lead } => {
            format!("cancelled {} - {}", project, lead.name())
        }
        TraceKind::ProjectDecommissioned { project, lead } => {
            format!("decommissioned {} - {}", project, lead.name())
        }
        TraceKind::SlotUnlocked { total } => {
            format!("programme slot unlocked - {total} total")
        }
        TraceKind::OpportunityOpened { id, continent } => {
            format!("opportunity: {} ({})", id, continent.name())
        }
        TraceKind::OpportunityClaimed { id } => format!("claimed {id}"),
        TraceKind::OpportunityExpired { id } => format!("{id} lapsed"),
        TraceKind::QueueBlocked { reason } => format!("queue waiting: {}", block_text(reason)),
        TraceKind::VictoryReached => "GROSS EMISSIONS REACHED ZERO".into(),
    }
}
