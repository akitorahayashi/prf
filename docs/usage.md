# Usage

The scan flow executes via:

```sh
prf scan                              # Scan every default-scope target
prf scan python rust                  # Scan multiple named targets
prf scan rust --current --verbose     # Inspect Rust artifacts in the current directory
prf scan --list                       # List targets without footprint measurement
prf sc --current                      # Alias; scan only the current directory
```

The cleanup flow executes via:

```sh
prf clean                             # Interactive target selection + confirmation
prf clean nodejs -y                   # Clean one target without confirmation
prf clean python rust                 # Clean multiple targets, with confirmation
prf clean --all -y                    # Clean every eligible target without prompts
prf cln rust --current -y             # Alias; current-directory scoped cleanup
```

Target behavior:

- Default targets: xcode, python, rust, nodejs, brew, docker
- Positional target IDs are case-insensitive, deduplicated, and resolved through the registry
- No target selects all eligible targets for `scan` and opens target selection for `clean`
- `--all` explicitly selects all eligible targets and skips `clean` target selection
- Default mode recursively scans `~/Desktop`, evaluates applicable home-relative paths, and includes
  Brew and Docker
- Current-directory mode (`-c/--current`) scans only the working directory, disables home-relative
  discovery, and excludes Brew and Docker
- Arbitrary path arguments are not accepted; another directory is handled by changing to it and
  using `--current`
- `--yes` skips deletion confirmation but does not skip otherwise-required target selection
- Interactive selection and confirmation require terminal stdin; non-interactive cleanup specifies
  targets or `--all` and uses `--yes`
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
prf clean --help
```
