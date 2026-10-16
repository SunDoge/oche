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
- `value |> transform(argument)` inserts `value` as the first call argument. The
  right side must be a call, may be module-qualified, and pipe has lower
  precedence than arithmetic operators.
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
- compile-time evaluation runs ordinary Oche `const fn` code; `const` is an
  author assertion in 0.1 rather than a compiler-proven effect classification;
  generators such as `comptime jsonEncode(User)` are compile-time calls that
  materialize typed HIR declarations;
- tests are parameterless `test fn` or `test async fn` declarations returning
  `Bool`.

Oche 0.1 does not include classes, inheritance, mixins, dynamically dispatched methods,
traits, interfaces, implementation blocks, operator overloading, implicit
numeric or error conversions, nullable core types, `dynamic`, general type
inference, first-class functions, mutable fields or collections, macros, or
isolate-specific syntax.
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
The backend does not know about JSON, validation, tools, RPC schemas, or agent
protocols. Those features are Oche libraries evaluated through `comptime` and
return compiler-owned typed declarations, which then pass through the ordinary
name resolution, type checking, and Dart lowering pipeline.

`comptime` extends the program after parsing; it does not extend the parser.
Features described as complex "syntax" should therefore normally be explicit
generator calls such as `comptime jsonEncode(User)`, backed by ordinary Oche code.
Oche does not add token macros, quoted source, string evaluation, or a separate
template language. The metaprogramming boundary consists of reflected `Type`
and `Field` values plus a small typed declaration IR.

```text
source → syntax AST → module linking → base typed HIR
       → comptime Oche → Dart → Kernel → build-time Dart VM
       → generated declarations/values → merged typed HIR → final Dart
       → Kernel → Dart VM JIT (primary runtime)
                ↘ Dart AOT
                ↘ JavaScript → ESM host
```

`comptime` belongs between semantic analysis and runtime lowering. It executes
the same expressions, control flow, functions, and type rules as runtime Oche.
Functions eligible for that evaluator use `const fn`; they remain ordinary
runtime-callable functions. In 0.1 this modifier is the author's assertion that
the function is deterministic and capability-free. The compiler does not build
a general effect system or prove purity. Async functions are rejected at the
comptime boundary. Capability filtering and finite recursion, instruction, and
allocation budgets have not been implemented yet; until then, imported Dart
calls reachable from a `const fn` carry the same authority as the compiler
process.

The first value-level form is deliberately narrow:

```text
const fn scale(value: Int, factor: Int) -> Int { value * factor }
fn configured_capacity() -> Int { comptime scale(16384, 2) }
```

It accepts direct calls to synchronous `const fn` declarations returning `Int`,
`Float`, `Bool`, `String`, recursively composed `List<T>` values, or named
structs whose fields use those same types. Parameters use the same value types.
The first implementation accepts literal arguments;
constant expression arguments will follow. Every call site has its own request
identity, so two invocations of the same function with different arguments
cannot share a result accidentally.
The build-time Dart VM returns a typed scalar through a JSON protocol, and the
compiler replaces the expression with an AST literal before final HIR lowering.
User code does not return generated source, and no source string is evaluated.
Editors validate and type-check this form without running the function.
Type reflection, generated declarations, caching, and execution budgets are
later stages of the same pipeline.

The reflection kernel uses `type T` to produce a compile-time `Type` value.
`Type` exposes `name`, `arguments`, and `fields`; each `Field` exposes `name`
and `ty`. These are compiler-owned read-only values rather than syntax strings.

Generated functions use a composable builder API over the compiler-owned flat
declaration protocol. An expression builder owns its nodes and root; combining
builders rebases child indexes internally. Function generators therefore build
the body first and finish it through a declaration builder, for example
`functionBuilder(name, parameters, result) |> returns(body)`. Visibility,
type parameters, async state, and attributes can later extend that declaration
builder without exposing protocol indexes or requiring syntax-tree strings in
user generators.

Types and their struct fields or enum variants become compile-time values
through a typed reflection API. Explicit calls such as `comptime jsonEncode(User)`
pass the named type to a `const fn`. Generated declarations are
inserted into typed HIR before ordinary calls are resolved, so diagnostics,
hover, completion, and backend lowering use the same symbol table.

The compiler lowers comptime Oche through the same Dart backend used for runtime
code, compiles it to Kernel, and invokes it in a build-time Dart VM isolate. The
result is then merged into typed HIR before final code generation. This remains
the pipeline when the requested final artifact is AOT: Dart VM executes comptime
first, then the expanded Dart program is handed to the AOT compiler. Oche does
not maintain a second interpreter with subtly different arithmetic, matching,
or collection semantics.

The JSON generators are ordinary Oche `const fn` libraries in `stdlib/json.oche`.
They use type reflection and the typed declaration builder protocol, execute in
the build-time Dart VM, and return declarations that pass through the normal HIR
and backend. Validation still uses its bootstrap implementation.

Oche uses `let` for a single-assignment local and `let mut` when the binding must
be reassigned. Mutable bindings initially require a single identifier, retain
their inferred type, and lower directly to Dart local variables; immutable
bindings lower to Dart `final`. Oche has no separate `var` declaration, mutable
struct field, or module-level named constant yet. A `let` becomes a compile-time
value when evaluated inside comptime and a runtime value otherwise. `const fn`
marks evaluation eligibility; it does not introduce a second kind of stored
value.

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

The executable mapping suite exercises these representations through both
`oche run` and `oche build-exe`. A language feature is not part of the core
contract until it has JIT/AOT parity coverage or an explicit backend-boundary
restriction.

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

The language server does not start a fresh Dart VM or rerun every `const fn` on
each edit. Parsing and base type checking update immediately. Comptime expansion
runs after a debounce only when a generator, its typed inputs, or its transitive
dependencies change. A persistent worker caches successful generated
declarations by compiler version and dependency/input hashes; while it
recomputes, analysis may use the last successful expansion and mark it stale.
Worker time, allocation, recursion, and generated-output budgets keep editor
latency bounded. Build and save-time analysis always require a current
expansion.

```text
struct User {
  name: String,
  age: Int,
}

comptime jsonEncode(User)
comptime jsonDecode(User)
comptime validate(User, name: non_empty, age: range(0, 130))

fn encode(user: User) -> String { user.toJson() }

fn decode(source: String) -> Result<User, String> { User.fromJson(source) }
```

The two JSON directives invoke independent Oche compile-time generators, so a
program only materializes the direction it requests. They produce
compiler-reserved codec functions. Static call sugar resolves `user.toJson()`
and `User.fromJson(source)` to those functions without runtime dispatch.
Decoding and validation return `Result` so malformed input
stays in the language's normal control flow. JSON generation accepts primitive `Int`, `Float`,
`Bool`, and `String` fields plus nested derived structs and lists. Validation
supports `non_empty` for strings and inclusive `range` for integers.

### JSON namespace

The directives and generated member-call sugar above remain available as a
bootstrap API. New code imports the built-in JSON namespace. Typed calls request
their encode or decode generator automatically, so generated function names do
not enter the source-level API and cannot collide with user methods. Requests
also follow nested struct and `List<T>` fields transitively while preserving the
requested direction.

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
result type before the generator executes. Oche 0.1 will not infer a decode
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

At compilation, typed JSON calls specialize through ordinary Oche comptime
reflection and lower to concrete private Dart helpers. The final Dart program
contains no reflected `Type`, field table, trait object, or dynamic method call.
The language server can determine the call signature from the explicit type
argument immediately and cache the later comptime expansion independently.

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

The `oche lsp` server publishes parser, comptime, and HIR type diagnostics. It
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
and comptime evaluator.
