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

The CLI uses command-line arguments as its only runtime configuration surface.

- Category selection: `--type`, `--all`
- Scope selection: `--current` or explicit path arguments
- Deletion confirmation control: `-y/--yes`
- Verbose reporting: `-v/--verbose`

`HOME` resolves per-user paths and `PATH` resolves external tools. No prf-specific environment
variable changes cleanup behavior or enables a hidden diagnostic mode.

## CI/CD Contract

- `.github/workflows/ci-workflows.yml` orchestrates reusable workflows for static checks, tests, coverage, and build.
- Static checks, tests, and coverage execute via `mise exec -- just <recipe>`.
- `release.yml` delegates tagged release builds to `build.yml` using a `release_id` handoff.

## Release

`v*` tag push triggers `.github/workflows/release.yml`, which prepares a release, calls `.github/workflows/build.yml` for matrix builds, uploads artifacts, and publishes the release.
