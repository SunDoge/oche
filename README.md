# Oche /ˈɒki/

An experimental language that compiles to the Dart runtime.

Oche is a Rust-influenced frontend over Dart's VM, AOT toolchain, and ecosystem:
modern syntax up front, mature runtime underneath. An oche is the line behind
which a darts player stands to throw.

## Safe and expressive

- `let` / `let mut`, postfix types, expression-oriented bodies
- `Option<T>` and `Result<T, E>` with `match`, `try`, and `?` — no nullable core
  types, no `dynamic`
- Generic tagged unions and named-field structs with static exhaustiveness
  checking, including guarded arms
- Typed `List<T>`, `async fn` with postfix `.await`, and Dart isolates through
  `isolate.run(...)` for real parallelism
- Compiler-owned JSON derivations: `json.encode(type User, user)` and
  `json.decode(type User, source)` with no external code-generation step
- A reserved `name!` compiler-form syntax class, starting with `quote! { ... }`
  and `splice!(...)` for compile-time expression construction

## Quietly practical

- The full Dart ecosystem: pub packages, `dart:ffi` for native calls, and the
  same VM-backed interop surface
- One tool: `oche check` / `run` / `test` / `fmt` / `lsp` / `build` (Kernel) /
  `build-exe` (AOT) / `build-js` (ESM)
- A dependency-light LSP server with diagnostics, hover, go-to-definition,
  references, symbols, folding, and formatting — same binary for every editor
- Runs on the Dart VM JIT, compiles to self-contained native executables, and
  ships to JavaScript as an ESM module

## Quick tour

```oche
fn add(a: Int, b: Int) -> Int { a + b }
```

Tagged unions with exhaustive matching:

```oche
enum Message {
  Text(String),
  Number(Int),
  Stop,
}
```

`Result`, `try`, and `?`:

```oche
fn add_positive(left: Int, right: Int) -> Result<Int, String> {
  try {
    let a = positive(left)?;
    let b = positive(right)?;
    a + b
  }
}
```

Typed JSON without code generation:

```oche
import "std/json" as json

struct User { name: String, age: Int, active: Bool }

fn encode_user(user: User) -> String { json.encode(type User, user) }

fn decode_user(source: String) -> Result<User, String> {
  json.decode(type User, source)
}
```

Isolates for parallelism:

```oche
import "std/isolate" as isolate

fn double(value: Int) -> Int { value * 2 }

async fn isolated_double(value: Int) -> Int {
  isolate.run(double(value)).await
}
```

The agent scheduling kernel — Oche decides the next effect, the host performs
the I/O — is a working slice under [`agent/`](agent).

## Learn more

- 📖 [Language tour](docs/language-tour.md) — the 0.1 surface with runnable
  examples
- 📐 [Language contract](LANGUAGE.md) — the 0.1 convergence rules and explicit
  deferrals ([TODO.md](TODO.md))
- 🔧 [Building and tooling](docs/tooling.md) — mise, SDK discovery, CLI
  commands, acceptance probes, editor setup
- 🏗️ [Architecture](docs/architecture.md) — crate map, pipeline, versioning,
  design references
- 🤖 [Agent runtime](docs/agent.md) — the pi-agent-core validation slice
- 🔌 [FFI milestones](FFI.md) — the native binding direction
