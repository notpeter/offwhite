# Guidelines

## Structure

`offwhite` is a single-binary Rust crate.
Core behavior lives under `src/`:

- src/args.rs: parses CLI arguments and renders help text,
- src/main.rs: wires startup flow and command dispatch,
- src/action.rs: handles scanning/checking/fixing,
- src/configs.rs: derives policy from `.editorconfig`,
- src/ignores.rs: defines bundled ignore rules.
- src/inits.rs: has Initialization helpers,
- src/violation.rs: violation formatting
- src/tests.rs: Unit tests

## Coding Style & Naming Conventions

Follow Rust 2024 defaults and keep code `rustfmt`-clean.

## Runnables

```sh
cargo run -q -- --help
cargo fmt --check && cargo fmt
cargo check
cargo test -q
```

## Commit & Pull Request Guidelines

Recent commits use short imperative subjects. Less is more; sentece fragments ok.
Keep commit titles concise, action-oriented, and specific.

Include sample command output when changing diagnostics or user-facing help text.

## Contributor Notes

When changing ignore or policy behavior, update both README examples and tests so CLI documentation stays aligned with implementation.
CLI positional paths are parsed as UTF-8 strings; if that changes, update docs and tests together.
