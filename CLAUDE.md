# CLAUDE.md — HortProChecker

## Project Overview

Rust project (edition 2024). See `Cargo.toml` for dependencies and metadata.

## Build & Run Commands

```bash
cargo build              # Debug build
cargo build --release    # Release build
cargo run                # Run the binary
cargo test               # Run all tests
cargo clippy -- -D warnings   # Lint — must pass with zero warnings
cargo fmt -- --check     # Format check — must pass
```

## Quality Gate

Every change **must** pass the following before it is considered done:

```bash
cargo fmt -- --check && cargo clippy -- -D warnings && cargo test
```

Run all three. If any fails, fix it before moving on. No exceptions.

## Rust Conventions

### Error Handling

- Use **snafu** for all error handling.
- Define a **local `Error` enum per module** (not one global error type). Each module owns its failure modes.
- Derive `Debug` and `Snafu` on every error enum. Derive `Display` via snafu's `#[snafu(display(...))]` attribute.
- Use `ensure!`, `context()`, and `.whatever_context()` from snafu — never hand-roll `map_err` chains when snafu selectors exist.
- Re-export the module's `Result` type alias: `pub type Result<T, E = Error> = std::result::Result<T, E>;`

```rust
use snafu::{ResultExt, Snafu, ensure};

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to read config from {path}"))]
    ReadConfig { source: std::io::Error, path: String },

    #[snafu(display("invalid port number: {value}"))]
    InvalidPort { value: u16 },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
```

### Forbidden Patterns

| Pattern | Rule |
|---|---|
| `.unwrap()` | **Banned.** Propagate errors or use a snafu context instead. |
| `.expect(...)` | **Banned.** Same as above — no panic-on-error in any code path. |
| `panic!(...)` | **Banned** outside of tests. In `#[cfg(test)]` blocks, panics are fine. |
| `#[allow(clippy::...)]` | **Banned.** Fix the lint, don't silence it. |
| `#[allow(dead_code)]` | **Banned.** Remove unused code instead of suppressing warnings. |
| `todo!()` / `unimplemented!()` | **Banned** in committed code. Use compile errors or snafu variants for not-yet-handled cases. |
| `unsafe { ... }` | Avoid unless absolutely necessary. Justify with a `// SAFETY:` comment if used. |

### Code Style

- Run `cargo fmt` before every commit. Do not fight rustfmt — configure `rustfmt.toml` if needed, but prefer defaults.
- All public items get doc comments (`///`). Internal items get comments only when the logic is non-obvious.
- Prefer `impl Into<T>` / `impl AsRef<T>` in function signatures for ergonomic APIs.
- Prefer iterators and combinators over manual loops where readability is not harmed.
- Keep functions short. If a function exceeds ~40 lines, consider splitting it.
- Use `#[must_use]` on functions that return values the caller should not silently ignore.

### Project Structure

- One module = one file (or directory with `mod.rs`). Keep modules focused.
- Tests live in `#[cfg(test)] mod tests { ... }` inside the module they test, or in `tests/` for integration tests.
- Binary entry point (`main.rs`) should be thin — delegate to a lib or module immediately.

### Dependencies

- Add dependencies with `cargo add <crate>`. Keep `Cargo.toml` tidy.
- Prefer well-maintained crates from the Rust ecosystem. Check download counts and maintenance status.
- Pin major versions; let Cargo.lock handle exact pinning.

### Testing

- Write tests for all non-trivial logic.
- Use `#[test]` functions in the module's `tests` submodule for unit tests.
- Use `assert!`, `assert_eq!`, `assert_ne!` — panics are fine inside `#[test]`.
- For error-path tests, match on the error variant explicitly rather than using `.is_err()` alone.
- Integration tests go in `tests/` at the project root.

### Git & Workflow

- Commit messages: imperative mood, concise summary line, optional body for context.
- Do not commit code that fails the quality gate.
- Prefer small, focused commits over large omnibus ones.
