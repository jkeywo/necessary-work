//! The main dashboard: global totals and deltas, three continental summaries,
//! active programmes, the planning queue with its blocked-head explanation,
//! notifications and opportunities, available projects, and a context side
//! panel showing the selected project's projected preview and sources.
//!
//! The whole dynamic tree is rebuilt whenever the simulation ticks or the
//! player acts — an immediate-mode approach in retained clothing, cheap at
//! four ticks per second and impossible to desynchronise. Wording lives here;
//! every number comes from the simulation's state, calc traces, and previews.
//!
//! Everything hoverable carries a [`Tooltip`]; a cursor-following tooltip
//! shows the most specific (smallest) hovered explanation. Quantities carry
//! icon chips — small coloured badges standing in for final icon art — and
//! Finance is always shown to two decimal places.

use bevy::ecs::relationship::RelatedSpawnerCommands;
use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy::window::PrimaryWindow;
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
const TOOLTIP_BG: Color = Color::srgb(0.16, 0.18, 0.24);
const CHIP_TEXT: Color = Color::srgb(0.05, 0.06, 0.08);
const FG: Color = Color::srgb(0.87, 0.88, 0.90);
const DIM: Color = Color::srgb(0.55, 0.57, 0.62);
const EMISSIONS: Color = Color::srgb(0.95, 0.60, 0.40);
const FINANCE: Color = Color::srgb(0.55, 0.85, 0.55);
const MANDATE: Color = Color::srgb(0.55, 0.72, 0.95);
const KNOWLEDGE: Color = Color::srgb(0.75, 0.58, 0.95);
const INFRASTRUCTURE: Color = Color::srgb(0.45, 0.80, 0.80);
const WORKFORCE: Color = Color::srgb(0.92, 0.72, 0.38);
const INSTITUTIONS: Color = Color::srgb(0.63, 0.68, 0.88);
const WARN: Color = Color::srgb(0.95, 0.78, 0.40);
const GOOD: Color = Color::srgb(0.50, 0.95, 0.60);

/// Marker for the rebuildable dashboard root.
#[derive(Component)]
pub struct DynamicUi;

/// Marker for the floating tooltip.
#[derive(Component)]
pub struct TooltipUi;

/// Marker for the floating tooltip's text child.
#[derive(Component)]
pub struct TooltipText;

/// The tooltip's sticky state. The dashboard rebuilds its whole tree on every
/// simulation tick, and a freshly spawned node is not re-marked hovered until
/// the next focus pass — so "nothing hovered" for a frame or two does not mean
/// the cursor moved away. The grace period bridges that gap.
#[derive(Resource, Default)]
pub struct TooltipState {
    text: String,
    grace_frames: u8,
}

/// Hover text explaining the thing under the cursor. Attach beside an
/// `Interaction` so the focus system tracks hovering.
#[derive(Component)]
pub struct Tooltip(pub String);

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

/// The placeholder icon chips: one per icon family, plus the three core
/// quantities. Rendered as coloured badges after their numbers.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Chip {
    Knowledge,
    Infrastructure,
    Workforce,
    Institutions,
    Emissions,
    Finance,
    Mandate,
}

impl Chip {
    fn for_icon(icon: Icon) -> Chip {
        match icon {
            Icon::Knowledge => Chip::Knowledge,
            Icon::Infrastructure => Chip::Infrastructure,
            Icon::Workforce => Chip::Workforce,
            Icon::Institutions => Chip::Institutions,
        }
    }

    fn letter(self) -> &'static str {
        match self {
            Chip::Knowledge => "K",
            Chip::Infrastructure => "I",
            Chip::Workforce => "W",
            Chip::Institutions => "T",
            Chip::Emissions => "E",
            Chip::Finance => "$",
            Chip::Mandate => "M",
        }
    }

    fn color(self) -> Color {
        match self {
            Chip::Knowledge => KNOWLEDGE,
            Chip::Infrastructure => INFRASTRUCTURE,
            Chip::Workforce => WORKFORCE,
            Chip::Institutions => INSTITUTIONS,
            Chip::Emissions => EMISSIONS,
            Chip::Finance => FINANCE,
            Chip::Mandate => MANDATE,
        }
    }

    /// Resources are round; installed capacities are square.
    fn round(self) -> bool {
        matches!(self, Chip::Emissions | Chip::Finance | Chip::Mandate)
    }

    fn explain(self) -> &'static str {
        match self {
            Chip::Knowledge => {
                "Knowledge: installed research, demonstration, standards, and institutional \
                 learning on one continent. An installed capacity, not a spendable resource - \
                 projects use it for prerequisites, scaling, and breakpoints."
            }
            Chip::Infrastructure => {
                "Infrastructure: installed grids, networks, manufacturing, and supply chains \
                 on one continent. An installed capacity, not a spendable resource - it \
                 scales deployment rather than being consumed."
            }
            Chip::Workforce => {
                "Workforce: the trained labour installed on one continent. An installed \
                 capacity, not a spendable resource - retrofit and deployment projects \
                 scale with it."
            }
            Chip::Institutions => {
                "Institutions: planning, permitting, and coordination capacity installed on \
                 one continent. Reaching Institutions milestones on any continent unlocks \
                 additional global programme slots."
            }
            Chip::Emissions => {
                "Gross greenhouse-gas emissions, in GtCO2e per year. Sector baselines only \
                 change through projects. Victory: every sector at zero with non-negative \
                 Finance and Mandate income."
            }
            Chip::Finance => {
                "Finance: one global stock of abstract CFA units with no maximum. Each \
                 continent's contribution is visible, baseline income is always positive, \
                 and project costs are paid upfront when a programme starts."
            }
            Chip::Mandate => {
                "Mandate: the CFA's treaty coordination authority - not local public \
                 consent. A global stock with a maximum; contentious projects spend it \
                 upfront. Baseline income is always positive."
            }
        }
    }
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

// ----------------------------------------------------------------- tooltip

/// Show the most specific hovered explanation beside the cursor. When
/// tooltip targets nest (a chip inside an explained row), the smallest
/// hovered node wins. The tooltip entity persists and is updated in place —
/// never despawned and respawned per frame — and a short grace period keeps
/// it steady across the dashboard's own rebuilds.
#[allow(clippy::type_complexity)]
pub fn tooltips(
    mut commands: Commands,
    mut state: ResMut<TooltipState>,
    mut existing: Query<(Entity, &mut Node), With<TooltipUi>>,
    mut texts: Query<&mut Text, With<TooltipText>>,
    hovered: Query<(&Interaction, &Tooltip, &ComputedNode)>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    let cursor = windows.single().ok().and_then(|w| w.cursor_position());
    let window_size = windows
        .single()
        .map(|w| Vec2::new(w.width(), w.height()))
        .unwrap_or(Vec2::new(1600., 900.));

    let mut best: Option<(&Tooltip, f32)> = None;
    for (interaction, tooltip, computed) in &hovered {
        if matches!(interaction, Interaction::Hovered | Interaction::Pressed) {
            let area = computed.size().x * computed.size().y;
            if best.map(|(_, smallest)| area < smallest).unwrap_or(true) {
                best = Some((tooltip, area));
            }
        }
    }

    match best {
        Some((tooltip, _)) => {
            state.text = tooltip.0.clone();
            state.grace_frames = 8;
        }
        None => state.grace_frames = state.grace_frames.saturating_sub(1),
    }

    let Some(cursor) = cursor.filter(|_| state.grace_frames > 0) else {
        for (entity, _) in &existing {
            commands.entity(entity).despawn();
        }
        return;
    };

    let x = if cursor.x > window_size.x - 400. {
        (cursor.x - 392.).max(0.)
    } else {
        cursor.x + 14.
    };
    let y = (cursor.y + 20.).min((window_size.y - 140.).max(0.));

    if let Some((_, mut node)) = existing.iter_mut().next() {
        node.left = Val::Px(x);
        node.top = Val::Px(y);
        if let Some(mut tip_text) = texts.iter_mut().next() {
            if tip_text.0 != state.text {
                tip_text.0 = state.text.clone();
            }
        }
        return;
    }

    commands
        .spawn((
            TooltipUi,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(x),
                top: Val::Px(y),
                max_width: Val::Px(380.),
                padding: UiRect::all(Val::Px(8.)),
                ..default()
            },
            BackgroundColor(TOOLTIP_BG),
            GlobalZIndex(10),
        ))
        .with_children(|tip| {
            tip.spawn((text(state.text.clone(), 12., FG), TooltipText));
        });
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
                Some(Chip::Emissions),
                "The nine sector rates summed across all three continents. \
                 Reaching zero (with non-negative Finance and Mandate income) wins the run.",
            );
            stat(
                bar,
                "REDUCED SO FAR",
                format!("{} Gt", gt(baseline - now)),
                GOOD,
                Some(Chip::Emissions),
                "How far gross emissions have fallen from this run's starting baselines.",
            );
            stat(
                bar,
                "FINANCE",
                format!(
                    "{}  ({:+}/t)",
                    fin(state.finance_milli),
                    fin_delta(state.finance_delta_milli)
                ),
                FINANCE,
                Some(Chip::Finance),
                "The global Finance stock and its per-tick income. Income is the sum of \
                 every continent's baseline contribution plus project income, minus upkeep. \
                 Project costs are paid upfront when the queue head starts.",
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
                Some(Chip::Mandate),
                "Current Mandate, its maximum, and per-tick income. Mandate is treaty \
                 coordination authority - contentious projects spend it upfront, and some \
                 projects raise the maximum.",
            );
            stat(
                bar,
                "PROGRAMME SLOTS",
                format!("{} of {}", state.active.len(), state.slots_total),
                FG,
                None,
                "Programmes running now against the global slots available. A second slot \
                 unlocks when any continent reaches Institutions 2; a third at 6. Slots \
                 never regress.",
            );
            stat(
                bar,
                "RANKED TIME",
                format!("{}:{:02}", seconds / 60, seconds % 60),
                FG,
                None,
                "Active, unpaused simulation time - the run's only score. Pausing stops it; \
                 planning while paused is free.",
            );

            if let Some(victory) = state.victory_tick {
                let at = victory / u64::from(speed);
                stat(
                    bar,
                    "VICTORY",
                    format!("zero at {}:{:02}", at / 60, at % 60),
                    GOOD,
                    None,
                    "Gross global emissions reached zero with non-negative Finance and \
                     Mandate income. Completion time is a replay metric, not a moral grade.",
                );
            }

            // Spacer, then pause — the only speed control that exists.
            bar.spawn(Node {
                flex_grow: 1.,
                ..default()
            });
            let label = if state.paused { "RESUME" } else { "PAUSE" };
            button(
                bar,
                label,
                Action::TogglePause,
                state.paused,
                "Pause or resume the simulation. There is no fast-forward: the game runs at \
                 one authored speed. While paused, ranked time stops and every planning \
                 action stays available.",
            );
            if state.paused {
                bar.spawn(text("planning while paused", 12., WARN));
            }
        });
}

fn stat(
    parent: &mut Spawner,
    title: &str,
    value: String,
    color: Color,
    chip_kind: Option<Chip>,
    tip: &str,
) {
    parent
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.),
                ..default()
            },
            Interaction::default(),
            Tooltip(tip.into()),
        ))
        .with_children(|column| {
            column.spawn(text(title, 10., DIM));
            column
                .spawn(Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(5.),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn(text(value, 16., color));
                    if let Some(kind) = chip_kind {
                        chip(row, kind);
                    }
                });
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
            card.spawn((
                Node {
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                },
                Interaction::default(),
                Tooltip(continent_tip(continent)),
            ))
            .with_children(|row| {
                row.spawn(text(continent.name(), 15., FG));
                row.spawn(Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(5.),
                    ..default()
                })
                .with_children(|value| {
                    value.spawn(text(format!("{} Gt", gt(total)), 15., EMISSIONS));
                    chip(value, Chip::Emissions);
                });
            });
            for sector in Sector::ALL {
                let rate = state.sector_emissions_milli[c][sector.index()];
                card.spawn((
                    Node {
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    Interaction::default(),
                    Tooltip(sector_tip(sector).into()),
                ))
                .with_children(|row| {
                    row.spawn(text(sector.name(), 12., DIM));
                    row.spawn(Node {
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(5.),
                        ..default()
                    })
                    .with_children(|value| {
                        value.spawn(text(gt(rate), 12., if rate == 0 { GOOD } else { FG }));
                        chip(value, Chip::Emissions);
                    });
                });
            }
            // Installed capacity icons: number then chip, per family.
            card.spawn((
                Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(10.),
                    ..default()
                },
                Interaction::default(),
                Tooltip(
                    "This continent's installed capacities. Icons are installed systems, \
                     never spendable resources, and are never pooled globally."
                        .into(),
                ),
            ))
            .with_children(|row| {
                for icon in Icon::ALL {
                    row.spawn(Node {
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(3.),
                        ..default()
                    })
                    .with_children(|pair| {
                        pair.spawn(text(
                            format!("{}", state.icons[c][icon.index()]),
                            12.,
                            Chip::for_icon(icon).color(),
                        ));
                        chip(pair, Chip::for_icon(icon));
                    });
                }
            });
            // Visible baseline contributions.
            card.spawn((
                Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(4.),
                    ..default()
                },
                Interaction::default(),
                Tooltip(
                    "This continent's visible contribution to global baseline income, per \
                     tick. Baselines stay positive, which is what guarantees every run can \
                     still be won."
                        .into(),
                ),
            ))
            .with_children(|row| {
                row.spawn(text(
                    format!("+{}", fin(state.baseline_finance_income_milli[c])),
                    11.,
                    DIM,
                ));
                chip(row, Chip::Finance);
                row.spawn(text(
                    format!("/t   +{}", money(state.baseline_mandate_income_milli[c])),
                    11.,
                    DIM,
                ));
                chip(row, Chip::Mandate);
                row.spawn(text("/t", 11., DIM));
                if leads_programme {
                    row.spawn(text("  * leading a programme", 11., DIM));
                }
            });
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
    panel(
        parent,
        "ACTIVE PROGRAMMES",
        "Programmes under construction. Full costs were paid when each started; progress \
         advances only while unpaused.",
        |body| {
            if state.active.is_empty() {
                body.spawn(text("none - the queue head starts when it can", 12., DIM));
            }
            for (index, build) in state.active.iter().enumerate() {
                let def = &sim.catalogue().projects[build.project as usize];
                let percent = build.progress * 100 / build.duration.max(1);
                body.spawn((
                    Node {
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(6.),
                        ..default()
                    },
                    Interaction::default(),
                    Tooltip(format!(
                        "{} - led by {}. {} of {} ticks built. {}",
                        def.title,
                        build.lead.name(),
                        build.progress,
                        build.duration,
                        def.summary
                    )),
                ))
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
                        button(
                            end,
                            "cancel",
                            Action::CancelActive(index),
                            false,
                            "Cancel this programme: progress and partial effects are \
                             destroyed, nothing is refunded, and the slot is freed.",
                        );
                    });
                });
            }
        },
    );
}

fn queue_panel(parent: &mut Spawner, sim: &Sim) {
    let state = &sim.state;
    panel(
        parent,
        "PLANNING QUEUE (strict FIFO)",
        "Projects waiting to start, in strict order. Nothing is reserved while queued; the \
         head starts when its prerequisites, resources, and a slot are all available - and \
         explains itself when it cannot.",
        |body| {
            if state.queue.is_empty() {
                body.spawn(text("empty - queue projects from the right", 12., DIM));
            }
            for (index, entry) in state.queue.iter().enumerate() {
                let def = &sim.catalogue().projects[entry.project as usize];
                body.spawn((
                    Node {
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    Interaction::default(),
                    Tooltip(format!(
                        "Queued: {} led by {}. Reorder or remove it freely - nothing is \
                         spent until it starts.",
                        def.title,
                        entry.lead.name()
                    )),
                ))
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
                            button(
                                end,
                                "up",
                                Action::MoveQueuedUp(index),
                                false,
                                "Move this entry one place earlier in the queue.",
                            );
                        }
                        button(
                            end,
                            "x",
                            Action::RemoveQueued(index),
                            false,
                            "Remove this entry from the queue. Nothing was reserved, so \
                             nothing is lost.",
                        );
                    });
                });
                if index == 0 {
                    if let Some(reason) = &state.last_block {
                        body.spawn((
                            Node::default(),
                            Interaction::default(),
                            Tooltip(block_tip(reason)),
                        ))
                        .with_children(|line| {
                            line.spawn(text(
                                format!("   waiting: {}", block_text(reason)),
                                12.,
                                WARN,
                            ));
                        });
                    }
                }
            }
        },
    );
}

fn notification_panel(parent: &mut Spawner, sim: &Sim) {
    let state = &sim.state;
    panel(
        parent,
        "OPPORTUNITIES & EVENTS",
        "Beneficial opportunities appear on their own seeded schedule and simply lapse if \
         ignored - never punitively. Below them, the most recent simulation events.",
        |body| {
            for (index, open) in state.opportunities.iter().enumerate() {
                let def = &sim.catalogue().opportunities[open.def as usize];
                body.spawn((
                    Node {
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    Interaction::default(),
                    Tooltip(
                        "A low-value beneficial opportunity: no trade-off, no programme \
                         slot, no penalty for ignoring it."
                            .into(),
                    ),
                ))
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
                    button(
                        row,
                        "claim",
                        Action::ClaimOpportunity(index),
                        false,
                        "Claim this opportunity's one-off benefit.",
                    );
                });
            }
            for event in state.trace.iter().rev().take(8) {
                body.spawn(text(
                    format!("t{}  {}", event.tick, trace_text(&event.kind)),
                    11.,
                    DIM,
                ));
            }
        },
    );
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
                panel(
                    column,
                    "PROJECT DETAIL",
                    "Select a project on the left of this panel to preview its outcomes.",
                    |body| {
                        body.spawn(text("select a project to preview its outcomes", 12., DIM));
                    },
                );
            }
        });
}

fn available_panel(parent: &mut Spawner, sim: &Sim, selected: Option<&str>) {
    panel(
        parent,
        "AVAILABLE PROJECTS",
        "Every project the CFA can currently start. Click one to preview its projected \
         outcomes before queueing it.",
        |body| {
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
                        align_items: AlignItems::Center,
                        padding: UiRect::axes(Val::Px(6.), Val::Px(2.)),
                        ..default()
                    },
                    BackgroundColor(if is_selected {
                        BUTTON_SELECTED
                    } else {
                        PANEL_SUNKEN
                    }),
                    Action::SelectProject(def.id.clone()),
                    Tooltip(format!("{} {}", def.summary, repeat_tip(def.repeat))),
                ))
                .with_children(|row| {
                    row.spawn(text(&def.title, 12., FG));
                    row.spawn(Node {
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(4.),
                        ..default()
                    })
                    .with_children(|end| {
                        end.spawn(text(fin(cost), 12., DIM));
                        chip(end, Chip::Finance);
                        let tag = repeat_tag(def.repeat);
                        if !tag.is_empty() {
                            end.spawn(text(tag, 12., DIM));
                        }
                    });
                });
            }
        },
    );
}

fn detail_panel(parent: &mut Spawner, sim: &Sim, id: &str, lead: Continent) {
    let Some(projected) = preview(sim, id, lead) else {
        return;
    };
    let Some(def) = sim.catalogue().projects.iter().find(|p| p.id == id) else {
        return;
    };

    panel(
        parent,
        "PROJECT DETAIL",
        "The projected outcome of queueing this project now. Direct effects are separated \
         from conditional consequences, and every figure comes from the same calculation \
         the simulation will run.",
        |body| {
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
                        "Choose the lead continent. The project's local effects, icon \
                         outputs, scaling, and prerequisites all anchor there.",
                    );
                }
                button(
                    row,
                    "QUEUE",
                    Action::QueueSelected,
                    false,
                    "Add this project to the back of the planning queue with the chosen \
                     lead continent. Nothing is spent until it starts.",
                );
            });

            // Guaranteed direct effects, from the same calc the sim will run.
            let wait = match projected.ticks_until_affordable {
                Some(0) => "affordable now".to_string(),
                Some(ticks) => format!("affordable in ~{ticks}t"),
                None => "income cannot currently cover this".to_string(),
            };
            body.spawn((
                Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(4.),
                    ..default()
                },
                Interaction::default(),
                Tooltip(
                    "Upfront costs, paid in full when the queue head starts, and the \
                     construction duration. The affordability estimate assumes current \
                     income."
                        .into(),
                ),
            ))
            .with_children(|row| {
                row.spawn(text(
                    format!("cost {}", fin(projected.finance_cost_milli)),
                    12.,
                    FG,
                ));
                chip(row, Chip::Finance);
                row.spawn(text(
                    format!("+ {}", money(projected.mandate_cost_milli)),
                    12.,
                    FG,
                ));
                chip(row, Chip::Mandate);
                row.spawn(text(
                    format!("   duration {}t   {}", projected.duration_ticks, wait),
                    12.,
                    FG,
                ));
            });
            for modifier in &projected.cost_trace.modifiers {
                if modifier.permille != 0 {
                    body.spawn((
                        Node::default(),
                        Interaction::default(),
                        Tooltip(
                            "A named cost modifier, in the order applied: the repeat curve \
                             prices how often this has been built; other projects can \
                             discount it."
                                .into(),
                        ),
                    ))
                    .with_children(|line| {
                        line.spawn(text(
                            format!("   cost {} {:+} per 1000", modifier.name, modifier.permille),
                            11.,
                            DIM,
                        ));
                    });
                }
            }
            if projected.global_emissions_change_milli != 0 {
                body.spawn((
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(5.),
                        ..default()
                    },
                    Interaction::default(),
                    Tooltip(
                        "The guaranteed direct reduction at completion, at today's scaling. \
                         Completion locks in the magnitude - icons gained later never \
                         retroactively change a finished project."
                            .into(),
                    ),
                ))
                .with_children(|row| {
                    row.spawn(text(
                        format!(
                            "direct: {} Gt global on completion",
                            gt(projected.global_emissions_change_milli)
                        ),
                        12.,
                        EMISSIONS,
                    ));
                    chip(row, Chip::Emissions);
                });
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
                body.spawn((
                    Node::default(),
                    Interaction::default(),
                    Tooltip(
                        "Part of the active scaling calculation: continuous icon scaling \
                         applies first (capped), then discrete authored breakpoints, in \
                         authored order."
                            .into(),
                    ),
                ))
                .with_children(|line| {
                    line.spawn(text(
                        format!("   {} {:+} per 1000", modifier.name, modifier.permille),
                        11.,
                        MANDATE,
                    ));
                });
            }
            if projected.finance_delta_change_milli != 0
                || projected.mandate_delta_change_milli != 0
            {
                body.spawn((
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(4.),
                        ..default()
                    },
                    Interaction::default(),
                    Tooltip(
                        "Ongoing income or upkeep this project adds while it stands. \
                         Decommissioning removes it along with every other benefit."
                            .into(),
                    ),
                ))
                .with_children(|row| {
                    row.spawn(text(
                        format!(
                            "ongoing: {:+}",
                            fin_delta(projected.finance_delta_change_milli)
                        ),
                        12.,
                        FINANCE,
                    ));
                    chip(row, Chip::Finance);
                    row.spawn(text(
                        format!("/t   {:+}", money(projected.mandate_delta_change_milli)),
                        12.,
                        FINANCE,
                    ));
                    chip(row, Chip::Mandate);
                    row.spawn(text("/t", 12., FINANCE));
                });
            }

            // Conditional consequences, separated per the preview contract.
            if let Some(next) = &projected.next_breakpoint {
                body.spawn((
                    Node::default(),
                    Interaction::default(),
                    Tooltip(
                        "A conditional consequence: this bonus applies only once the lead \
                         continent holds enough of the named icon when the project \
                         completes."
                            .into(),
                    ),
                ))
                .with_children(|line| {
                    line.spawn(text(
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
                });
            }
            for unlock in &projected.unlocks {
                body.spawn((
                    Node::default(),
                    Interaction::default(),
                    Tooltip(
                        "Completing this project unlocks another project for everyone. \
                         Unlocks persist even if the unlocking project is later \
                         decommissioned."
                            .into(),
                    ),
                ))
                .with_children(|line| {
                    line.spawn(text(format!("unlocks: {unlock}"), 11., WARN));
                });
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
        },
    );
}

// ------------------------------------------------------------- primitives

fn panel(parent: &mut Spawner, title: &str, tip: &str, body: impl FnOnce(&mut Spawner)) {
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
            content
                .spawn((Node::default(), Interaction::default(), Tooltip(tip.into())))
                .with_children(|header| {
                    header.spawn(text(title, 11., DIM));
                });
            body(content);
        });
}

fn button(parent: &mut Spawner, label: &str, action: Action, highlighted: bool, tip: &str) {
    parent
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(8.), Val::Px(3.)),
                ..default()
            },
            BackgroundColor(if highlighted { BUTTON_SELECTED } else { BUTTON }),
            action,
            Tooltip(tip.into()),
        ))
        .with_children(|inner| {
            inner.spawn(text(label, 12., FG));
        });
}

/// A placeholder icon chip: a small coloured badge with a letter, standing in
/// for final icon art. Hovering it explains the concept it marks.
fn chip(parent: &mut Spawner, kind: Chip) {
    let radius = if kind.round() {
        BorderRadius::MAX
    } else {
        BorderRadius::all(Val::Px(3.))
    };
    parent
        .spawn((
            Node {
                width: Val::Px(14.),
                height: Val::Px(14.),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_shrink: 0.,
                ..default()
            },
            BackgroundColor(kind.color()),
            radius,
            Interaction::default(),
            Tooltip(kind.explain().into()),
        ))
        .with_children(|inner| {
            inner.spawn(text(kind.letter(), 9., CHIP_TEXT));
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

/// Finance is always shown to two decimal places.
fn fin(milli: i64) -> String {
    format!("{:.2}", milli as f64 / 1000.0)
}

/// Finance deltas: signed, two decimal places.
fn fin_delta(milli: i64) -> String {
    format!("{:+.2}", milli as f64 / 1000.0)
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
        Repeat::Unique => "(unique)",
        Repeat::Repeatable => "",
        Repeat::Tiered { .. } => "(tiered)",
        Repeat::CappedRollout { .. } => "(capped)",
    }
}

fn repeat_tip(repeat: Repeat) -> &'static str {
    match repeat {
        Repeat::Unique => "Can be built once, ever.",
        Repeat::Repeatable => "Repeatable: the cost curve prices each further build.",
        Repeat::Tiered { .. } => "Tiered: a fixed number of escalating completions.",
        Repeat::CappedRollout { .. } => {
            "Capped rollout: a limited number of standing completions per continent."
        }
    }
}

fn continent_tip(continent: Continent) -> String {
    let base = match continent {
        Continent::Europe => {
            "Europe hosts the CFA's administrative headquarters for contingent treaty \
             reasons only - it receives no technical, economic, or efficiency bonus."
        }
        Continent::NorthAmerica => {
            "North America. Continental differences are authored starting systems and \
             bottlenecks, never innate traits."
        }
        Continent::MajorityWorld => {
            "Majority World - a provisional prototype label, under review before any \
             public release. Differences are authored starting systems and bottlenecks, \
             never innate traits or development rankings."
        }
    };
    format!(
        "{base} The figure is this continent's summed gross emissions across its three \
         sectors."
    )
}

fn sector_tip(sector: Sector) -> &'static str {
    match sector {
        Sector::Power => {
            "Power: electricity generation. Decarbonising it enables - but does not \
             complete - every other transition. The rate only changes through projects."
        }
        Sector::TransportAndBuildings => {
            "Transport & Buildings: travel demand, freight, heating, and the building \
             stock, grouped for prototype simplicity. The rate only changes through \
             projects."
        }
        Sector::IndustryAndLand => {
            "Industry & Land: heavy industry, methane, and land use, grouped for \
             prototype simplicity. The rate only changes through projects."
        }
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
                fin(*needed_milli),
                fin(*have_milli)
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

fn block_tip(reason: &BlockReason) -> String {
    let specific = match reason {
        BlockReason::NoFreeSlot => {
            "Every global programme slot is in use. A slot frees when a programme \
             completes or is cancelled; Institutions milestones add more."
        }
        BlockReason::NotUnlocked => "Another project must unlock this one before it can start.",
        BlockReason::MissingIcon { .. } => {
            "An icon prerequisite: the lead continent must hold enough of this installed \
             capacity before construction can start."
        }
        BlockReason::MissingProject { .. } => "A project prerequisite has not been completed yet.",
        BlockReason::RepeatLimitReached => {
            "This project's repeat model allows no further builds here."
        }
        BlockReason::InsufficientFinance { .. } => {
            "Costs are paid upfront in full. Income is always positive, so the head will \
             start once enough Finance accrues."
        }
        BlockReason::InsufficientMandate { .. } => {
            "Contentious projects spend Mandate upfront. Mandate income is always \
             positive, so the head will start once enough accrues."
        }
    };
    format!(
        "{specific} The queue is strict FIFO: later entries wait rather than jumping \
         ahead."
    )
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
