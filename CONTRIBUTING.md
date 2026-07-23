# Contributing

## Contribution Policies

### Coding Standards

- Formatter: `rustfmt` (configuration in rustfmt.toml).
- Linter: `clippy` with `-D warnings` (configuration in clippy.toml).
- Minimum Supported Rust Version: 1.90.0 (configuration in clippy.toml and rust-toolchain.toml).
- Edition: 2024.

### Naming Conventions

- Types: `PascalCase`
- Functions and variables: `snake_case`
- Modules: `snake_case`

### Testing Strategies

- Unit tests are defined in owner modules under `#[cfg(test)]`.
- Integration tests are placed under tests/.

## Local Verification

### Environment Setup

```bash
just setup
```

### Verify Commands

```bash
just fix
just check
just test
```

### Coverage Command

```bash
just coverage
```
