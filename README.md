# Oche /ˈɒki/

An experimental agent language frontend and Dart JIT host. An oche is the line
behind which a darts player stands to throw. This directory is independent of
the SDK's build and does not modify the SDK runtime.

The language direction and 0.1 convergence contract are documented in
[`LANGUAGE.md`](LANGUAGE.md): Dart's runtime and ecosystem with a
Rust-influenced syntax, postfix types, `let mut`, expression patterns, `Option`/`Result`,
and deterministic built-in derivation. Explicitly deferred experiments are
tracked in [`TODO.md`](TODO.md) so they do not expand the current language core.

## Current state

- `crates/dart-vm`: bindgen-generated Dart C ABI plus a hand-written safe wrapper
  for an already-entered Dart JIT isolate. `DART_SDK_VERSION` pins the header
  version used for the checked-in raw bindings; scoped handles, VM errors,
  Kernel loading, primitive invocation, and byte-buffer finalizers remain in the
  reviewed wrapper.
- `crates/frontend`: lexer, AST, parser, postfix generic types, function and
  variable resolution, structs, `const fn`, and comptime
  declarations, and early semantic checks.
- `crates/hir`: typed expressions, module-qualified callable and data identities,
  local generic variant inference, and signatures produced by comptime generators
  and `comptime` declarations.
- `crates/formatter`: comment-preserving, width-aware formatting built on
  Wadler-style document combinators.
- `crates/analysis`: stable file identities, source revisions, shared compiler
  snapshots, spans, and staged diagnostics for the CLI and LSP.
- `crates/comptime`: bootstrap derivation plus value-level `const fn` requests.
  The CLI runs scalar requests in the build-time Dart VM and substitutes typed
  literals into the syntax tree before final HIR lowering.
- `crates/codegen`: deterministic typed-HIR lowering to Dart. Resolved callable
  and field identities are preserved through emission; indexed Dart identifiers
  cannot collide with Dart keywords.
- `crates/interop`: narrow Dart-to-Oche definition generation for typed,
  namespaced library calls.
- `crates/lsp`: a dependency-light LSP server with diagnostics, hover help, and
  relative linking of Dart definitions and transitive Oche modules across open
  workspace documents and disk.
- `crates/cli`: inspection, Kernel/AOT build, JIT execution, and LSP commands.
- `agent`: a completed validation slice in which a TypeScript host drives an
  Oche state machine that models pi-agent-core's outer scheduling loop as
  explicit effects.
- `editors/vscode`: TextMate syntax highlighting and a VS Code LSP client.
- `tests`: real VM/Rust FFI integration probes, including an Oche agent Kernel
  driven through model and queue transitions in one live JIT isolate.

Errors use SNAFU across the frontend, CLI, and VM binding. Frontend diagnostics
are typed, expression type errors retain byte spans, and the CLI adds
file/line/column context, while
I/O, process, UTF-8, compiler, and VM failures retain their source chains.

```sh
cargo run --manifest-path oche/Cargo.toml -p oche -- sdk-info
cargo run --manifest-path oche/Cargo.toml -p oche -- bindgen-dart tools.dart -o tools.d.oche
cargo run --manifest-path oche/Cargo.toml -p oche -- fmt oche/examples/arithmetic.oche
cargo run --manifest-path oche/Cargo.toml -p oche -- fmt --check oche/examples/*.oche
cargo run --manifest-path oche/Cargo.toml -p oche -- check oche/examples/arithmetic.oche
cargo run --manifest-path oche/Cargo.toml -p oche -- ast oche/examples/arithmetic.oche
cargo run --manifest-path oche/Cargo.toml -p oche -- emit-dart oche/examples/arithmetic.oche
cargo run --manifest-path oche/Cargo.toml -p oche -- build oche/examples/arithmetic.oche
cargo run --manifest-path oche/Cargo.toml -p oche -- build-module oche/examples/arithmetic.oche -o tool.dill
cargo run --manifest-path oche/Cargo.toml -p oche -- serve oche/agent/pi_core.oche
cargo run --manifest-path oche/Cargo.toml -p oche -- build-exe oche/examples/arithmetic.oche
cargo run --manifest-path oche/Cargo.toml -p oche -- build-js oche/examples/js_module.oche -o core.mjs
cargo run --manifest-path oche/Cargo.toml -p oche -- run oche/examples/arithmetic.oche add 20 22
cargo run --manifest-path oche/Cargo.toml -p oche -- test oche/examples/testing.oche
cargo run --manifest-path oche/Cargo.toml -p oche -- lsp
cargo run --manifest-path oche/Cargo.toml -p xtask -- dart-bindings --sdk "$(mise where dart)"
cargo run --manifest-path oche/Cargo.toml -p xtask -- dart-bindings --sdk "$(mise where dart)" --check
cargo test --manifest-path oche/Cargo.toml --workspace
cargo clippy --manifest-path oche/Cargo.toml --workspace --all-targets -- -D warnings
python3 oche/tests/run_vm_probe.py
python3 oche/tests/run_oche_module_probe.py
python3 oche/tests/run_comptime_acceptance.py
python3 oche/tests/run_comptime_values_acceptance.py
python3 oche/tests/run_tagged_union_acceptance.py
python3 oche/tests/run_match_guard_acceptance.py
python3 oche/tests/run_async_acceptance.py
python3 oche/tests/run_isolate_acceptance.py
python3 oche/tests/run_list_acceptance.py
python3 oche/tests/run_js_module_acceptance.py
python3 oche/tests/run_agent_machine_acceptance.py
python3 oche/tests/run_jit_agent_machine_acceptance.py
python3 oche/tests/run_pi_mono_acceptance.py
python3 oche/tests/run_dart_interop_acceptance.py
python3 oche/tests/run_module_acceptance.py
python3 oche/tests/run_dart_mapping_acceptance.py
python3 oche/tests/check_editor_assets.py
```

Editor integrations live under `editors/`. The VS Code extension and the Zed
development extension both start the same `oche lsp` process. For Zed, build
the CLI, put `target/debug` on the project `PATH`, then install `editors/zed`
with **Install Dev Extension** from `zed: extensions`.

## Initial syntax slice

```text
fn add(a: Int, b: Int) -> Int { a + b }
fn offset(value: Int) -> Int { (value + 2) * 3 }

async fn later(value: Int) -> Int { value + 1 }
async fn chained(value: Int) -> Int { later(value).await + 1 }
```

Currently supports `Int`, `Float`, `Bool`, `String`, typed `List<T>` literals, named-field structs, generic tagged unions,
`let`, `let mut`, assignment, signed 64-bit integer literals, finite double-precision float literals, function calls, unary minus,
`+`, `-`, `*`, strings, booleans, expression `match`, constructor patterns,
`try` blocks, `?` propagation, parentheses, and line comments. `Option<T>` and
`Result<T, E>` are prelude enums using the same sealed-class lowering as user
enums. `async fn` returns `Future<T>` to callers while its body returns `T`;
List elements are statically checked; non-empty literals infer their element type,
while empty literals require an expected `List<T>` type. `left + right` creates a
new list when both operands have the same list type; `.len()` returns its element
count, `.first()` and `.last()` return `Option<T>`, and `.rest()` supports
recursive traversal. String literals can be used in match
patterns. Async values are awaited with the postfix `.await` operator. Awaiting
is checked to occur only inside an async function and can consume both Oche async
calls and Dart functions declared as returning `Future<T>`. `import "std/isolate"
as isolate` exposes `isolate.run(expression)`, which runs the expression through
Dart's `Isolate.run` and returns `Future<T>`. If the expression already returns
`Future<T>`, the result remains `Future<T>`. This API is available on Dart Native
targets (JIT, Kernel, and AOT); JavaScript builds reject it because Dart Web does
not implement isolates.
Matches are checked against their typed subject, including nested enum payloads,
booleans, and struct fields; arms may use a typed `if` guard. Guarded arms do not
count as exhaustive coverage, and arms already covered by earlier unguarded
patterns are rejected. Matches lower to Dart object patterns and `when` clauses.
Native Dart VM `int` arithmetic gives this MVP signed 64-bit wraparound behavior.
`Float` maps to Dart `double` and JavaScript `number`; arithmetic requires
matching numeric types and does not implicitly convert between `Int` and `Float`.
`comptime jsonEncode(Type)` and `comptime jsonDecode(Type)` independently generate
JSON encoding and checked decoding for primitive fields, nested structs, and `List<T>` values;
`import "std/json" as json` provides the preferred collision-free namespace API:
`json.encode(type User, user)`, `json.decode(type User, source)`, and dynamic
`JsonValue` parsing automatically request the required generators.
`comptime validate(Type, ...)` generates `non_empty`
and inclusive integer `range(min, max)` validation. Dart definition files are
imported through a mandatory alias, so `import "tools.d.oche" as tools` exposes
only qualified calls such as `tools.greet(name)`. The typed HIR checks every
currently supported expression and infers generic variants such as `Some(1)` in
local bindings. Transitive aliased Oche modules compile across Dart JIT, AOT,
Kernel module, JavaScript, RPC, and test entry points. Cross-module APIs are
currently limited to recursively composed core and prelude types; general
inference is intentionally outside the 0.1 contract.
Diagnostics include source locations; unknown names/types, wrong arity,
duplicate declarations, out-of-range literals, and excessive nesting are rejected.

Value-level comptime uses the ordinary expression form `comptime function()`.
Its initial boundary accepts a direct synchronous `const fn` returning `Int`,
`Float`, `Bool`, `String`, or a recursively composed `List<T>` of those values.
Named structs whose fields recursively use supported value types are supported
as well. Parameters use the same value types, and each argument must currently
be a literal. The compiler emits a temporary
typed Dart runner, compiles it to Kernel, executes it in the build-time Dart VM,
decodes the scalar result, and replaces the comptime expression with an Oche
literal. Runtime Dart and AOT artifacts therefore contain the value rather than
an evaluator or source string. LSP analysis type-checks the expression and
reports boundary errors without executing user code.

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

The frontend and VM will be versioned together because Kernel is an internal
format. The first backend deliberately uses generated Dart as a narrow bridge
to the SDK's Kernel compiler. It already provides an executable
Oche → syntax AST → typed HIR → Dart → Kernel → Dart VM path. Runtime function
bodies are emitted directly from typed HIR. A direct Kernel serializer
can replace that bridge after the language semantics settle, without changing
the parser or CLI contract. `build-exe` uses the same generated program and the
SDK's AOT compiler to produce a self-contained native executable.

`serve` compiles public primitive functions to Kernel and starts one persistent
Dart VM JIT process. TypeScript and other hosts send newline-delimited JSON
requests over standard input and receive correlated responses on standard
output. The process accepts multiple calls, including async Oche functions,
without recompiling or restarting the VM between transitions.

`build-js` produces an optimized Dart-to-JavaScript artifact and a small ESM
wrapper. JavaScript can call `pub fn` and `pub async fn` declarations through
the wrapper's default export; private helpers remain inside the generated Dart
program. The first boundary deliberately
accepts only `Int`, `Float`, `Bool`, and `String`; structured agent values will follow as
an explicit protocol instead of exposing generated Dart objects.
Pass `--single` before `-o` to embed the optimized Dart output in one `.mjs`
file. This form is intended for npm assets and other hosts that cannot ship the
wrapper beside a generated `*.dart.js` file.

`agent/pi_core.oche` is the first incremental pi-agent-core replacement. Oche
owns the phase, turn number, continuation rule, and next effect. The TypeScript
driver performs model, tool, queue, and cancellation I/O, then reports each
result back to Oche. State crosses the initial primitive JS ABI as checked JSON;
the acceptance test covers plain responses, tool calls, steering, follow-ups,
termination, model failure, and invalid transitions.
`run_pi_mono_acceptance.py` additionally compiles that scheduler, injects it
through pi-agent-core's `AgentLoopStateMachine` option, and runs a complete
model/tool/model loop against the local pi-mono checkout. Set `PI_MONO` when the
checkout is outside the default sibling repository layout.

`build-module` emits an unlinked Kernel component. Its `register` entry point
offers every function through a host-provided registrar callback; the module
does not receive the tool map, and the test host rejects duplicate names. The module probe demonstrates
the full path through the Rust binding in the same isolate group: load Kernel,
finalize loading, call `register`, and invoke registered closures. It also
drives the Oche agent scheduler through model and queue transitions.
This is a trusted in-process extension mechanism, not a security sandbox.

## Design references

[Lisette](https://github.com/ivov/lisette) is a useful reference because it is
also a Rust frontend targeting an existing language ecosystem. The review used
commit `8155fcef4402de86e0722b367fdcd8894c631aae` (MIT). Oche follows its broad
separation of syntax, semantic passes, emission, diagnostics, and CLI pipeline.
No Lisette source has been copied or added as a dependency: its Go-specific AST,
ABI planner, and backend are too coupled to Lisette to reuse directly. Parser
recovery, accumulated diagnostics, stable diagnostic codes, and snapshot-style
codegen tests are good candidates to adopt as Oche grows.

Tests use ordinary typed functions with a `test` modifier. A test takes no
parameters and returns `Bool`; both `test fn` and `test async fn` are supported.
`oche test` generates a small runner and executes it directly on the Dart VM in
JIT mode, reporting each test and returning a non-zero status when one fails.
