# Deferred work

Oche is currently converging on its smallest Dart-shaped language core. Items
in this file are design notes, not commitments for the 0.1 grammar. A deferred
feature should only move into the language after ordinary Oche code exposes a
repeated problem that the feature clearly solves.

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
