//! The deterministic, pure-Rust simulation core of *The Necessary Work*.
//!
//! This crate is independent of Bevy, rendering, the OS, save UI, and network
//! code. Clients drive it through a narrow command/state interface. Given a
//! ruleset version, content version, scenario identifier, scenario seed, and an
//! ordered log of timestamped commands, it is fully deterministic: wall-clock
//! time, frame rate, locale, and platform APIs may not affect its results.
//!
//! Authoritative state uses integers / fixed-point, never floating-point
//! outcomes, so native and WASM builds cannot diverge.

pub mod calc;
pub mod command;
pub mod preview;
pub mod rng;
pub mod sim;
pub mod state;
pub mod trace;
pub mod units;
pub mod world;

pub use command::{Command, LoggedCommand, Rejection};
pub use preview::{preview, Preview};
pub use sim::{derive_state, Sim, RULESET_VERSION};
pub use state::RunState;
pub use trace::{BlockReason, TraceEvent, TraceKind};
pub use units::Tick;
pub use world::{Continent, Icon, Sector};
