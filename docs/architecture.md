# Architecture

This is the repository layout and how the pieces fit together. The language
contract itself lives in [`../LANGUAGE.md`](../LANGUAGE.md).

```
source → syntax AST → module linking → type inference and checked HIR
       → compiler-owned standard derivations and native bindings → Dart
       → Kernel → Dart VM JIT (primary runtime)
                ↘ Dart AOT
                ↘ JavaScript → ESM host
```

## Crates

- `crates/dart-vm`: bindgen-generated Dart C ABI plus a hand-written safe wrapper
  for an already-entered Dart JIT isolate. `DART_SDK_VERSION` pins the header
  version used for the checked-in raw bindings; scoped handles, VM errors,
  Kernel loading, primitive invocation, and byte-buffer finalizers remain in the
  reviewed wrapper.
- `crates/frontend`: lexer, AST, parser, postfix generic types, function and
  variable resolution, structs, and early semantic checks. The repository still
  carries an experimental comptime parser path outside the 0.1 contract.
- `crates/hir`: typed expressions, module-qualified callable and data identities,
  local generic variant inference, and compiler-owned synthetic declarations.
- `crates/formatter`: comment-preserving, width-aware formatting built on
  Wadler-style document combinators.
- `crates/analysis`: stable file identities, source revisions, shared compiler
  snapshots, spans, and staged diagnostics for the CLI and LSP.
- `crates/comptime`: an experimental bootstrap retained while JSON and other
  standard derivations move behind compiler-owned APIs. It is not a public 0.1
  language feature.
- `crates/codegen`: deterministic typed-HIR lowering to Dart. Resolved callable
  and field identities are preserved through emission; indexed Dart identifiers
  cannot collide with Dart keywords.
- `crates/interop`: narrow Dart-to-Oche definition generation for typed,
  namespaced library calls.
- `crates/lsp`: a dependency-light LSP server with diagnostics, hover help,
  document symbols, folding, go-to-definition (including through imports and
  Dart definition modules), references, in-file highlights, formatting, and
  relative linking of Dart definitions and transitive Oche modules across open
  workspace documents and disk.
- `crates/cli`: inspection, Kernel/AOT build, JIT execution, and LSP commands.
- `agent`: a completed validation slice in which a TypeScript host drives an
  Oche state machine that models pi-agent-core's outer scheduling loop as
  explicit effects.
- `editors/vscode`: TextMate syntax highlighting and a VS Code LSP client.
- `editors/zed`: a Zed development extension (see
  [`../editors/zed/README.md`](../editors/zed/README.md)).
- `tests`: real VM/Rust FFI integration probes, including an Oche agent Kernel
  driven through model and queue transitions in one live JIT isolate.

Errors use SNAFU across the frontend, CLI, and VM binding. Frontend diagnostics
are typed, expression type errors retain byte spans, and the CLI adds
file/line/column context, while I/O, process, UTF-8, compiler, and VM failures
retain their source chains.

## Versioning

The frontend and VM will be versioned together because Kernel is an internal
format. The first backend deliberately uses generated Dart as a narrow bridge to
the SDK's Kernel compiler. It already provides an executable
Oche → syntax AST → typed HIR → Dart → Kernel → Dart VM path. Runtime function
bodies are emitted directly from typed HIR. A direct Kernel serializer can
replace that bridge after the language semantics settle, without changing the
parser or CLI contract.

## Design references

[Lisette](https://github.com/ivov/lisette) is a useful reference because it is
also a Rust frontend targeting an existing language ecosystem. The review used
commit `8155fcef4402de86e0722b367fdcd8894c631aae` (MIT). Oche follows its broad
separation of syntax, semantic passes, emission, diagnostics, and CLI pipeline.
No Lisette source has been copied or added as a dependency: its Go-specific AST,
ABI planner, and backend are too coupled to Lisette to reuse directly. Parser
recovery, accumulated diagnostics, stable diagnostic codes, and snapshot-style
codegen tests are good candidates to adopt as Oche grows.
