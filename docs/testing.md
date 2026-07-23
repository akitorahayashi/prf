# Testing

## Structure

Testing is organized by ownership boundary and externally observable behavior:

| Boundary | Location | Purpose |
|---|---|---|
| Owner unit tests | `src/app/`, `src/targets/`, `src/fs/` | Owner-local behavior verification inside `#[cfg(test)]` blocks |
| CLI integration tests | `tests/cli/` via `tests/cli.rs` | Command, output, selection, Docker, help, and alias contracts |
| Safety integration tests | `tests/safety.rs` | Scope, confirmation, and usage-error contracts |
| Runtime integration tests | `tests/runtime.rs` | Compiled binary and unknown-command contracts |

## Principles

- Unit tests validate owner logic at module scope.
- Integration tests validate user-observable CLI behavior and command semantics.
- Tests avoid asserting private implementation details not owned by the boundary under test.

## Execution

All tests execute via:

```bash
just test
```

The CLI integration target executes via:

```bash
cargo test --test cli
cargo test --test safety
cargo test --test runtime
```

Run a specific module test:

```bash
cargo test app::scan::tests
```
