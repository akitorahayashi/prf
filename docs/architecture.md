# Architecture

## Canonical Model

- Target: A registered cleanup definition with an identifier, display name, scope support, and
  discovery contract.
- Candidate: A target-attributed path or process action with a known or explicitly unestimated
  footprint basis.
- Removal Catalog: The owned scanned candidates, canonical physical roots, and their association.
- Scan Report: Target-grouped candidates plus the footprint data required for later subsets.
- Removal Plan: A user-selected, non-overlapping subset shared by footprint aggregation and action
  application.

## Ownership Boundaries

| Boundary | Path | Responsibility |
|---|---|---|
| Binary entry | `src/main.rs`, `src/lib.rs` | Process entry and minimal public execution facade |
| CLI adapter | `src/cli/` | Clap parsing, target resolution, and app option conversion |
| Application orchestration | `src/app/` | Scan and clean use-case sequencing |
| Cleanup domain | `src/cleanup/` | Target contracts, discovery, candidates, removal plans, application, and reports |
| Footprint domain | `src/footprint/` | Allocated-space measurement, reported estimates, and selection-aware aggregation |
| Target definitions | `src/targets/` | Declarative target definitions, the authoritative registry, and target-specific inspection |
| Filesystem boundary | `src/fs/` | Filesystem mutation mechanics |
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
│   ├── scope.rs
│   ├── target.rs
│   ├── scan.rs
│   └── clean.rs
├── app/
│   ├── mod.rs
│   ├── scan.rs
│   └── clean.rs
├── cleanup/
│   ├── mod.rs
│   ├── target.rs
│   ├── scope.rs
│   ├── environment.rs
│   ├── discovery.rs
│   ├── candidate.rs
│   ├── action.rs
│   ├── estimate.rs
│   ├── removal_path.rs
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
│   ├── bun.rs
│   ├── docker.rs
│   ├── mise.rs
│   ├── nodejs.rs
│   ├── pnpm.rs
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
definitions. Clap possible values use the same identifiers. Identifiers are lowercase, unique,
may contain digits and hyphens, and cannot use the interactive `all` keyword.

A standard target consists of one module containing metadata and standard discovery rules, plus one
registry entry. Custom inspectors own dynamic cache resolution and tool protocols for mise, Bun,
pnpm, and Docker.

The CLI represents target input as `Omitted`, `Named`, or `All`. `scan` resolves both `Omitted` and
`All` to every eligible registry target. `clean` maps `Omitted` to an interactive selection over the
scanned report and maps `Named` and `All` to fixed selections. Positional target IDs and `--all` are
mutually exclusive.

## Scope Model

`Scope` is either `Current { root }` or `Default { root, home }`. `InspectionInputs` pairs that
scope with an immutable snapshot of HOME, the working and temporary directories, XDG paths, and
supported tool cache overrides. CLI resolution captures these inputs once before target selection
and inspection. Default scope always uses `~/Desktop` as its recursive root and evaluates
applicable home-relative rules. An unavailable `HOME` makes default scope invalid.

Current scope uses the captured working directory as its only root, does not require `HOME`, disables
home-relative discovery, and rejects default-only targets. The CLI accepts no arbitrary scan-root
argument; another directory is selected by changing the working directory and using `--current`.

## Discovery Model

Standard rules cover recursive directory names, parent-marker constraints, marker-relative child
artifacts, and vetted home-relative paths. A single inspection produces both cleanup candidates and
listing information. `scan --list` skips footprint measurement while using the same definitions.

Discovery diagnostics are explicit inspection results. Command failures and malformed external
output are errors rather than empty successful scans.

mise resolves `MISE_CACHE_DIR`, `XDG_CACHE_HOME`, and the macOS default into one path candidate. Bun
resolves `BUN_INSTALL_CACHE_DIR`, the global `bunfig.toml`, and its default cache path into one path
candidate. Malformed configuration and unsafe dynamic paths are discovery errors rather than
fallbacks.

pnpm queries `pnpm store path --silent`; an unavailable CLI produces a diagnostic, while failed,
empty, relative, or malformed output is a discovery error. An existing store creates one
unestimated process candidate whose owned argument vector fixes the scanned store for
`pnpm store prune`. Docker queries daemon availability and structured disk-usage output; a positive
reclaimable total creates one known-estimate process candidate for
`docker system prune -a -f --volumes`.

## Action Model

`RemovePath` and `RunProcess` form the finite action vocabulary. A path action always uses allocated
storage measurement, while a process action owns either an externally reported estimate or an
explicit unestimated state. Process labels and separated argument vectors are owned values, so a
dynamic path resolved during scanning remains part of the confirmed action. Files, directories,
and terminal symbolic links are distinct removal entry kinds. Action application is exhaustive in
the cleanup domain and delegates low-level filesystem operations to `src/fs/`. Process actions run
without a shell.

Every applied action originates from the selected scan report. Application and output code contain
no target-specific execution branches.

## Removal Planning

The removal catalog owns the scanned candidate set, validates paths against captured protected
roots, canonicalizes candidate parents after discovery, merges physical ancestor aliases, rejects
conflicting entry kinds, and retains the terminal path component. Relative, parent-containing,
overly broad, and protected-root ancestor paths are rejected. A removal plan selects catalog roots
for a report subset and omits roots already covered by a selected ancestor. Plan construction
accepts only candidate indices, so a different candidate collection cannot be paired with catalog
normalization state. Component-aware path sorting followed by one prefix pass selects maximal roots
and their attribution in `O(n log n)` time including sorting.

Footprint measurement and action application consume the same normalized entries. A terminal
symbolic-link candidate contributes and removes only the link entry; its target is never traversed.
Symbolic links found inside a removal directory follow the same non-following rule. A missing path
remains an idempotent plan root with a zero footprint.

Application records removed, already-absent, retained, and failed outcomes without discarding
successful mutations when another action fails. Reclaimed summaries include known bytes and the
count of completed unestimated actions. The outcome report is rendered before retained or failed
actions cause a non-zero command result.
Directory application streams a contents-first walk and removes entries as they are yielded, so
memory grows with traversal depth rather than the full removal tree.

Terminal writes propagate through the application error model. A broken output pipe terminates
successfully at the process boundary; other output failures remain explicit.
Target-selection and deletion-confirmation prompts require terminal stdin. Non-interactive cleanup
uses positional targets or `--all`, plus `-y/--yes` when a non-empty deletion plan needs approval.

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
directly without constructing a retained file tree. The index derives known estimates for target
subsets, confirmation, and successfully applied roots without another filesystem interpretation.
Docker reclaimable bytes use a reported basis and remain outside path and inode aggregation. pnpm
remains unestimated because its safe prune protocol does not expose a non-destructive size.

Allocated-block values remain estimates. APFS clones, snapshots, concurrent changes, and partial or
failed removal can make eventual free-space changes differ from scan output.

## Safety Invariants

- Scanning is non-destructive.
- Deletion requires explicit confirmation unless `-y/--yes` is provided.
- Only candidates surfaced by the approved scan report are applied.
- Known bytes and unestimated actions remain distinct throughout scan, confirmation, and outcome
  reporting.
- Dynamic removal paths cannot contain captured protected roots.
- A terminal symbolic-link candidate is measured and removed as a link entry without following its
  target.
- Removed, already-absent, retained, and failed outcomes remain available after partial mutation.
- Retained and failed confirmed actions produce a non-zero result after outcome rendering.
- Current-directory mode excludes registered targets without current-mode support.
- Global discovery rules are absent from current-mode inspection.
- The default scan root is `~/Desktop`; a missing `HOME` without `--current` produces an error.
- Interactive decisions require terminal stdin; automation explicitly selects targets and supplies
  `-y/--yes`.
- Missing tools, failed processes, malformed command output, and traversal problems are explicit
  errors or diagnostics.
- Footprint overflow and unsupported allocated-storage measurement are explicit errors rather than
  logical-size fallbacks.
