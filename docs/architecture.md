# Architecture

## Canonical Model

- Category: Cleanup domain unit (`xcode`, `python`, `rust`, `nodejs`, `brew`, `docker`).
- Scan Item: A concrete file or directory candidate with measured size.
- Scan Report: Category-grouped aggregation of reclaimable targets.
- Run Plan: User-selected subset of scan results approved for deletion.

## Ownership Boundaries

| Boundary | Path | Responsibility |
|---|---|---|
| Binary entry | `src/main.rs` | Delegates process entry to library CLI runner |
| CLI adapter | `src/cli/` | Clap parsing, argument normalization, and app option conversion |
| Application orchestration | `src/app/` | Scan and run use-case flow orchestration |
| Target ownership | `src/targets/` | Category model, target registry, and target-specific discovery/cleanup rules |
| Filesystem boundary | `src/fs/` | Root resolution, size calculation, and deletion mechanics |
| Output boundary | `src/output/` | Byte formatting, progress styles, reporting, and interactive prompts |
| Scan model | `src/report.rs` | Category-grouped scan aggregation shared by `app` and `output`, owned by neither |
| Error kernel | `src/error.rs` | Typed application error model |

## Package Structure

```text
src/
├── main.rs
├── lib.rs
├── error.rs
├── report.rs
├── cli/
│   ├── mod.rs
│   ├── scan.rs
│   └── run.rs
├── app/
│   ├── mod.rs
│   ├── scan.rs
│   └── run.rs
├── targets/
│   ├── mod.rs
│   ├── catalog.rs
│   ├── category.rs
│   ├── item.rs
│   ├── target.rs
│   ├── name_matcher.rs
│   ├── python.rs
│   ├── nodejs.rs
│   ├── rust.rs
│   ├── xcode.rs
│   ├── brew.rs
│   └── docker.rs
├── fs/
│   ├── mod.rs
│   ├── roots.rs
│   ├── size.rs
│   └── remove.rs
└── output/
    ├── mod.rs
    ├── bytes.rs
    ├── progress.rs
    ├── report.rs
    └── prompt.rs

tests/
├── cli.rs
├── cli/
│   ├── mod.rs
│   ├── aliases.rs
│   ├── help_and_version.rs
│   ├── run.rs
│   └── scan.rs
├── harness/
│   ├── mod.rs
│   └── test_context.rs
├── runtime.rs
└── safety.rs
```

## Execution Model

- `scan` performs target discovery first and size calculation second, with parallel execution for throughput.
- `run` always starts from a scan report, then applies selection, confirmation, and deletion phases.
- Docker cleanup is owned by `targets/docker.rs` and remains separate from filesystem deletion.

## Safety Invariants

- Scanning is non-destructive.
- Deletion requires explicit confirmation unless `-y/--yes` is provided.
- Current-directory mode excludes system-wide categories (`brew` and `docker`).
  `Category::supports_current_mode` is the single policy source for this exclusion: `catalog::resolve`
  enforces it as the user-facing gate that rejects unsupported categories, and `catalog::build_targets`
  consults the same predicate when constructing targets. Orchestration does not re-derive the policy.
- The default scan root is `~/Desktop`; when `HOME` is unset and neither an explicit path nor
  `--current` is given, root resolution fails explicitly rather than silently falling back to the
  current directory.