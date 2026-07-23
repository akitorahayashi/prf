# Testing

## Structure

Testing is organized by ownership boundary and externally observable behavior:

| Boundary | Location | Purpose |
|---|---|---|
| Cleanup contract tests | `src/cleanup/` | Standard discovery, removal planning, reporting, and action-application behavior |
| Footprint contract tests | `src/footprint/` | Allocated blocks, hard links, path-set aggregation, and measurement failures |
| Target protocol tests | `src/targets/` | Registry invariants and target-specific parsing behavior |
| Filesystem tests | `src/fs/` | Mutation mechanics |
| Integration tests | `tests/cli.rs` with cases under `tests/cli/` | CLI, safety, and packaging contracts through compiled binary execution |

## Principles

- Unit tests validate owner logic at module scope.
- Integration tests validate user-observable CLI behavior and command semantics.
- Tests avoid asserting private implementation details not owned by the boundary under test.
- Expected values come from independent filesystem, registry, schema, or protocol authorities.
- Platform preconditions are asserted; tests do not return successfully when a contract was not
  exercised.
- CI runs the full suite on Linux and macOS. Coverage remains an 80 percent Linux gate.

## Execution

Run all tests:

```bash
just test
```

Run the integration test target:

```bash
cargo test --test cli
```

Run a specific module test:

```bash
cargo test footprint::allocation::tests
cargo test cleanup::plan::tests
```
