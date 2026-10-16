# Deferred work

Oche is currently converging on its smallest Dart-shaped language core. Items
in this file are design notes, not commitments for the 0.1 grammar. A deferred
feature should only move into the language after ordinary Oche code exposes a
repeated problem that the feature clearly solves.

## Native FFI foundation

FFI is required for 0.1 rather than deferred language work. Keep it as a thin
typed lowering to `dart:ffi`, and build it as small end-to-end increments, each
covered in both Dart VM JIT and AOT:

1. Define native modules, library resolution, symbol names, exact scalar ABI
   types, and clear diagnostics; reject them on the JavaScript target.
2. Add borrowed byte buffers and UTF-8 input adapters whose lifetime is one
   native call.
3. Add opaque handles with explicit create/destroy functions and wrappers that
   prevent ordinary Oche code from manufacturing handles.
4. Add owned native output buffers with required release functions and status
   code to `Result` wrappers.
5. Generate bindings from C headers using `ffigen`, and document a
   `cbindgen`-based Rust crate workflow.

Do not begin with callbacks, variadics, arbitrary pointer arithmetic, inferred
ownership, structs passed by value, unions, or bitfields. Do not automatically
move a blocking FFI call to another isolate; make that scheduling choice visible
in Oche code.

## Staged metaprogramming

Stage 0 is implemented as `splice!(quote! { expression })`. The first local code
value is also available as `let code = quote! { expression }; splice!(code)`.
The trailing `!` is the reserved compiler-form syntax class, so `quote` and
`splice` remain ordinary identifiers and later compiler forms such as `tsx!`
parse without further grammar changes; 0.1 rejects every other `name!` form.
These forms parse a bounded expression or expression block into the ordinary AST
and remove all code values before name resolution and type checking. Code values
are immutable, lexically scoped, reusable, and compile-time only. They cannot
cross function boundaries, enter runtime data, execute generators, or produce
declarations.

Revisit Oche programs generating Oche programs after the 0.1 type system, LSP,
and module model are stable. Dart's shared JIT and AOT semantics remain the main
reason to explore this: compile-time Oche could execute on the Dart VM and emit
code consumed by the same backend.

Do not expose the current `ComptimeDeclarations` protocol as a user API. A
public design must first settle phase ordering, hygienic names, typed reflection,
capability isolation, deterministic dependency tracking, execution budgets,
cancellation, caching, expansion source maps, and identical CLI/LSP behavior.
Compiler-owned JSON, validation, schema, and interop derivations can cover common
0.1 needs without committing the language to a general macro model.

The next useful experiment is making `Code<T>` explicit in function signatures
so generators can compose code values. That requires definition-site hygiene,
typed quote checking independent of a splice context, and a rule for transporting
captured values. The local form deliberately uses splice-site name resolution
until those rules exist.

## `tsx!` syntax island

Explore a compiler-provided `tsx! { ... }` expression for typed prompt trees:

```oche
let value = tsx! {
  <conversation>
    <system>You are a coding agent.</system>
    <user>{request}</user>
    <tool name="search" schema={type SearchArgs} />
  </conversation>
}
```

This would be a bounded parser extension rather than a user macro system. The
lexer would recognize tags only inside `tsx! { ... }`, so comparison and generic
syntax elsewhere would remain unchanged. Parsing would produce structured AST,
never generated source or evaluated strings.

The intended lowering is ordinary typed Oche calls:

- tags resolve to statically known functions;
- attributes bind to named function parameters and lower to positional calls;
- a conventional `children` parameter receives nested nodes;
- text and fragments use statically resolved builder functions;
- `{expression}` embeds a normal Oche expression;
- the final Dart backend sees only normal calls, lists, strings, and values.

A later form may name a builder namespace, such as `tsx! prompt { ... }`, so
prompt, HTML, schema, and workflow libraries can provide different constructors
without adding domain semantics to the compiler.

Do not implement spread attributes, runtime tag names, hooks, a virtual DOM,
token macros, arbitrary grammar plugins, or React compatibility as part of the
initial experiment. Before implementation, validate that ordinary function and
list construction is materially obstructing real prompt code.
