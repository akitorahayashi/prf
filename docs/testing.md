# Testing

## Structure

Testing is organized by ownership boundary and externally observable behavior:

| Boundary | Location | Purpose |
|---|---|---|
| Owner unit tests | `src/app/**/*.rs`, `src/targets/**/*.rs`, `src/fs/**/*.rs` | Owner-local behavior verification inside `#[cfg(test)]` blocks |
| Integration tests | `tests/cli.rs` (with `tests/cli/`), `tests/runtime.rs`, `tests/safety.rs` | CLI contract verification through compiled binary execution |

## Principles

- Unit tests validate owner logic at module scope.
- Integration tests validate user-observable CLI behavior and command semantics.
- Tests avoid asserting private implementation details not owned by the boundary under test.

## Execution

Run all tests:

```bash
just test
```

Run by integration test target:

```bash
cargo test --test cli
cargo test --test runtime
cargo test --test safety
```

Run a specific module test:

```bash
cargo test app::scan::tests
```