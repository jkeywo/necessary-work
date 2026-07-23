//! `nw-client` — the Bevy presentation client (UI spike).
//!
//! A non-authoritative view over the simulation's command/state interface: it
//! renders [`nw_simulation::RunState`] and maps clicks to semantic commands.
//! The simulation advances on a fixed timestep at the scenario's authored
//! speed — there is no player-facing fast-forward, and pause (with full
//! planning while paused) goes through the same `Pause`/`Resume` commands as
//! any other client.
//!
//! Deliberately rough per the Stage 1 roadmap: flat panels, default font,
//! placeholder colours. The layered-explanation *data* it shows comes from the
//! simulation's calc traces and previews, so nothing here re-derives a number.
//!
//! Dev hooks: `NW_SEED` fixes the scenario seed; `NW_SMOKE_SECS` exits after
//! that many seconds; `NW_SHOT` writes a screenshot to the given path.

mod ui;

use bevy::prelude::*;
use nw_content::Catalogue;
use nw_simulation::{Continent, Sim};

/// The client's handle on the authoritative simulation.
#[derive(Resource)]
pub struct Game {
    pub sim: Sim,
}

/// Client-side session state: what is selected, nothing more. It never
/// reaches the simulation, the log, or a digest.
#[derive(Resource)]
pub struct UiState {
    pub selected: Option<String>,
    pub lead: Continent,
    pub dirty: bool,
}

/// A fresh seed from the platform clock. `SystemTime` is unavailable on
/// wasm32-unknown-unknown, so the browser asks `Date.now()` instead.
#[cfg(not(target_arch = "wasm32"))]
fn seed_from_clock() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or(1)
}

#[cfg(target_arch = "wasm32")]
fn seed_from_clock() -> u64 {
    js_sys::Date::now() as u64 / 1000
}

fn main() {
    let seed = std::env::var("NW_SEED")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or_else(seed_from_clock);
    let catalogue = Catalogue::embedded();
    let ticks_per_second = f64::from(catalogue.scenario.authored_speed_ticks_per_second.max(1));
    println!("The Necessary Work — seed {seed}");

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: format!("The Necessary Work — prototype (seed {seed})"),
                resolution: (1600., 900.).into(),
                // In the browser the canvas tracks its parent element; the
                // field is ignored on native.
                fit_canvas_to_parent: true,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(ui::BACKGROUND))
        .insert_resource(Game {
            sim: Sim::new(catalogue, seed),
        })
        .insert_resource(UiState {
            selected: None,
            lead: Continent::Europe,
            dirty: true,
        })
        .insert_resource(Time::<Fixed>::from_hz(ticks_per_second))
        .init_resource::<ui::TooltipState>()
        .add_systems(Startup, setup)
        .add_systems(FixedUpdate, advance_simulation)
        .add_systems(
            Update,
            (
                ui::handle_buttons,
                ui::rebuild.after(ui::handle_buttons),
                ui::tooltips.after(ui::rebuild),
                dev_hooks,
            ),
        )
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
}

/// One authored-speed simulation tick. While paused the tick is a no-op in
/// the simulation itself; the UI still refreshes so planning stays live.
fn advance_simulation(mut game: ResMut<Game>, mut ui_state: ResMut<UiState>) {
    game.sim.tick();
    ui_state.dirty = true;
}

/// Development-only smoke and screenshot hooks, driven by environment
/// variables so CI and agents can boot the app without a human. `NW_DEMO`
/// queues a few projects and selects one, so a screenshot shows every panel
/// populated.
fn dev_hooks(
    time: Res<Time>,
    mut commands: Commands,
    mut exit: EventWriter<AppExit>,
    mut shot_taken: Local<bool>,
    mut demo_done: Local<bool>,
    mut game: ResMut<Game>,
    mut ui_state: ResMut<UiState>,
) {
    if !*demo_done {
        *demo_done = true;
        if std::env::var("NW_DEMO").is_ok() {
            for (project, lead) in [
                ("wind-solar-deployment", Continent::MajorityWorld),
                ("grid-modernisation", Continent::Europe),
                ("methane-and-land-programme", Continent::NorthAmerica),
            ] {
                let _ = game.sim.execute(nw_simulation::Command::QueueProject {
                    project: project.into(),
                    lead,
                });
            }
            ui_state.selected = Some("wind-solar-deployment".into());
            ui_state.lead = Continent::MajorityWorld;
            ui_state.dirty = true;
        }
    }
    if let Some(path) = std::env::var("NW_SHOT").ok().filter(|_| !*shot_taken) {
        let shot_at = std::env::var("NW_SHOT_SECS")
            .ok()
            .and_then(|value| value.parse::<f32>().ok())
            .unwrap_or(1.5);
        if time.elapsed_secs() > shot_at {
            *shot_taken = true;
            use bevy::render::view::screenshot::{save_to_disk, Screenshot};
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path));
        }
    }
    if let Some(secs) = std::env::var("NW_SMOKE_SECS")
        .ok()
        .and_then(|value| value.parse::<f32>().ok())
    {
        if time.elapsed_secs() > secs {
            exit.write(AppExit::Success);
        }
    }
}
