# Usage

The scan flow executes via:

```sh
prf scan --all                       # Explicitly require all categories
prf scan --type python ~/Desktop     # Scan only python targets
prf scan --type rust --verbose .     # Show item-level paths and sizes
prf scan --list ~/Desktop            # Fast target listing without size calculation
prf sc --current                     # Alias; scan only current directory
```

The delete flow executes via:

```sh
prf run ~/Desktop                    # Interactive category selection + confirmation
prf run --type nodejs -y ~/Desktop   # Non-interactive deletion for one category
prf run --all -y ~/Desktop           # Delete all categories without prompts
prf rn --current --type rust -y      # Alias; current-directory scoped cleanup
```

Category behavior:

- Default categories: xcode, python, rust, nodejs, brew, docker
- Current-directory mode (`--current`) excludes brew and docker categories
- Repeated `--type` values are deduplicated in catalog order
- An implicitly selected unavailable Docker provider is reported and does not fail other categories
- Explicit `--type docker` and `--all` fail when Docker is unavailable
- Docker cleanup runs only when a nonzero typed action appeared in the confirmed scan plan

Root behavior:

- Positional paths are existing readable directories and collapse when they overlap
- No positional path resolves to `$HOME/Desktop`
- `--current` resolves to the current directory and conflicts with positional paths
- Failed home, current-directory, or explicit-root resolution has no substitute path

Run behavior:

- `run` without `--type` or `--all` prompts for categories with ready candidates
- `--yes` skips only final confirmation and never expands category or path scope
- Empty selection, rejected confirmation, and clean state are successful no-ops
- Complete cleanup, skipped missing candidates, and failures are reported separately

Exit behavior:

- Exit code `0` represents complete operations and user-declined no-ops
- Exit code `1` represents operational, incomplete-scan, and partial-cleanup failures
- Exit code `2` represents command usage errors

Help displays via:

```sh
prf --help
prf scan --help
prf run --help
```
