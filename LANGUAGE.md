# Oche language direction

Oche is a Dart-family language for agent runtimes. It keeps Dart's execution
model and ecosystem—VM, isolates, async, libraries, FFI, JIT, and AOT—while
using a smaller Rust-influenced surface syntax.

The Dart VM JIT is the primary runtime target. Runtime design and performance
work prioritize fast iteration, long-lived agent processes, dynamic modules,
memory use, and call overhead. AOT remains a compatibility path rather than a
constraint on the language model.

## Surface conventions

- Type names use `UpperCamelCase`; functions, methods, fields, parameters, and
  local bindings use `lowerCamelCase`. File names may use `snake_case`.
  Identifiers containing underscores remain valid for interop and migration.
- Functions use `fn`, parameter types are postfix, and return types follow `->`.
- `pub fn` and `pub async fn` define the callable JavaScript module boundary;
  unmarked functions remain implementation details.
- `let` bindings are immutable; reassignment requires `let mut`.
- `match` is an expression and uses structural patterns with optional `if`
  guards.
- `Option<T>` and `Result<T, E>` are non-nullable tagged values. `Some`, `None`,
  `Ok`, and `Err` are constructors and patterns.
- `List<T>` maps directly to Dart's typed list representation. List literals are
  immutable Oche expressions even though Dart's initial backing object is mutable;
  `+` concatenates two lists into a new value, `.len()` returns the element count,
  `.first()` and `.last()` safely return `Option<T>`, and `.rest()` returns all
  elements after the first for recursive traversal.
- `Float` maps to Dart `double` and JavaScript `number`. Numeric operations keep
  `Int` and `Float` distinct; conversion must be explicit when conversion syntax
  is added.
- Dart interop should remain direct. Oche types and calls must have predictable
  Dart representations rather than a second object runtime.
- Imports require a module alias. They never inject another library's names into
  the current scope.
- `async`/`await` should preserve Dart scheduling and `Future` semantics.
- Native concurrency starts as a library API: `isolate.run(expression)` lowers
  directly to Dart's `Isolate.run(() => expression)` and returns `Future<T>`.
  The expression is evaluated in the worker isolate. No isolate-specific syntax
  is part of 0.1.
- Native interop is a 0.1 capability implemented as a thin, typed surface over
  `dart:ffi`, not as a separate Oche FFI runtime. Oche reuses Dart's native
  types, loading, symbol lookup, calls, pointers, and finalizers; bindings are
  statically checked and unavailable on the JavaScript target. The first boundary covers
  fixed-width scalars, byte buffers, UTF-8 strings through explicit adapters,
  and opaque owned or borrowed handles. Native ownership and release functions
  must be visible in a binding rather than inferred from pointer types.
- `value |> transform(argument)` inserts `value` as the first call argument. The
  right side must be a call, may be module-qualified, and pipe has lower
  precedence than arithmetic operators.
- `quote { expression }` creates a local compile-time code value and `splice`
  inserts it before name resolution and type checking. A quote may be consumed
  directly or bound with immutable `let`: `let code = quote { value + 1 };
  splice code`. Ordinary identifiers inside it resolve at the splice site. Code
  values never reach Dart and cannot yet cross a function boundary.
- Tests use the single canonical form `test [async] fn`; `test` is always a
  prefix modifier.

```text
fn unwrapOrZero(value: Option<Int>) -> Int {
  match value {
    Some(inner) => inner,
    None => 0,
  }
}
```

A guarded arm uses `pattern if condition => value`. Its condition is checked as
`Bool` in the pattern-binding scope. Because it may reject a matching value, a
guarded arm does not contribute to exhaustiveness; a later unguarded fallback is
still required.

`Option` and `Result` are prelude tagged unions, not privileged object types.
They use the same lowering as a user declaration such as `enum Message {
Text(String), Stop }`: one sealed Dart base plus one final class per variant.
Oche patterns lower to Dart object patterns. This representation is an MVP
contract; specialized or unboxed representations can be introduced behind the
lowering boundary.

`?` propagates `None` or `Err` from a function. A `try` block captures that
propagation locally and wraps its tail value in the matching success variant:

```text
fn combine(left: Int, right: Int) -> Result<Int, String> {
  try {
    let a = positive(left)?;
    let b = positive(right)?;
    a + b
  }
}
```

The current Dart lowering uses a private control-flow sentinel caught at the
nearest `try` block or function boundary. It never catches user exceptions.

## 0.1 convergence contract

The 0.1 language is a small, statically typed expression language. New runtime
experiments should be expressed through libraries and host protocols until they
demonstrate a missing language primitive. The following surface is the baseline
for implementation work:

- declarations have explicit parameter and result types; inference stays local
  to expressions and generic enum constructors;
- structs are record-like values with named fields and named construction;
  enums are tagged unions with positional variant payloads;
- function declarations are top-level; `value.method()` and `Type.function()`
  provide statically resolved call syntax for type-prefixed functions;
- `match`, immutable `let`, explicit `let mut`, assignment, `Option`, `Result`,
  `try`, and `?` provide ordinary data and error control flow;
- async functions expose `Future<T>` to callers, declare their body result as
  `T`, and use postfix `.await` only;
- imports are always aliased and references across a module boundary are always
  qualified;
- native definition modules describe C symbols separately from ordinary Oche
  source, use exact ABI types, and expose an idiomatic checked Oche signature;
  Rust libraries are expected to export `extern "C"` functions with stable,
  unmangled symbol names;
- compiler-owned derivations may synthesize typed helpers for standard-library
  features such as JSON, but 0.1 exposes no general comptime or macro system;
- `quote` and `splice` may construct and locally reuse expression AST before
  checking; code values cannot generate declarations, cross a function
  boundary, enter runtime data, or execute user code;
- tests are parameterless `test fn` or `test async fn` declarations returning
  `Bool`.

Oche 0.1 does not include classes, inheritance, mixins, dynamically dispatched methods,
traits, interfaces, implementation blocks, operator overloading, implicit
numeric or error conversions, nullable core types, `dynamic`, general type
inference, first-class functions, mutable fields or collections, `const fn`,
`comptime`, macros, or isolate-specific syntax.
These are design deferrals rather than placeholders that each need to be filled.

In particular, a Rust-style trait system is not part of the initial language.
It would require method lookup, generic bounds, coherence rules, dispatch
semantics, and a decision about associated types before Oche has a concrete use
case for them. If agent libraries later require nominal runtime polymorphism,
the smallest candidate is an `interface` declaration implemented explicitly by
a struct and lowered to Dart's `abstract interface class`. Its first version
would have required methods only: no default bodies, associated types, blanket
implementations, or implicit conformance. Until at least two real
implementations need that abstraction, namespaced free functions and tagged
unions remain the language model.

Exceptions from Dart interop are also not converted to `Result` implicitly.
Doing so would erase which exceptions are expected and could accidentally catch
cancellation or process failures. A future definition file may describe a
specific checked wrapper returning `Result<T, E>`; that conversion remains
visible in the imported signature.

## Compilation stages

The transpiler is intentionally mechanical. Surface declarations, expressions,
patterns, async operations, and core types each have a direct Dart 3 lowering.
Standard-library features may request compiler-owned typed helpers, but those
helpers are an implementation detail and do not expose an AST or declaration IR
to Oche programs.

```text
source → syntax AST → module linking → type inference and checked HIR
       → compiler-owned standard derivations and native bindings → Dart
       → Kernel → Dart VM JIT (primary runtime)
                ↘ Dart AOT
                ↘ JavaScript → ESM host
```

The repository contains an earlier comptime prototype that executes Oche through
the Dart VM. It is retained as implementation research while standard derivations
are moved behind compiler-owned APIs; it is not part of the 0.1 language contract.
General staged evaluation must define typing phases, capabilities, budgets,
caching, reproducibility, source mapping, and LSP execution before it can become
public syntax.

The bounded `quote`/`splice` form stops before staged evaluation. It parses an
expression into the existing AST and removes the generation boundary before
module linking. An immutable local may hold that frontend-only code value and
splice it more than once within its lexical scope. Quoted identifiers use the
ordinary scope at each splice site, and the resulting expression follows the
normal type checker, diagnostics, LSP, and Dart lowering paths. This is the
first restricted `Code<T>` model, although `Code<T>` cannot yet be written in a
function signature or stored inside runtime data.

Oche uses `let` for a single-assignment local and `let mut` when the binding must
be reassigned. Mutable bindings initially require a single identifier, retain
their inferred type, and lower directly to Dart local variables; immutable
bindings lower to Dart `final`. Oche has no separate `var` declaration, mutable
struct field, or module-level named constant yet.

Language additions should have a direct, typed Dart 3 lowering. Features that
would require semantics beyond Dart 3 stay out until concrete user feedback
justifies them. Oche evolves through small end-to-end additions rather than
front-loading a larger independent object or runtime model.

## Dart mapping contract

JIT and AOT consume the same emitted Dart program. Oche does not define a
target-specific branch of its type or control-flow semantics.

| Oche | Dart 3 representation |
| --- | --- |
| `Int` | `int` |
| `Float` | `double` |
| `Bool` | `bool` |
| `String` | `String` |
| `List<T>` | `List<T>` with no mutation exposed by Oche |
| `struct` | `final class` with final typed fields |
| `enum`, `Option`, `Result` | sealed base class plus final variant classes |
| `fn` | statically typed top-level function |
| `async fn -> T` | Dart `Future<T>` function |
| `match` | exhaustive Dart switch expression and object patterns |
| `let` | inferred Dart `final` local |
| `let mut` | inferred Dart `var` local of the checked Oche type |
| `try` and `?` | private typed propagation sentinel caught at the Oche boundary |
| `splice quote { expression }` | the checked Dart expression produced by the ordinary lowering |
| native C ABI declaration | typed `dart:ffi` lookup or native-asset binding |

The executable mapping suite exercises these representations through both
`oche run` and `oche build-exe`. A language feature is not part of the core
contract until it has JIT/AOT parity coverage or an explicit backend-boundary
restriction.

## Native FFI contract

FFI is a backend boundary, not a second dynamic type system or runtime. A native
binding maps mechanically to `dart:ffi` and may pair that representation with a
safer Oche-facing wrapper. ABI integers
use an explicit width and signedness; platform C types such as `long` are not
silently treated as Oche `Int`. Raw addresses do not implicitly become strings,
lists, structs, or managed objects.

The first supported Rust pattern is a C API composed from:

- `extern "C"` exported functions with stable symbols;
- fixed-width integer and floating-point arguments and results;
- `(pointer, length)` byte slices whose borrow lasts for the call;
- opaque handles created and destroyed by named functions;
- owned result buffers paired with an explicit native release function;
- status codes translated by a checked Oche wrapper into `Result`.

JIT and AOT use the same binding signature and ownership rules. Native calls
execute on the calling isolate unless an Oche wrapper explicitly uses
`isolate.run`; a blocking call is never moved automatically. The JavaScript
backend rejects modules that reach native bindings. Callbacks, variadic
functions, unions, bitfields, packed layouts, and structs passed by value are
outside the first FFI milestone.

Binding generation should consume C headers through the mature Dart `ffigen`
toolchain rather than duplicating its C parser, while keeping the small Oche
definition format as the checked public boundary. Rust crates may generate those headers with
`cbindgen`; Oche does not parse Rust source or depend on Rust's unstable ABI.

The HIR assigns every module a stable identity. Resolved source functions,
structs, variants, generated functions, and imported Dart functions combine that
module identity with a local index. Each expression carries its resolved type.
The Dart emitter consumes this typed HIR directly, so generated calls and field
accesses use the same resolved identities checked by analysis.

CLI and editor tooling share an analysis database with stable file IDs and source
revisions. The parser recovers at top-level declarations and reports multiple
syntax errors in one snapshot. Source declarations and their members, every
parsed expression, and every nested pattern carry complete byte spans. Typed HIR
preserves expression and pattern locations, and type diagnostics retain the
innermost failing range. Match exhaustiveness uses the match offset; arm
usefulness highlights the complete redundant pattern.

The language server performs parsing, linking, type analysis, and compiler-owned
derivation in process. It never executes user code to provide diagnostics,
completion, hover, or navigation.

```text
struct User {
  name: String,
  age: Int,
}

import "std/json" as json

fn encode(user: User) -> String { json.encode(type User, user) }

fn decode(source: String) -> Result<User, String> { json.decode(type User, source) }
```

Typed JSON calls request compiler-owned codec helpers. No user comptime
execution is involved. Typed encoding writes directly to a `StringBuffer`
without first allocating a `Map<String, Object?>`; typed decoding reads the JSON
stream and constructs the target struct without first building a complete
dynamic object tree. Decoding returns `Result` so malformed input stays in the
language's normal control flow. JSON derivation accepts primitive `Int`,
`Float`, `Bool`, and `String` fields plus nested derived structs and lists.

### JSON namespace

New code imports the built-in JSON namespace. Typed calls request their encode or
decode helper automatically, so generated function names do not enter the
source-level API and cannot collide with user methods. Requests also follow
nested struct and `List<T>` fields transitively while preserving the requested
direction.

```text
import "std/json" as json
```

Dynamic JSON uses a statically typed recursive `JsonValue` tree:

```text
json.parse(source) -> Result<JsonValue, String>
json.stringify(value) -> String
```

Typed conversion takes the target type as an explicit compile-time value:

```text
json.encode(type User, user) -> String
json.decode(type User, source) -> Result<User, String>
json.toValue(type User, user) -> JsonValue
json.fromValue(type User, value) -> Result<User, String>
```

The same `encode` and `decode` calls accept `type JsonValue` when the shape is
unknown at compile time.

`stringify` and `encode` accept already validated JSON values, so they are
infallible in the 0.1 type model. `parse`, `decode`, and `fromValue` report shape
and input errors through `Result`.

The explicit `type User` argument determines every dependent parameter and
result type before codec derivation. Oche 0.1 will not infer a decode
target from its result location and will not add `anytype`, trait bounds, or
overload resolution for this API. A future shorthand may infer the encode type
from its value argument without changing the underlying model.

`JsonValue` rather than `dynamic` represents an unknown JSON shape. The initial
API keeps object storage behind `JsonObject` and exposes checked operations:

```text
json.asObject(value) -> Result<JsonObject, String>
json.asArray(value) -> Result<List<JsonValue>, String>
json.asInt(value) -> Result<Int, String>
json.asFloat(value) -> Result<Float, String>
json.asBool(value) -> Result<Bool, String>
json.asString(value) -> Result<String, String>
json.field(object, name) -> Result<JsonValue, String>
```

The matching `object`, `array`, `fromInt`, `fromFloat`, `fromBool`, and
`fromString` operations construct checked values. Raw Dart `Object?` stays
inside the backend representation and does not leak into Oche types.

At compilation, typed JSON calls lower to concrete private Dart helpers. The final Dart program
contains no reflected `Type`, field table, trait object, or dynamic method call.
The language server can determine the call signature from the explicit type
argument immediately.

## Dart interop

`oche bindgen-dart tools.dart -o tools.d.oche` generates a declaration file for
the supported public Dart API. A program imports that declaration under an
explicit name:

```text
import "tools.d.oche" as tools

fn greet(name: String) -> String { tools.greet(name) }
```

Unqualified `greet(name)` is rejected. The declaration file records the Dart
library URI and function signatures; it contributes no executable Oche names or
source text. The initial bindgen accepts one-line, top-level functions whose
parameters and result use `int`, `double`, `bool`, `String`, or `Future` of those types.

## Oche modules

An ordinary `.oche` import adds another Oche source module under the required
alias. Imports are resolved relative to the importing file and may be
transitive. A diamond dependency is compiled once, and an import cycle is
reported with the complete path chain. Only `pub fn` declarations are visible
to importing modules:

```text
import "math.oche" as math

fn answer() -> Int { math.double(21) }
```

Module calls retain the target module and function identities through typed HIR
and lower to direct statically typed Dart calls. The initial module ABI accepts
only core and prelude types (`Int`, `Float`, `Bool`, `String`, `List`, `Future`,
`Option`, and `Result`) recursively. Structs and user enums remain private to
their source module until qualified type syntax and exported data declarations
are designed together.

## Tooling

The `oche lsp` server publishes parser, linking, and HIR type diagnostics. It
resolves aliased Dart definitions and transitive Oche modules relative to the
source document from open editor buffers or disk, and reanalyzes open dependents
when any imported document changes. It provides hover help for core types,
keywords and compiler-generated functions, plus whole-document
formatting. The same comment-preserving formatter is available as `oche fmt` and
`oche fmt --check`. The VS Code extension registers `.oche`, ships a TextMate
grammar, and launches that server over stdio.

## Convergence status

The parser, HIR, and Dart lowering support the complete 0.1 convergence
contract, including transitive aliased Oche modules. CLI and editor paths use the
same frontend, linked-package HIR, visibility rules, module-qualified identities,
and compiler-owned derivation pipeline.
