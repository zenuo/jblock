# jblock

## Cursor Cloud specific instructions

This repository is currently a bare Rust/Cargo skeleton: it contains only
`README.md` and a Cargo-oriented `.gitignore`. There is no `Cargo.toml`,
`src/`, or application code yet, so there is nothing to build, run, or test
until Rust sources are added.

- The Rust toolchain (`cargo`/`rustc`) is preinstalled in the environment
  (`/usr/local/cargo/bin`); no manual install is needed.
- Once a `Cargo.toml` exists, the standard Cargo workflow applies:
  `cargo build`, `cargo run`, `cargo test`, `cargo fmt`, `cargo clippy`.
- The startup update script runs `cargo fetch` only when a `Cargo.toml` is
  present, so it is a safe no-op in the current empty state.
