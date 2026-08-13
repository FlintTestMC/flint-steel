# Getting started

## Prerequisites

- Rust nightly from `rust-toolchain.toml`
- Git

The exact coordinated Steel and `flint-core` commits are pinned in `Cargo.toml` and `Cargo.lock`. Cargo can build them directly. To work on all three repositories together, keep local checkouts beside `flint-steel`, then enable the optional path patches:

```bash
cp .cargo/config.toml.example .cargo/config.toml
```

Adjust the paths in `.cargo/config.toml` if your checkouts use a different layout. The patch selects source checkouts only; Steel's `test-harness` feature remains declared in `Cargo.toml`.

## Verify the adapter

```bash
cargo test --locked --all-targets
```

This includes paired JSON controls. Four supported scenarios must turn green, while three deliberately broken scenarios must turn red for their expected reasons.

## Run a test file

```bash
cargo run --locked --bin flint-steel -- tests/fixtures/mvp/inventory_positive.json
```

The JSON summary is printed to standard output. A gameplay failure is still printed as JSON, followed by a nonzero process exit.
Running a file or directory does not create Flint's tag-index cache.

## Run a test repository

```bash
git clone https://github.com/FlintTestMC/FlintBenchmark.git FlintBenchmark
cargo run --locked --bin flint-steel -- FlintBenchmark/tests
```

Directories are recursive. An empty directory, incompatible or skipped test, adapter error, or failed assertion makes the command fail.

## Current limits

- Entity summon, movement, and assertions return an unsupported error.
- Only clear weather is supported.
- Item data must name registered persistent Steel components and contain valid SNBT values.
- Block-entity assertions read only the requested NBT paths.

These limits are explicit so unsupported behavior cannot produce a false green result.
