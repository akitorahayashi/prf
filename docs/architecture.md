# Architecture

## Canonical Model

- Target: A registered cleanup definition with an identifier, display name, scope support, and
  discovery contract.
- Candidate: A target-attributed path or process action whose variant determines its footprint
  inputs.
- Removal Catalog: The owned scanned candidates, canonical physical roots, and their association.
- Scan Report: Target-grouped candidates plus the footprint data required for later subsets.
- Removal Plan: A user-selected, non-overlapping subset shared by footprint aggregation and action
  application.

## Ownership Boundaries

| Boundary | Path | Responsibility |
|---|---|---|
| Binary entry | `src/main.rs`, `src/lib.rs` | Process entry and minimal public execution facade |
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

`RemovePath` and `RunProcess` form the finite action vocabulary. A path action always uses allocated
storage measurement, while a process action owns its externally reported estimate. Files,
directories, and terminal symbolic links are distinct removal entry kinds. Action application is
exhaustive in the cleanup domain and delegates low-level filesystem operations to `src/fs/`.
Process actions use an executable and separated argument vector without a shell.

Every applied action originates from the selected scan report. Application and output code contain
no target-specific execution branches.

## Removal Planning

The removal catalog owns the scanned candidate set, canonicalizes candidate parents after discovery,
merges physical ancestor aliases, rejects conflicting entry kinds, and retains the terminal path
component. A removal plan selects catalog roots for a report subset and omits roots already covered
by a selected ancestor. Plan construction accepts only candidate indices, so a different candidate
collection cannot be paired with catalog normalization state. Component-aware path sorting followed
by one prefix pass selects maximal roots and their attribution in `O(n log n)` time including
sorting.

Footprint measurement and action application consume the same normalized entries. A terminal
symbolic-link candidate contributes and removes only the link entry; its target is never traversed.
Symbolic links found inside a removal directory follow the same non-following rule. A missing path
remains an idempotent plan root with a zero footprint.

Application records removed, already-absent, retained, and failed outcomes without discarding
successful mutations when another action fails. Reclaimed estimates include only completed actions.
The outcome report is rendered before retained or failed actions cause a non-zero command result.
Directory application streams a contents-first walk and removes entries as they are yielded, so
memory grows with traversal depth rather than the full removal tree.

## Footprint Model

Unix filesystem estimates derive from allocated blocks in 512-byte units. Regular files,
directories, and symbolic-link entries contribute their own allocation. Sparse files therefore
contribute allocated storage rather than logical length.

The allocation index records ordinary per-root totals and observations only for regular files with
multiple links. A hard-linked inode contributes once when every link reported by filesystem metadata
belongs to the selected removal roots; a link outside the selection makes that inode contribute
zero. Duplicate roots and selected descendants do not inflate aggregate totals.

Each rendered report attributes a selected root's contribution to one deterministic source
candidate, so candidate and target contributions sum to the report total. Standalone target
estimates are calculated once during report construction and cached for the interactive target
menu rather than reusing context-dependent contributions from the complete scan.

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
