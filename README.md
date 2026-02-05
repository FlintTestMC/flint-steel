# flint-steel

Flint testing framework integration for [SteelMC](https://github.com/FlintTestMC/steel-core). This crate implements the Flint traits (`FlintAdapter`, `FlintWorld`, `FlintPlayer`) so that automated Minecraft tests run against the real SteelMC server logic — no mocks, 100% code reuse with production behavior.

## How It Works

flint-steel wraps real `steel-core` internals in lightweight test harnesses:

- **SteelTestWorld** — wraps an `Arc<World>` with RAM-only storage and on-demand empty chunks (no disk I/O)
- **SteelTestPlayer** — uses real inventory, item, and block behavior with a mock network connection that records packets
- **SteelAdapter** — creates test worlds and drives the Flint test runner

Tests are written as JSON specs (see [SteelBenchmark](https://github.com/FlintTestMC/SteelBenchmark)) and executed against the real game engine, including block neighbors, shapes, callbacks, and tick processing.

## Prerequisites

- Rust (edition 2024)
- [steel-core](https://github.com/FlintTestMC/steel-core) — the SteelMC server crate (workspace dependency)
- [flint-core](https://github.com/JunkyDeveloper/flint-core) — the Flint testing framework

## Installation

> Coming soon — setup instructions are being finalized.

## Getting Tests

Clone [SteelBenchmark](https://github.com/FlintTestMC/SteelBenchmark) to provide the test files:

```bash
git clone https://github.com/FlintTestMC/SteelBenchmark.git test
```

By default, tests are loaded from `./test`. You can change this with the `TEST_PATH` environment variable.

## Usage

```bash
# Run all Flint tests
cargo test --lib

# Run a single test by name
FLINT_TEST=place_fence cargo test --lib

# Run tests matching a glob pattern
FLINT_PATTERN="*fence" cargo test --lib

# Run tests filtered by tags
FLINT_TAGS=redstone,walls cargo test --lib
```

### Environment Variables

| Variable | Default | Description |
|---|---|---|
| `FLINT_TEST` | — | Run a single test by exact name |
| `FLINT_PATTERN` | — | Run tests matching a glob pattern |
| `FLINT_TAGS` | — | Filter tests by comma-separated tags |
| `TEST_PATH` | `./test` | Path to test files directory |
| `INDEX_NAME` | `.cache/index_new.json` | Cache index location |
| `DEFAULT_TAG` | `default` | Default tag for untagged tests |

You can set these in a `.env` file (see `.env.example`).

## Related Projects

| Project | Description |
|---|---|
| [steel-core](https://github.com/FlintTestMC/steel-core) | SteelMC server implementation |
| [flint-core](https://github.com/JunkyDeveloper/flint-core) | Flint testing framework |
| [SteelBenchmark](https://github.com/FlintTestMC/SteelBenchmark) | Test suite and benchmark specs |
| [FlintCLI](https://github.com/JunkyDeveloper/FlintCLI) | Command-line interface for Flint |
| [FlintDocs](https://github.com/JunkyDeveloper/FlintDocs) | Documentation |

## License

[MIT](LICENSE)
