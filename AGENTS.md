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
  cli/             Clap commands, target selection, root resolution, and app option conversion
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
    apply.rs       Removal-plan execution and successful-action summaries
  footprint/
    amount.rs      Estimated byte amounts and allocated/reported bases
    allocation.rs  Parallel allocation measurement and selection-aware aggregation
    error.rs       Footprint-specific failure taxonomy
  targets/
    registry.rs    Authoritative ordered target catalog and CLI resolution
    *.rs           One definition per target; Docker owns its CLI protocol and parser
  fs/              Root resolution and filesystem removal
  output/          Byte/path display, reports, messages, progress styles, and prompts
tests/
  cli.rs           CLI integration-test entry point with cases under tests/cli/
  runtime.rs       Binary startup and dispatch contracts
  safety.rs        Confirmation and current-mode safety contracts
  harness/         Isolated HOME, working directory, and executable PATH
docs/              Architecture, usage, configuration, and testing references
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
Docker installation cannot enter a test accidentally. Tests that mutate process-global state use
serial execution where required.

The local task surface is:

- `just fix` - applies Rust and justfile formatting.
- `just check` - verifies formatting, Clippy with warnings denied, and justfile formatting.
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

### Target Registry

`src/targets/registry.rs` is the sole ordered catalog. Default selection, `--all`, case-insensitive
`--type` resolution, deduplication, display order, and current-mode eligibility derive from the
registered `Target` values. Registry validation enforces non-empty display names, unique IDs, and
lowercase identifier syntax.

Standard targets declare `Rule` values for directory names, manifest-relative children, or vetted
home-relative paths. Discovery walks roots to a maximum depth of 10, deduplicates paths per target,
and stops descending once a matching removable directory is found. A target uses a private
`Inspector` only when standard rules cannot express its external protocol.

### Action and Removal Model

`Action` is the complete application vocabulary:

- `RemovePath` carries a path and its expected file or directory kind.
- `RunProcess` carries a static executable, separated argument vector, and label.

A `RemovalCatalog` canonicalizes existing paths and merges aliases after discovery. A selected
`RemovalPlan` omits descendants of another selected root while retaining candidate and target
ownership. Parent paths are canonicalized without following a candidate's terminal symbolic link,
so measurement, confirmation, and removal refer to the same entry. Within a traversed directory,
files and symbolic links are removed before directories, deepest directories are attempted first,
vanished paths are idempotent, and directories that become non-empty remain in place and are
reported as retained.

Path removals run in parallel before process actions. Application is not transactional: an error
can follow other successful mutations, and no rollback occurs. Removed, already-absent, retained,
and failed outcomes remain available for the final report. Retained or failed actions produce a
non-zero command result after the partial report is rendered. Process actions run without a shell
and surface startup or non-zero-exit failures.

### Footprint Model

Filesystem estimates use Unix allocated blocks rather than logical file length. Measurement includes
regular files, directory entries, and non-followed symbolic links. Regular files with multiple hard
links contribute once only when every reported link belongs to the selected removal roots.

The allocation walker shares one bounded Rayon pool across maximal candidate roots and nested
directories. It retains per-root totals and multi-link inode observations rather than a display tree.
An `Index` derives candidate contributions and aggregate estimates for arbitrary report subsets.
Process actions own their externally reported estimates, which remain outside path and inode
normalization. Path actions always use allocated measurement. Missing paths contribute zero; other
metadata and traversal failures are explicit. APFS clones, snapshots, and concurrent filesystem
changes keep all displayed values estimates.

### Docker Inspection

Docker is the only custom inspector. An unavailable CLI or daemon produces a diagnostic and no
candidate. A usable daemon is queried with `docker system df --format "{{json .}}"`; malformed
JSON, missing reclaimable fields, invalid sizes, and command failures are discovery errors. A
positive reclaimable total creates one process candidate for
`docker system prune -a -f --volumes`; a zero total creates no action.

### CLI

`src/cli/mod.rs` owns the two subcommands and their aliases: `scan`/`sc` and `run`/`rn`. Both accept
repeatable `--type`, `--all` as its mutually exclusive alternative, `--current`, optional paths, and
verbose output. `scan --list` uses target inspection without measuring sizes. `run` is
target-selection interactive only when neither `--type` nor `--all` is present; deletion
confirmation remains required unless `-y/--yes` is supplied.

### Key Types

- `Target` - registered metadata plus scope support and a `Discovery` contract.
- `Scope` - either one current root or default roots with an optional captured home.
- `Inspection` - candidates, list results, and diagnostics from one target.
- `Candidate` - a target-attributed path or process action; the variant owns its footprint inputs.
- `RemovalCatalog` - the owned scanned candidates, canonical physical roots, and their association.
- `RemovalPlan` - a selected, non-overlapping subset shared by estimation and application.
- `Estimate` - a checked byte amount produced from allocated or externally reported data.
- `Index` - selection-aware allocation totals and multi-link inode observations.
- `ScanReport` - candidates and footprint data grouped by `TargetId`, and the authority for run
  subsets.
- `ApplyReport` - per-action outcomes and the estimate reclaimed by completed actions.
- `AppError` - the typed CLI-wide error model used across discovery, cleanup, I/O, and selection.

## Safety Invariants

- Scanning and `scan --list` perform no cleanup actions.
- Every applied action originates in the confirmed subset of the immediately preceding scan report.
- A terminal symbolic-link candidate removes only the link entry and never follows its target.
- Destructive execution requires confirmation unless `-y/--yes` is present.
- Current mode cannot select default-only targets or evaluate global home-relative rules.
- Missing roots and unavailable optional tools are visible diagnostics; failed commands, malformed
  structured output, footprint overflow, and unexpected filesystem errors are explicit failures.
- A path that disappears between discovery, measurement, and application is treated idempotently.

## Documentation Responsibilities

- `AGENTS.md` - source map, cross-cutting invariants, key contracts, and documentation pointers.
- `README.md` - installation, quick start, target overview, and safety summary.
- `docs/architecture.md` - ownership boundaries, canonical model, discovery and action mechanics,
  and safety invariants.
- `docs/usage.md` - command examples, flags, aliases, and target behavior.
- `docs/configuration.md` - repository tooling, runtime configuration surface, CI, and release flow.
- `docs/testing.md` - test layers, ownership principles, and focused test commands.
- `CONTRIBUTING.md` - coding standards, environment setup, and local verification tasks.
