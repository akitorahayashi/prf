# Configuration

## Files

| File | Purpose |
|------|---------|
| `Cargo.toml` | Rust package metadata and dependencies |
| `clippy.toml` | Clippy linter configuration |
| `rustfmt.toml` | Rust formatter configuration |
| `rust-toolchain.toml` | Rust toolchain version pinning |
| `justfile` | Development task automation (`setup`, `check`, `test`, `coverage`) |
| `mise.toml` | Development tool version management |
| `mise.lock` | Locked tool source URLs and checksums for mise-managed tools |
| `mise.vm.toml` | VM-scoped tooling manifest |
| `mise.vm.lock` | Locked tool source URLs and checksums for VM-scoped tools |

## Runtime Configuration

The CLI currently uses command-line flags as the primary runtime configuration surface.

- Target selection: `--type`, `--all`
- Scope selection: `--current` or explicit path arguments
- Deletion confirmation control: `-y/--yes`
- Verbose reporting: `-v/--verbose`

## CI/CD Contract

- `.github/workflows/ci-workflows.yml` orchestrates reusable workflows for static checks, tests, coverage, and build.
- Static checks, tests, and coverage execute via `mise exec -- just <recipe>`.
- Coverage applies to `prf` sources and excludes reference-only clones under `references/`.
- `release.yml` delegates tagged release builds to `build.yml` using a `release_id` handoff.

## Release

`v*` tag push triggers `.github/workflows/release.yml`, which prepares a release, calls `.github/workflows/build.yml` for matrix builds, uploads artifacts, and publishes the release.
