# Explicit component startup experiment

This experiment tests a replacement for the per-export constructor workaround:
provide one `_initialize` entry point, then let component initialization run
constructors before application exports. It builds on the branch that removes
`run_ctors_once` and its generated calls.

This is a prototype, not a production linker change. The caller explicitly
chooses whether to supply startup or use the toolchain's existing startup.

## What changes

[`startup.c`](startup.c) exports `_initialize`, calls `__wasm_call_ctors`, and
traps if initialization is attempted twice. The script compiles it to a Wasm
object and links it into each fixture module when `--startup explicit` is used.

The explicit constructor call tells the linker that startup is handled, so it
does not insert constructor calls around ordinary exports. Component packaging
then recognizes `_initialize` and schedules it after imported functions are
connected. Generated exports no longer check a separate constructor guard.

The fixtures still register counter constructors through `.init_array`. Those
entries are test instruments, not the marker from the original fix.

## Run it

Requirements: Rust with the requested Wasm target installed, Python 3, Clang
with WebAssembly support, Node.js, and Wasmtime. Build the CLI from this branch:

```sh
cargo build --bin wit-bindgen --no-default-features --features rust
python3 experiments/component-startup/run.py \
  --toolchain beta --target wasm32-unknown-unknown
```

`--toolchain` and `--target` are required. `--startup explicit` is the default;
use `--startup native` for a toolchain that already supplies `_initialize`:

```sh
python3 experiments/component-startup/run.py \
  --toolchain beta --target wasm32-wasip2 --startup native
```

Optional arguments are `--cli PATH`, `--output PATH`, `--rustup-home PATH`,
`--clang PATH`, and `--node PATH`. The CLI defaults to `target/debug/wit-bindgen`;
output goes under `target/component-startup-experiment/<toolchain>/<target>/<mode>`.
Use `--rustup-home` for an isolated Rust installation. Target installation must
use the same toolchain and Rustup home as the experiment.

The results below used Rust 1.99.0 and Rust 1.100.0-beta.4. The `beta` alias
changes over time; select the corresponding installed toolchain to reproduce
those versions exactly.

## What the checks demonstrate

Every normal run executes both component suites:

- `count/`: a constructor increments a counter. Two calls to its exported
  getter must both return `1`.
- `imports/`: a constructor calls an imported provider component. The runner
  checks the provider's count before calling an ordinary export of the tested
  component, then checks that repeated exports do not increment it again.

The component suites do not call `_initialize` themselves. Their success checks
that component packaging schedules startup and that imports are usable then.

For explicit startup on `wasm32-unknown-unknown`, [`probe-core.js`](probe-core.js)
also instantiates the same core module directly. It checks this sequence:

| Action | Constructor count |
| --- | --- |
| Instantiate | 0 |
| Allocate and reallocate through `cabi_realloc` | 0 |
| Call `_initialize` | 1 |
| Call two ordinary exports | 1 |
| Reallocate again | 1 |
| Call `_initialize` a second time | Traps; count remains 1 |

This direct-core check calls startup manually, as a core-module embedder would.
It complements the automatic startup exercised by the component suites.

## Tested configurations

| Rust | Target | Startup | Observed result |
| --- | --- | --- | --- |
| 1.99.0 | wasm32-unknown-unknown | Explicit object | Pass |
| 1.100.0-beta.4 | wasm32-unknown-unknown | Explicit object | Pass |
| 1.99.0 | wasm32-wasip2 | Explicit object | Pass |
| 1.100.0-beta.4 | wasm32-wasip2 | Native CRT | Pass |
| 1.100.0-beta.4 | wasm32-wasip3 | Native CRT | Pass |

A negative control deliberately adds our startup object to a WASIp2 build
that already provides `_initialize`:

```sh
python3 experiments/component-startup/run.py \
  --toolchain beta --target wasm32-wasip2 \
  --expect-duplicate-initialize
```

That check succeeds only when linking reports the expected duplicate
`_initialize` definition. It demonstrates why production startup ownership
must be resolved before automatically adding this object.

## Limits

The experiment manually selects startup. It does not implement automatic CRT
selection in Rust or a component linker, establish a command-module startup
contract, or validate threaded and dynamic-linking configurations. It shows
that explicit component startup can replace the observed per-export behavior
in the configurations above. Compiled objects and generated logs remain in
the output directory and are not part of this source change.
