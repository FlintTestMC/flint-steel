# Contributing

Thanks for helping improve `flint-steel`. This crate is the SteelMC adapter for Flint, so changes should preserve the goal of testing real SteelMC behavior instead of adding mock-only behavior.

## Development Setup

Follow [GETTING_STARTED.md](GETTING_STARTED.md) to clone the repo, build the harness, and add the SteelBenchmark specs.

Before making changes, check the current state:

```bash
git status --short
cargo test --lib test_world_creation
```

## Project Layout

- `src/lib.rs` initializes SteelMC registries, behaviors, and the shared Tokio runtime
- `src/adapter.rs` implements `SteelAdapter` and the Flint benchmark test entry points
- `src/world.rs` wraps a real RAM-only `steel-core` world
- `src/player.rs` wraps a real SteelMC player for inventory and interaction tests
- `src/convert.rs` maps Flint data types to SteelMC data types
- `src/test_connection.rs` records test player connection events
- `flint.toml.example` shows persistent Flint filter configuration

## Expectations

- Prefer real SteelMC code paths over test-only shortcuts.
- Keep test worlds RAM-only and deterministic.
- Add comments only when they explain non-obvious integration behavior.
- Keep public documentation in sync with supported environment variables and config fields.
- Avoid unrelated formatting or dependency churn.

## Verification

Run the narrowest relevant check first:

```bash
cargo test --lib test_world_creation
```

For adapter or Flint filtering changes, run:

```bash
cargo test --lib test_run_flint_selected -- --nocapture
```

For broader behavior changes, run:

```bash
cargo test --lib test_run_all_flint_benchmarks -- --nocapture
```

Full benchmark tests require the SteelBenchmark specs in `./test` or `TEST_PATH`.

## Pull Requests

Please include:

- What changed
- Why it changed
- Which tests or commands were run
- Any benchmark specs, tags, or filters used during verification

If a change intentionally diverges from production SteelMC behavior for test stability, call that out clearly in the PR description.
