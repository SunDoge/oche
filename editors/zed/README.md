# Oche for Zed

This development extension provides `.oche` syntax highlighting and connects
Zed to the existing Oche language server for diagnostics, hover information,
and formatting.

Build the CLI and expose it on the project `PATH`:

```sh
cargo build -p oche
PATH="$PWD/target/debug:$PATH" zed .
```

In that Zed process, open `zed: extensions`, choose **Install Dev Extension**,
and select this `editors/zed` directory. Reopen an `.oche` file after
installation.

The initial extension pins `tree-sitter-rust` because the Oche 0.1 syntax is
deliberately Rust-shaped. Oche-specific highlight queries cover its additional
keywords. A separately versioned `tree-sitter-oche` grammar is required before
publishing the extension, once the language surface diverges enough to justify
maintaining it.
