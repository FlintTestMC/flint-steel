# Steel-Test-Suite

The goal of this project is to build testing suite for [SteelMC](https://github.com/4lve/SteelMC).
Which should test automatic blocks, items and more.

## Current Plan
 - [ ] Write a json parser
 - [ ] Write a chunk generator
 - [ ] Write a runner which handels the communication with SteelMC

## Usage

Currently, all tests will be stored in `./test` directory.

The basic configuration will run all tests which are available
```bash
cargo run --package steel_block_simulation --bin steel_block_simulation 
```

### Indexing
would be recommended with a lot of tests, to have it available for a quicker start or if you want to only run some tests

```bash
cargo run --package steel_block_simulation --bin steel_block_simulation -- index 
```

### Scoped tests
You have also the option to run tests in a specific scope a scope is the directory.
A scope ends with `:` and will run all tests which are below that. You can nest as much scopes you want.

```bash
cargo run --package steel_block_simulation --bin steel_block_simulation -- new: 
```

To run a single test you add the full scope to it, this will a single test new:test

```bash
cargo run --package steel_block_simulation --bin steel_block_simulation -- new:test 
```


## Test files
Currently the test files will be in json written.
This describes the full test cases if what needs to be tested and done.

## Documentation
To have a starting point [here](./docs/Get_Started.md) will come the documentation of everything.

## Future Ideas
 - Mock Client to run tests on vanilla server and on every minecraft server
 - implement a powerful testing api which can be implemented by other minecraft server to use also this suite
 - For more repetitive test cases there will be [jsonnet](https://jsonnet.org/) available