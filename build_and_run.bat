@echo off
rem Build and run The Necessary Work prototype (native client, release build).
rem Set NW_SEED=n first to fix the scenario seed.
cargo run --release -p nw-client
