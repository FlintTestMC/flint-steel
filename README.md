# flint-steel

`flint-steel` runs Flint JSON tests against Steel's production world, player, inventory, block behavior, and tick code without starting a network server.

## MVP scope

The current runner supports:

- block placement, removal, fill, state properties, and assertions
- exact world ticks and Overworld daytime assertions
- player inventory, hotbar selection, movement, game mode, and item interaction
- requested persistent item components and block-entity NBT paths where Steel provides codecs
- named or numeric time, clear weather, and registered game rules

It fails closed for unknown blocks, items, states, properties, components, invalid stack counts, and rejected world mutations. Entity actions and rain or thunder are not implemented yet and return explicit errors.

## Run JSON tests

Pass one JSON file or a directory. Directories are searched recursively.

```bash
cargo run --locked --bin flint-steel -- tests/fixtures/mvp/block_fill_positive.json
cargo run --locked --bin flint-steel -- /path/to/FlintBenchmark/tests
```

The command writes a full `TestSummary` as JSON. It exits successfully only when at least one test executes, no tests are skipped, and every test passes.
This unfiltered runner does not create or update Flint's tag-index cache.

## Prove red and green behavior

```bash
# Must exit 0
cargo run --locked --bin flint-steel -- tests/fixtures/mvp/place_fence_positive.json

# Must exit nonzero and report expected dirt, actual stone
cargo run --locked --bin flint-steel -- tests/fixtures/mvp/wrong_block_negative.json

# Runs all paired contract controls
cargo test --locked --test mvp_positive_controls --test mvp_negative_controls
```

The checked-in controls cover fill, inventory, exact time advancement, real player fence placement, a deliberately wrong assertion, an invalid block ID, and the distinction between cave air and regular air.

See [GETTING_STARTED.md](GETTING_STARTED.md) for setup details and [CONTRIBUTING.md](CONTRIBUTING.md) for the development workflow.

## Related projects

- [SteelMC](https://github.com/Steel-Foundation/SteelMC)
- [flint-core](https://github.com/FlintTestMC/flint-core)
- [FlintBenchmark](https://github.com/FlintTestMC/FlintBenchmark)
- [FlintCLI](https://github.com/FlintTestMC/FlintCLI)

## License

[MIT](LICENSE)
