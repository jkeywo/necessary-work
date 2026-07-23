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
//!
//! Scaffold status: the units and world structure below are real (early
//! implementation order, step 1). The tick loop, command handling, effect
//! vocabulary, and validation are grown against `pasm/spec/core`.

pub mod units;
pub mod world;

pub use units::Tick;
pub use world::{Continent, Icon, Sector};
