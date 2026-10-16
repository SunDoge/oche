# Building and tooling

## Toolchain

[mise](https://mise.jdx.dev/) provisions the required CLI tools. The Dart SDK is
pinned in [`../mise.toml`](../mise.toml) to the release the VM binding was
validated against; `sdk-info` discovers it through mise, an explicit SDK
directory, or `OCHE_DART_SDK`.

```sh
mise install        # provisions dart and rust as pinned in mise.toml
mise run check      # fmt --check + clippy -D warnings + cargo test --workspace
```

CI (`.github/workflows/ci.yml`) runs the same checks through mise on Linux and
macOS. Releases are cut by release-plz from conventional commits on `main`:
the workspace shares one version, so every crate and the changelog bump
together in one release PR.

## SDK headers

The installed SDK ships these under `include/`:

| Header | Role |
| --- | --- |
| `dart_api.h` | Full embedding API: isolate lifecycle, handles, invocation, Kernel loading |
| `dart_api_dl.h` / `dart_api_dl.c` | Dynamic-link API subset for native libraries |
| `dart_native_api.h` | Native ports and messages |
| `dart_tools_api.h` | Tooling APIs |
| `dart_version.h` | Dynamic-link API version constants |

`sdk-info` discovers the installed SDK via mise, an explicit SDK directory, or
`OCHE_DART_SDK`. Do not vendor headers copied from a different VM release.
`dart_api_dl.h` is a subset, not a replacement for the full library loader API.
Headers alone do not supply a VM shared library: the official SDK's `dartvm`
is an executable. The current integration probe runs an unmodified `dartvm`
which loads the Rust example library; a standalone Rust-first host is future
work requiring a suitable VM embedding build.

## Binding constraints

The process-symbol resolver currently targets Linux; the generic resolver can
be adapted to another platform's embedding library. Calls require an entered
isolate on the same OS thread with callbacks allowed. Use non-leaf Dart FFI.
Native-facing Dart entry points need `@pragma('vm:entry-point')`.
Keep the Rust library loaded through isolate-group shutdown so external-buffer
finalizers remain callable. Individual Kernel library unloading is not promised.

## Commands

```sh
cargo run -p oche -- sdk-info
cargo run -p oche -- bindgen-dart tools.dart -o tools.d.oche
cargo run -p oche -- fmt examples/arithmetic.oche
cargo run -p oche -- fmt --check examples/*.oche
cargo run -p oche -- check examples/arithmetic.oche
cargo run -p oche -- ast examples/arithmetic.oche
cargo run -p oche -- emit-dart examples/arithmetic.oche
cargo run -p oche -- build examples/arithmetic.oche
cargo run -p oche -- build-module examples/arithmetic.oche -o tool.dill
cargo run -p oche -- serve agent/pi_core.oche
cargo run -p oche -- build-exe examples/arithmetic.oche
cargo run -p oche -- build-js examples/js_module.oche -o core.mjs
cargo run -p oche -- run examples/arithmetic.oche add 20 22
cargo run -p oche -- test examples/testing.oche
cargo run -p oche -- lsp
cargo run -p xtask -- dart-bindings --sdk "$(mise where dart)"
cargo run -p xtask -- dart-bindings --sdk "$(mise where dart)" --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## Acceptance probes

```sh
python3 tests/run_vm_probe.py
python3 tests/run_oche_module_probe.py
python3 tests/run_comptime_acceptance.py
python3 tests/run_comptime_values_acceptance.py
python3 tests/run_tagged_union_acceptance.py
python3 tests/run_match_guard_acceptance.py
python3 tests/run_async_acceptance.py
python3 tests/run_isolate_acceptance.py
python3 tests/run_list_acceptance.py
python3 tests/run_js_module_acceptance.py
python3 tests/run_agent_machine_acceptance.py
python3 tests/run_jit_agent_machine_acceptance.py
python3 tests/run_pi_mono_acceptance.py
python3 tests/run_dart_interop_acceptance.py
python3 tests/run_system_ffi_acceptance.py
python3 tests/run_generated_expression_acceptance.py
python3 tests/run_module_acceptance.py
python3 tests/run_dart_mapping_acceptance.py
python3 tests/check_editor_assets.py
python3 benchmarks/json_benchmark.py
```

`run_pi_mono_acceptance.py` additionally compiles the agent scheduler, injects
it through pi-agent-core's `AgentLoopStateMachine` option, and runs a complete
model/tool/model loop against the local pi-mono checkout. Set `PI_MONO` when the
checkout is outside the default sibling repository layout.

Tests use ordinary typed functions with a `test` modifier. A test takes no
parameters and returns `Bool`; both `test fn` and `test async fn` are supported.
`oche test` generates a small runner and executes it directly on the Dart VM in
JIT mode, reporting each test and returning a non-zero status when one fails.

## Editor integrations

Editor integrations live under `editors/`. The VS Code extension and the Zed
development extension both start the same `oche lsp` process. For Zed, build
the CLI, put `target/debug` on the project `PATH`, then install `editors/zed`
with **Install Dev Extension** from `zed: extensions`.

## Targets

### `build-exe`

Uses the same generated program and the SDK's AOT compiler to produce a
self-contained native executable.

### `serve`

Compiles public primitive functions to Kernel and starts one persistent Dart VM
JIT process. TypeScript and other hosts send newline-delimited JSON requests
over standard input and receive correlated responses on standard output. The
process accepts multiple calls, including async Oche functions, without
recompiling or restarting the VM between transitions.

### `build-js`

Produces an optimized Dart-to-JavaScript artifact and a small ESM wrapper.
JavaScript can call `pub fn` and `pub async fn` declarations through the
wrapper's default export; private helpers remain inside the generated Dart
program. The first boundary deliberately accepts only `Int`, `Float`, `Bool`,
and `String`; structured agent values will follow as an explicit protocol
instead of exposing generated Dart objects. Pass `--single` before `-o` to embed
the optimized Dart output in one `.mjs` file. This form is intended for npm
assets and other hosts that cannot ship the wrapper beside a generated
`*.dart.js` file.

### `build-module`

Emits an unlinked Kernel component. Its `register` entry point offers every
function through a host-provided registrar callback; the module does not
receive the tool map, and the test host rejects duplicate names. The module probe
demonstrates the full path through the Rust binding in the same isolate group:
load Kernel, finalize loading, call `register`, and invoke registered closures.
It also drives the Oche agent scheduler through model and queue transitions.
This is a trusted in-process extension mechanism, not a security sandbox.
