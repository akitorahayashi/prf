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

- Target selection: `--type`, `--all`
- Scope selection: `--current` or explicit path arguments
- Deletion confirmation control: `-y/--yes`
- Verbose reporting: `-v/--verbose`

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
