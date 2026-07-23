//! `nw-client` — the presentation client. It will be a Bevy application (native
//! desktop plus a narrower WASM layout) that renders simulation state and maps
//! input to semantic commands. It is a non-authoritative view over the
//! command/state interface: it never owns authoritative state.
//!
//! Scaffold status: a placeholder entry point until the Bevy UI spike. See
//! `pasm/spec/core/production.yaml`.

fn main() -> anyhow::Result<()> {
    println!("The Necessary Work — client scaffold.");
    println!("The Bevy dashboard is not built yet; see docs/design and pasm/spec.");
    Ok(())
}
