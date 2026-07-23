# prf

## Project Overview

`prf` is a Rust 2024 CLI for finding and removing development caches and generated artifacts. It
is macOS-oriented: the catalog includes project-local Python, Rust, Node.js, Xcode, and SwiftPM
artifacts, vetted macOS home-directory caches for Xcode and Homebrew, and Docker system data.
`scan` performs discovery without mutation; `run` scans first and applies only actions represented
by the resulting report. The default scan root is `~/Desktop`, explicit path arguments replace that
root, and `--current` selects the process working directory while suppressing global targets and
home-relative discovery.

## Directory Structure

```text
src/
  main.rs          Process entry point delegating to the library facade
  lib.rs           Minimal public CLI execution facade
  error.rs         Application error taxonomy
  cli/             Clap commands, registry-backed target parsing, scope resolution, and dispatch
  app/
    scan.rs        Parallel inspection, footprint measurement, and scan/list rendering
    run.rs         Scan, interactive selection, confirmation, and action application
  cleanup/
    target.rs      Target identity, scope support, and discovery contract
    scope.rs       Resolved default or current scope and captured home
    discovery.rs   Standard discovery rules, inspections, diagnostics, and listings
    candidate.rs   Target-attributed cleanup actions
    action.rs      Filesystem and process action vocabulary
    plan.rs        Scanned candidates and canonical roots shared by estimation and application
    report.rs      Target-grouped scan reports and selected subsets
    apply.rs       Removal-plan execution and complete per-action outcomes
  footprint/
    amount.rs      Checked estimated byte amounts
    allocation.rs  Parallel allocation measurement and selection-aware aggregation
    error.rs       Footprint-specific failure taxonomy
  targets/
    registry.rs    Authoritative ordered target catalog and CLI resolution
    *.rs           One definition per target; Docker owns its CLI protocol and parser
  fs/              Filesystem removal
  output/          Byte/path display, reports, messages, progress styles, and prompts
tests/
  cli.rs           CLI integration-test entry point with cases under tests/cli/
  harness/         Isolated HOME, working directory, and executable PATH
docs/              Architecture, usage, configuration, and testing references
scripts/           Release tag and Cargo package version validation
```

Target-specific protocols and parsers remain private to their target module. Shared discovery
behavior belongs in `cleanup/discovery.rs`; footprint semantics belong in `footprint/`; filesystem
mutation belongs in `fs/`; terminal-facing formatting and interaction belong in `output/`.

## Testing

Unit tests are colocated under `#[cfg(test)]` in their owning source modules. They cover pure
selection and parsing logic, footprint contracts, and filesystem boundaries through temporary
directories.
Integration tests execute the compiled binary from `tests/`. `TestContext` creates state under
`target/test_tmp`, assigns a temporary `HOME`, and pins `PATH` to a mock-command directory so a host
Docker installation cannot enter a test accidentally. Unit tests receive environment-derived inputs
directly and do not mutate process-global environment or working-directory state.

The local task surface is:

- `just fix` - applies Rust and justfile formatting.
- `just check` - verifies Rust and justfile formatting, Clippy with warnings denied, and workflows
  with pinned actionlint.
- `just test` - runs all targets and features.
- `just coverage` - runs cargo-tarpaulin for `prf` sources with an 80 percent threshold.
- `just build` / `just build-release` - builds debug or release binaries.

## Core Concepts

### Scan-to-Apply Flow

`cli` resolves targets and roots, then `app::scan::scan_targets()` inspects selected targets in
parallel. Each `Inspection` can contain candidates, list-only information, and non-fatal
diagnostics. A `RemovalCatalog` owns the scanned candidates and canonicalizes their paths, and one
footprint index measures its maximal physical roots before candidates enter a `ScanReport`.

`run` always uses this same scan flow. Interactive target selection produces a subset of the report,
confirmation approves that subset, and `apply_plan()` receives the subset's canonical
`RemovalPlan`. No additional action is synthesized after scanning; estimation and application use
the same normalized roots.

### Scope Semantics

No path argument resolves to `~/Desktop`; an unset `HOME` is an error. Explicit paths replace only
the recursive roots and continue without `HOME` while reporting unavailable home discovery once.
Exact duplicate roots are removed while descendant roots remain distinct. In default mode,
applicable `HomePaths` rules are still evaluated in addition to those roots.

`--current` is not an alias for passing `.`. It resolves the current working directory, excludes
targets whose `ScopeSupport` is `DefaultOnly`, and disables all `HomePaths` rules. Brew and Docker
are currently default-only; Xcode, Python, Rust, and Node.js support both modes. `Scope` represents
default and current modes as distinct variants, and environment inputs are captured once by CLI
resolution.

### Contract Authorities

`src/targets/registry.rs` is the sole ordered target catalog. CLI possible values, default and all
selection, display order, case-insensitive resolution, and current-mode eligibility derive from it.
Detailed discovery, planning, footprint, application, Docker, and output mechanics belong in
`docs/architecture.md`; supported command behavior belongs in `docs/usage.md`.

## Safety Invariants

- Scanning and `scan --list` perform no cleanup actions.
- Every applied action originates in the confirmed subset of the immediately preceding scan report.
- A terminal symbolic-link candidate removes only the link entry and never follows its target.
- Destructive execution requires confirmation unless `-y/--yes` is present.
- Current mode cannot select default-only targets or evaluate global home-relative rules.
- Missing roots and unavailable optional tools are visible diagnostics; failed commands, malformed
  structured output, footprint overflow, and unexpected filesystem errors are explicit failures.
- A path that disappears between discovery, measurement, and application is treated idempotently.
- Removed, already-absent, retained, and failed actions remain distinguishable; retained or failed
  actions produce a non-zero result after the partial report.

## Documentation Responsibilities

- `AGENTS.md` - source map, cross-cutting invariants, key contracts, and documentation pointers.
- `README.md` - installation, quick start, target overview, and safety summary.
- `docs/architecture.md` - ownership boundaries, canonical model, discovery and action mechanics,
  and safety invariants.
- `docs/usage.md` - command examples, flags, aliases, and target behavior.
- `docs/configuration.md` - repository tooling, runtime configuration surface, CI, and release flow.
- `docs/testing.md` - test layers, ownership principles, and focused test commands.
- `CONTRIBUTING.md` - coding standards, environment setup, and local verification tasks.
