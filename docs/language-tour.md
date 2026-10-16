# Language tour

The 0.1 convergence contract lives in [`../LANGUAGE.md`](../LANGUAGE.md); this
page walks the same surface with runnable examples from `examples/`.

## Functions

```oche
fn add(a: Int, b: Int) -> Int { a + b }

async fn later(value: Int) -> Int { value + 1 }
async fn chained(value: Int) -> Int { later(value).await + 1 }
```

`async fn` returns `Future<T>` to callers while its body returns `T`. Async
values are awaited with the postfix `.await` operator; awaiting is checked to
occur only inside an async function and can consume both Oche async calls and
Dart functions declared as returning `Future<T>`.

## Types

Currently supports `Int`, `Float`, `Bool`, `String`, typed `List<T>` literals,
named-field structs, and generic tagged unions.

- Native Dart VM `int` arithmetic gives signed 64-bit wraparound behavior.
- `Float` maps to Dart `double` and JavaScript `number`; arithmetic requires
  matching numeric types and never implicitly converts between `Int` and
  `Float`.
- List elements are statically checked; non-empty literals infer their element
  type, while empty literals require an expected `List<T>` type. `left + right`
  creates a new list when both operands have the same list type; `.len()`
  returns the element count, `.first()` and `.last()` return `Option<T>`, and
  `.rest()` supports recursive traversal.

## Enums, `Option`, and `Result`

```oche
enum Message {
  Text(String),
  Number(Int),
  Stop,
}
```

`Option<T>` and `Result<T, E>` are prelude enums using the same sealed-class
lowering as user enums. String literals can be used in match patterns.

## Matching

```oche
fn choose(value: Option<Int>) -> Int {
  match value {
    Some(inner) if selected(inner) => inner + 10,
    Some(_) => 0,
    None => -1,
  }
}
```

Matches are checked against their typed subject, including nested enum
payloads, booleans, and struct fields; arms may use a typed `if` guard. Guarded
arms do not count as exhaustive coverage, and arms already covered by earlier
unguarded patterns are rejected. Matches lower to Dart object patterns and
`when` clauses.

## Error handling

```oche
fn add_positive(left: Int, right: Int) -> Result<Int, String> {
  try {
    let a = positive(left)?;
    let b = positive(right)?;
    a + b
  }
}
```

`try` blocks and `?` propagation capture `Option` or `Result` flow; `try` is
restricted to a block that propagates with `?`.

## Code generation

`quote! { expression }` creates a frontend-only local code value;
`splice!(quote! { expression })` consumes it directly, while
`let code = quote! { ... }; splice!(code)` permits local reuse. Expansion
happens before name resolution and type checking, including when the quote
contains local statements. These restricted `Code<T>` values cannot cross
function boundaries, enter runtime data, or execute generator code. General
comptime, `const fn`, reflection-driven declaration generation, and macros are
deferred beyond 0.1; see [`../TODO.md`](../TODO.md).

## JSON

`import "std/json" as json` provides the collision-free namespace API:

```oche
fn encode_user(name: String, age: Int, active: Bool) -> String {
  json.encode(type User, User { name: name, age: age, active: active })
}
```

`json.encode(type User, user)` and `json.decode(type User, source)`, plus
dynamic `JsonValue` parsing, request compiler-owned typed helpers as needed.
Typed encoding writes JSON directly and typed decoding constructs structs from
a streaming reader, avoiding an intermediate dynamic object tree. Field
validation derivation is deferred beyond the 0.1 core.

## Isolates

```oche
import "std/isolate" as isolate

fn double(value: Int) -> Int { value * 2 }

async fn isolatedDouble(value: Int) -> Int {
  isolate.run(double(value)).await
}
```

`isolate.run(expression)` runs the expression through Dart's `Isolate.run` and
returns `Future<T>`. If the expression already returns `Future<T>`, the result
remains `Future<T>`. This API is available on Dart Native targets (JIT, Kernel,
and AOT); JavaScript builds reject it because Dart Web does not implement
isolates.

## Imports

Oche modules and Dart definition files are both imported through a mandatory
alias: `import "tools.d.oche" as tools` exposes only qualified calls such as
`tools.greet(name)`. The typed HIR checks every currently supported expression
and infers generic variants such as `Some(1)` in local bindings. Transitive
aliased Oche modules compile across Dart JIT, AOT, Kernel module, JavaScript,
RPC, and test entry points. Cross-module APIs are currently limited to
recursively composed core and prelude types; general inference is intentionally
outside the 0.1 contract.

## Diagnostics

Diagnostics include source locations; unknown names/types, wrong arity,
duplicate declarations, out-of-range literals, and excessive nesting are
rejected.

## FFI

Native FFI is part of the 0.1 foundation and is deliberately a thin typed layer
over `dart:ffi`. The intended production path is Oche → generated Dart FFI
binding → a Rust `extern "C"` API, with JIT/AOT parity and an explicit ownership
contract for every pointer. The existing Rust/Dart VM probes exercise the
reverse embedding direction; they do not yet constitute the source-level native
binding API. See [`../FFI.md`](../FFI.md) for the first implementation
milestones.
