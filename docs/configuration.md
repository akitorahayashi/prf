# Configuration

## Files

| File | Purpose |
|------|---------|
| `Cargo.toml` | Rust package metadata and dependencies |
| `clippy.toml` | Clippy linter configuration |
| `rustfmt.toml` | Rust formatter configuration |
| `rust-toolchain.toml` | Rust toolchain version pinning |
| `justfile` | Development task automation (`setup`, `fix`, `check`, `test`, `coverage`, builds) |
| `mise.toml` | Development tool version management |
| `mise.lock` | Locked tool source URLs and checksums for mise-managed tools |
| `mise.vm.toml` | VM-scoped tooling manifest |
| `mise.vm.lock` | Locked tool source URLs and checksums for VM-scoped tools |

## Runtime Configuration

The CLI currently uses command-line flags as the primary runtime configuration surface.

- Target selection: positional `TARGET...` values or `--all`
- Scope selection: default `~/Desktop` mode or `-c/--current`
- Deletion confirmation control: `-y/--yes`
- Verbose reporting: `-v/--verbose`

Environment-derived paths are captured once before target resolution and inspection:

- `MISE_CACHE_DIR`, then `XDG_CACHE_HOME`, controls the mise cache location
- `BUN_INSTALL_CACHE_DIR` controls the Bun package cache location
- `XDG_CONFIG_HOME/.bunfig.toml` or `~/.bunfig.toml` supplies Bun's global
  `install.cache.dir` when the environment override is absent
- pnpm's effective configuration is resolved by `pnpm store path --silent`, and that scanned path
  is bound to the later prune action

Empty or relative XDG base-directory values are treated as unset. Other invalid configured paths
and malformed Bun configuration are discovery failures. Dynamic cache paths cannot contain HOME,
the scan root, the working directory, or the process temporary root, including their canonical
filesystem locations.

## CI/CD Contract

- `.github/workflows/ci-workflows.yml` orchestrates reusable workflows for static checks, tests, coverage, and build.
- Static checks, tests, and coverage execute via `mise exec -- just <recipe>`.
- Static checks include pinned actionlint workflow validation.
- Tests run on Ubuntu and macOS; coverage remains on Ubuntu.
- Third-party actions use immutable full commit SHAs with version comments. Repository-owned
  `akitorahayashi` actions use reviewed release or major tags.
- Coverage applies to `prf` sources with an 80 percent threshold.
- `release.yml` delegates tagged release builds to `build.yml` using a `release_id` handoff.

## Release

`v*` tag push triggers `.github/workflows/release.yml`. The prepare job first runs
`scripts/verify-release-version.sh` and requires the tag to equal `v` plus the Cargo package
version. A valid release then calls `.github/workflows/build.yml` for matrix builds, uploads
artifacts, and publishes the release.
