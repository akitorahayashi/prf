# Architecture

## Canonical Model

- Category: Cleanup domain unit (`xcode`, `python`, `rust`, `nodejs`, `brew`, `docker`).
- Scope Authority: A canonical local root, an exact per-user allowlist entry, or an external-tool
  boundary.
- Scan Candidate: A typed filesystem path or external action with category attribution and
  measured size.
- Scan Report: Unique candidates plus `ready`, `clean`, `unavailable`, or `failed` category status.
- Run Plan: A normalized user-selected subset of one complete scan report.

## Ownership Boundaries

| Boundary | Path | Responsibility |
|---|---|---|
| Binary entry | `src/main.rs` | Delegates process entry to library CLI runner |
| CLI adapter | `src/cli/` | Clap parsing, argument normalization, and app option conversion |
| Application orchestration | `src/app/` | Scan and run use-case flow orchestration |
| Target ownership | `src/targets/` | Authoritative category catalog, typed candidate model, discovery, and Docker behavior |
| Filesystem boundary | `src/fs/` | Validated roots, allocated-size measurement, candidate revalidation, and non-following deletion |
| Output boundary | `src/output/` | Byte formatting, progress styles, reporting, and interactive prompts |
| Error kernel | `src/error.rs` | Typed application error model |

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
├── targets/
│   ├── mod.rs
│   ├── catalog.rs
│   ├── category.rs
│   ├── item.rs
│   ├── report.rs
│   ├── target.rs
│   ├── traversal.rs
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
├── safety.rs
└── runtime.rs
```

## Execution Model

- `scan` performs provider discovery before optional allocated-size measurement.
- Provider discovery is parallel while report and plan ordering remains deterministic.
- `run` applies scan, selection, plan normalization, display, confirmation, revalidation, execution,
  and summary phases.
- Duplicate and nested paths are normalized after category selection.
- Docker cleanup is a typed external action owned by `targets/docker.rs` and never enters filesystem
  deletion.

## Safety Invariants

- Scanning is non-destructive.
- Failed discovery or measurement never authorizes deletion.
- Deletion requires explicit confirmation unless `-y/--yes` is provided.
- Current-directory mode excludes system-wide categories (`brew` and `docker`).
- Deletion touches only displayed plan entries.
- Filesystem kind, identity, and authority are revalidated immediately before removal.
- Symbolic links are never followed during discovery, measurement, or deletion.
