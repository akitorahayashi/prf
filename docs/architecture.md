# Architecture

## Canonical Model

- Target: A registered cleanup definition with an identifier, display name, scope support, and
  discovery contract.
- Candidate: A concrete, scanned cleanup action with an explicit size-estimation state.
- Scan Report: Target-grouped aggregation of candidates approved for later selection.
- Run Plan: A user-selected subset of the scan report approved for action application.

## Ownership Boundaries

| Boundary | Path | Responsibility |
|---|---|---|
| Binary entry | `src/main.rs` | Process entry delegation to the library CLI runner |
| CLI adapter | `src/cli/` | Clap parsing, target resolution, and app option conversion |
| Application orchestration | `src/app/` | Scan and run use-case sequencing |
| Cleanup domain | `src/cleanup/` | Target contracts, discovery, candidates, actions, application, and reports |
| Target definitions | `src/targets/` | Declarative target definitions, the authoritative registry, and target-specific inspection |
| Filesystem boundary | `src/fs/` | Root resolution, size calculation, and filesystem mutation mechanics |
| Output boundary | `src/output/` | Byte formatting, progress styles, reporting, diagnostics, and prompts |
| Error model | `src/error.rs` | Typed application errors |

## Package Structure

```text
src/
├── main.rs
├── lib.rs
├── error.rs
├── cli/
│   ├── mod.rs
│   ├── scan.rs
│   └── run.rs
├── app/
│   ├── mod.rs
│   ├── scan.rs
│   └── run.rs
├── cleanup/
│   ├── mod.rs
│   ├── target.rs
│   ├── scope.rs
│   ├── discovery.rs
│   ├── candidate.rs
│   ├── action.rs
│   ├── apply.rs
│   └── report.rs
├── targets/
│   ├── mod.rs
│   ├── registry.rs
│   ├── brew.rs
│   ├── docker.rs
│   ├── nodejs.rs
│   ├── python.rs
│   ├── rust.rs
│   └── xcode.rs
├── fs/
│   ├── mod.rs
│   ├── roots.rs
│   ├── size.rs
│   └── remove.rs
└── output/
    ├── mod.rs
    ├── bytes.rs
    ├── messages.rs
    ├── progress.rs
    ├── report.rs
    └── prompt.rs
```

## Target Registry

`src/targets/registry.rs` is the authoritative ordered target collection. CLI name resolution,
default selection, presentation order, and current-mode eligibility derive from registered target
definitions.

A standard target consists of one module containing metadata and standard discovery rules, plus one
registry entry. Docker uses the target-specific inspector extension because its discovery protocol
depends on Docker CLI availability and structured command output.

## Discovery Model

Standard rules cover recursive directory names, parent-marker constraints, marker-relative child
artifacts, and vetted home-relative paths. A single inspection produces both cleanup candidates and
listing information. `scan --list` skips size measurement while using the same definitions.

Discovery diagnostics are explicit inspection results. Command failures and malformed external
output are errors rather than empty successful scans.

## Action Model

`RemovePath` and `RunProcess` form the finite action vocabulary. Action application is exhaustive in
the cleanup domain and delegates low-level filesystem operations to `src/fs/`. Process actions use
an executable and separated argument vector without a shell.

Every applied action originates from the selected scan report. Application and output code contain
no target-specific execution branches.

## Safety Invariants

- Scanning is non-destructive.
- Deletion requires explicit confirmation unless `-y/--yes` is provided.
- Only candidates surfaced by the approved scan report are applied.
- Current-directory mode excludes registered targets without current-mode support.
- Global discovery rules are absent from current-mode inspection.
- The default scan root is `~/Desktop`; a missing `HOME` without an explicit path or `--current`
  produces an error.
- Missing tools, failed processes, malformed command output, and traversal problems are explicit
  errors or diagnostics.
