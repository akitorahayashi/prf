# prf

prf is a macOS command-line cleaner for development caches and generated artifacts.

`prf` scans project-local build outputs and vetted tool caches to help reclaim disk
space. Scans are dry-run by default, and deletion requires explicit confirmation unless `-y/--yes`
is supplied.

## Quick Start

### Installation

```bash
cargo install --path .
```

`cargo install --path .` places the `prf` binary on your PATH (under `~/.cargo/bin`). To build the
release binary in-tree instead, run `just build-release`, which produces `target/release/prf`.

### Verification

```bash
prf --version
prf scan --list
```

### Common Commands

```bash
prf scan python rust          # Scan multiple targets under ~/Desktop
prf sc --current             # Alias for a current-directory scan
prf clean                    # Scan, select targets, and confirm deletion
prf clean nodejs -y          # Clean one target without confirmation
prf cln rust --current -y    # Alias for current-directory cleanup
```

### Targets

| Target    | Description |
|-----------|-------------|
| `xcode`   | Project-local Xcode/Swift caches and, outside `--current`, vetted global Xcode and SwiftPM caches. |
| `python`  | Python caches such as `__pycache__`, `.pytest_cache`, `.ruff_cache`, `.mypy_cache`, and `.venv`. |
| `rust`    | Rust build artifacts in `target` directories. |
| `nodejs`  | Node.js artifacts including `node_modules`, `.next`, `.nuxt`, and `.svelte-kit`. |
| `mise`    | The global mise cache. Skipped in `--current` mode. |
| `bun`     | Bun's global package cache, including its optional global virtual store. Skipped in `--current` mode. |
| `pnpm`    | Unreferenced packages removed by `pnpm store prune`. Skipped in `--current` mode. |
| `brew`    | Homebrew caches and build artifacts. Skipped in `--current` mode. |
| `docker`  | Unused Docker images, containers, networks, build cache, and volumes. Skipped in `--current` mode. |

### Safety Model

1. Scans report known reclaimable allocated disk space and mark actions that cannot be estimated
   without mutation.
2. Positional targets, `--all`, and interactive selection constrain deletion scope.
3. Destructive actions require confirmation unless `-y/--yes` is supplied.
4. Symbolic-link cleanup removes the link entry without following its target.
5. Partial cleanup reports completed, absent, retained, and failed actions before exiting non-zero.

The pnpm CLI does not expose a non-destructive store-prune estimate, so pnpm remains explicitly
unestimated until the confirmed `pnpm store prune` action completes. Clearing Bun's cache also
clears its optional global virtual store; affected projects require `bun install` before reuse.

## Architecture

The implementation follows explicit boundaries:

- `src/cli/` parses CLI arguments and converts them into app options.
- `src/app/` orchestrates scan and clean use cases.
- `src/cleanup/` owns discovery contracts, cleanup candidates, action application, and reports.
- `src/footprint/` owns allocated-space measurement and selection-aware estimates.
- `src/targets/` declares supported targets and owns target-specific inspection.
- `src/fs/` owns filesystem deletion.
- `src/output/` owns terminal rendering, progress styles, and prompts.

## Documentation

- [Docs](docs/README.md): Usage, architecture, configuration, and testing references.
- [Contributing](CONTRIBUTING.md): Development guidelines and verification commands.
