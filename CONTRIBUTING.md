# Contributing

`flint-steel` is a thin Flint adapter over Steel's in-memory test harness. Infrastructure such as runtime ownership, chunks, player attachment, and shutdown belongs in Steel. Flint type conversion and trait implementation belong here.

## Project layout

- `src/adapter.rs` creates disposable Steel-backed Flint worlds
- `src/world.rs` implements world operations and configuration
- `src/player.rs` implements inventory, movement, and interaction
- `src/convert.rs` performs fail-closed Flint and Steel conversions
- `src/bin/flint-steel.rs` runs a JSON file or directory
- `tests/fixtures/mvp` contains paired green and red controls

## Correctness rules

- Unknown or invalid input must return an error. Never substitute air, an empty item, or a no-op.
- Exercise production Steel gameplay behavior through `steel_core::test_harness`.
- A green control is not sufficient by itself. Add a red control that proves the assertion can detect the regression.
- Keep dependency revisions and `Cargo.lock` reproducible.
- Document unsupported behavior explicitly.

## Verification

```bash
cargo fmt --all --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets -- --test-threads=1
```

Also run the relevant fixture through the executable and verify its exit status. Positive controls must exit zero, while intentional negative controls must produce a useful JSON failure and exit nonzero.

## Pull requests

Include the exact commands run, the dependency revisions used, and any unsupported behavior introduced or removed.
