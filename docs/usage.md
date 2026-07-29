# Usage

The scan flow executes via:

```sh
prf scan --all                       # Scan all targets (default set)
prf scan --type python ~/Desktop     # Scan only python targets
prf scan --type rust --verbose .     # Show item-level paths and footprint contributions
prf scan --list ~/Desktop            # Fast target listing without footprint measurement
prf sc --current                     # Alias; scan only current directory
```

The delete flow executes via:

```sh
prf run ~/Desktop                    # Interactive target selection + confirmation
prf run --type nodejs -y ~/Desktop   # Non-interactive deletion for one target
prf run --all -y ~/Desktop           # Delete all targets without prompts
prf rn --current --type rust -y      # Alias; current-directory scoped cleanup
```

Target behavior:

- Default targets: xcode, python, rust, nodejs, brew, docker
- Current-directory mode (`--current`) excludes brew and docker targets
- Positional paths replace `~/Desktop` while retaining applicable home-relative discovery
- `--type` and `--all` skip target selection for `run` but retain deletion confirmation
- `--yes` skips deletion confirmation but does not skip otherwise-required target selection
- Docker cleanup uses `docker system prune -a -f --volumes` and names unused images, containers,
  networks, build cache, and volumes in the deletion plan

Filesystem scan values estimate allocated disk space released by the selected removal roots. Sparse
files use allocated blocks, hard-linked files contribute only when every link is selected, and
symbolic-link candidates and links inside a removal tree are never followed. Docker values remain
estimates reported by Docker. APFS clones, snapshots, concurrent filesystem changes, and failed
removals can make the eventual released space differ from the scan estimate. A cleanup with retained
or failed actions renders its partial outcome and exits unsuccessfully.

Help displays via:

```sh
prf --help
prf scan --help
prf run --help
```
