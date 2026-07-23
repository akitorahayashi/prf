# Architecture

## Canonical Model

- Target: A registered cleanup definition with an identifier, display name, scope support, and
  discovery contract.
- Candidate: A concrete, scanned cleanup action with an allocated or externally reported footprint
  basis.
- Removal Catalog: The owned scanned candidates, canonical physical roots, and their association.
- Scan Report: Target-grouped candidates plus the footprint data required for later subsets.
- Removal Plan: A user-selected, non-overlapping subset shared by footprint aggregation and action
  application.

## Ownership Boundaries

| Boundary | Path | Responsibility |
|---|---|---|
| Binary entry | `src/main.rs` | Process entry delegation to the library CLI runner |
| CLI adapter | `src/cli/` | Clap parsing, target resolution, and app option conversion |
| Application orchestration | `src/app/` | Scan and run use-case sequencing |
| Cleanup domain | `src/cleanup/` | Target contracts, discovery, candidates, removal plans, application, and reports |
| Footprint domain | `src/footprint/` | Allocated-space measurement, reported estimates, and selection-aware aggregation |
| Target definitions | `src/targets/` | Declarative target definitions, the authoritative registry, and target-specific inspection |
| Filesystem boundary | `src/fs/` | Root resolution and filesystem mutation mechanics |
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
│   ├── plan.rs
│   ├── apply.rs
│   └── report.rs
├── footprint/
│   ├── mod.rs
│   ├── amount.rs
│   ├── allocation.rs
│   └── error.rs
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
listing information. `scan --list` skips footprint measurement while using the same definitions.

Discovery diagnostics are explicit inspection results. Command failures and malformed external
output are errors rather than empty successful scans.

## Action Model

`RemovePath` and `RunProcess` form the finite action vocabulary. Each candidate also declares whether
its footprint is allocated storage or an externally reported estimate. Action application is
exhaustive in the cleanup domain and delegates low-level filesystem operations to `src/fs/`.
Process actions use an executable and separated argument vector without a shell.

Every applied action originates from the selected scan report. Application and output code contain
no target-specific execution branches.

## Removal Planning

The removal catalog owns the scanned candidate set, canonicalizes existing paths after discovery,
merges physical aliases, rejects conflicting entry kinds, and retains original paths for output. A
removal plan selects catalog roots for a report subset and omits roots already covered by a selected
ancestor. Plan construction accepts only candidate indices, so a different candidate collection
cannot be paired with catalog normalization state.

Footprint measurement and action application consume the same physical roots. A symbolic-link
candidate therefore resolves once to the target used by both stages, while symbolic links found
inside a removal directory are measured and removed without following them. A missing path remains
an idempotent plan root with a zero footprint.

## Footprint Model

Unix filesystem estimates derive from allocated blocks in 512-byte units. Regular files,
directories, and symbolic-link entries contribute their own allocation. Sparse files therefore
contribute allocated storage rather than logical length.

The allocation index records ordinary per-root totals and observations only for regular files with
multiple links. A hard-linked inode contributes once when every link reported by filesystem metadata
belongs to the selected removal roots; a link outside the selection makes that inode contribute
zero. Duplicate roots and selected descendants do not inflate aggregate totals.

Each rendered report attributes a selected root's contribution to one deterministic source candidate,
so candidate and target contributions sum to the report total. The interactive target menu queries
the same index for each target as a standalone selection rather than reusing context-dependent
contributions from the complete scan.

One bounded Rayon pool traverses maximal roots and nested directories. Ordinary entries contribute
directly without constructing a retained file tree. The index derives estimates for target subsets,
confirmation, and successfully applied roots without another filesystem interpretation. Docker
reclaimable bytes use a reported basis and remain outside path and inode aggregation.

Allocated-block values remain estimates. APFS clones, snapshots, concurrent changes, and partial or
failed removal can make eventual free-space changes differ from scan output.

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
- Footprint overflow and unsupported allocated-storage measurement are explicit errors rather than
  logical-size fallbacks.
