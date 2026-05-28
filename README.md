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

The release binary will be available at `target/release/prf`.

### Verification

```bash
prf --version
prf scan --list
```

### Common Commands

```bash
prf scan --all                  # Scan every category
prf sc --current                # Alias for scan in current-directory mode
prf run                         # Scan, select categories, and confirm deletion
prf rn --current --type rust -y # Alias for run with explicit deletion
prf scan --type python -v       # Show detailed Python cleanup targets
```

### Categories

| Category  | Description |
|-----------|-------------|
| `xcode`   | Project-local Xcode/Swift caches and, outside `--current`, vetted global Xcode and SwiftPM caches. |
| `python`  | Python caches such as `__pycache__`, `.pytest_cache`, `.ruff_cache`, `.mypy_cache`, `.venv`, and `.uv-cache`. |
| `rust`    | Rust build artifacts in `target` directories. |
| `nodejs`  | NodeJS artifacts including `node_modules`, `.next`, `.nuxt`, and `.svelte-kit`. |
| `brew`    | Homebrew caches and build artifacts. Skipped in `--current` mode. |
| `docker`  | Docker cache and unused data. Skipped in `--current` mode. |
| `mise`    | Mise cache and temporary files. Skipped in `--current` mode. |

### Safety Model

1. Scans report reclaimable size per category.
2. `--type <category>`, `--all`, and interactive selection constrain deletion scope.
3. Destructive actions require confirmation unless `-y/--yes` is supplied.

## Architecture

The implementation follows explicit boundaries:

- `src/cli/` parses CLI arguments and converts them into app options.
- `src/app/` orchestrates scan and run use cases.
- `src/targets/` owns cleanup target discovery and Docker cleanup behavior.
- `src/fs/` owns root resolution, size measurement, and filesystem deletion.
- `src/output/` owns terminal rendering, progress styles, and prompts.

## Documentation

- [Docs](docs/README.md): Usage, architecture, configuration, and testing references.
- [Contributing](CONTRIBUTING.md): Development guidelines and verification commands.
