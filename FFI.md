# Oche native FFI

Oche uses `dart:ffi` to call Rust and other native libraries. Oche does not
implement a separate FFI runtime or invent another ABI model: it exposes a
smaller, statically checked surface over Dart's native types, dynamic-library
loading, symbol lookup, pointers, native calls, and finalizers. This is an
initial 0.1 design contract; concrete binding syntax will be accepted only with
the first executable vertical slice.

## Boundary model

A convenient binding may present two views of one `dart:ffi` declaration:

1. The native signature maps directly to Dart FFI representations such as
   `Int32`, `Uint64`, `Double`, and `Pointer<Void>`.
2. The public signature describes the Oche values callers use, such as `Int`,
   `Float`, `String`, `Result<T, E>`, or an opaque handle type.

Generated Dart glue performs only declared conversions. It never guesses
whether a pointer is borrowed, owned, nullable, terminated by zero, or paired
with a length. This keeps binding behavior reviewable and lets the compiler
diagnose unsupported combinations before invoking Dart.

## Rust-facing C API

Rust libraries should expose a deliberately small C surface:

```rust
#[unsafe(no_mangle)]
pub extern "C" fn engine_create() -> *mut Engine;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn engine_run(
    engine: *mut Engine,
    input_ptr: *const u8,
    input_len: usize,
    output: *mut OcheBuffer,
) -> i32;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn engine_buffer_free(buffer: OcheBuffer);

#[unsafe(no_mangle)]
pub unsafe extern "C" fn engine_destroy(engine: *mut Engine);
```

The generated C header is the binding source of truth. A status code plus an
explicit output parameter is preferable to unwinding or passing Rust-owned
layout types over the boundary. Rust panics must be caught inside the native
library and converted to a documented failure status; they must not cross C.

## Memory and concurrency

- Input bytes are borrowed only for the duration of a synchronous call unless
  the binding says the native library copies them.
- Every owned pointer returned from native code names its release function.
- Opaque handles cannot be constructed from integers in ordinary Oche code.
- A wrapper may use a native finalizer as a leak backstop, but deterministic
  `close` or scoped use remains available for scarce resources.
- A native call runs on its current isolate. Potentially blocking work should
  be wrapped explicitly with `isolate.run` or exposed through an asynchronous
  native protocol.
- Native resources are not assumed to be transferable between isolates.

## Tooling and targets

The compiler will lower native modules mechanically to `dart:ffi`; Dart remains
the source of truth for supported native types and calling conventions. Header
ingestion should reuse Dart `ffigen`, and Rust projects can produce headers with `cbindgen`.
Oche's definition artifact remains small and diffable so it can be checked into
a project and indexed by the LSP without loading the native library.

Native bindings must behave the same under `oche run` and `oche build-exe`.
Library packaging and lookup must not depend on the current working directory.
The JavaScript backend reports a source error when reachable code imports a
native module.

The first acceptance library should be a Rust `cdylib` that exports scalar
arithmetic, a borrowed byte operation, an opaque counter handle, an owned UTF-8
result, and matching release functions. Tests should validate successful calls,
error conversion, repeated allocation and release, JIT/AOT parity, missing
symbols, and JavaScript rejection.
