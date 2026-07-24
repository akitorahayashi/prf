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
prf scan --all                  # Scan every target
prf sc --current                # Alias for scan in current-directory mode
prf run                         # Scan, select targets, and confirm deletion
prf rn --current --type rust -y # Alias for run with explicit deletion
prf scan --type python -v       # Show detailed Python cleanup targets
```

### Targets

| Target    | Description |
|-----------|-------------|
| `xcode`   | Project-local Xcode/Swift caches and, outside `--current`, vetted global Xcode and SwiftPM caches. |
| `python`  | Python caches such as `__pycache__`, `.pytest_cache`, `.ruff_cache`, `.mypy_cache`, and `.venv`. |
| `rust`    | Rust build artifacts in `target` directories. |
| `nodejs`  | Node.js artifacts including `node_modules`, `.next`, `.nuxt`, and `.svelte-kit`. |
| `brew`    | Homebrew caches and build artifacts. Skipped in `--current` mode. |
| `docker`  | Unused Docker images, containers, networks, build cache, and volumes. Skipped in `--current` mode. |

### Safety Model

1. Scans report estimated reclaimable allocated disk space per target.
2. `--type <target>`, `--all`, and interactive selection constrain deletion scope.
3. Destructive actions require confirmation unless `-y/--yes` is supplied.
4. Symbolic-link cleanup removes the link entry without following its target.
5. Partial cleanup reports completed, absent, retained, and failed actions before exiting non-zero.

## Architecture

The implementation follows explicit boundaries:

- `src/cli/` parses CLI arguments and converts them into app options.
- `src/app/` orchestrates scan and run use cases.
- `src/cleanup/` owns discovery contracts, cleanup candidates, action application, and reports.
- `src/footprint/` owns allocated-space measurement and selection-aware estimates.
- `src/targets/` declares supported targets and owns target-specific inspection.
- `src/fs/` owns filesystem deletion.
- `src/output/` owns terminal rendering, progress styles, and prompts.

## Documentation

- [Docs](docs/README.md): Usage, architecture, configuration, and testing references.
- [Contributing](CONTRIBUTING.md): Development guidelines and verification commands.
