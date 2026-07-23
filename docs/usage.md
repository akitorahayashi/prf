# Usage

The scan flow executes via:

```sh
prf scan --all                       # Scan all targets (default set)
prf scan --type python ~/Desktop     # Scan only python targets
prf scan --type rust --verbose .     # Show item-level paths and sizes
prf scan --list ~/Desktop            # Fast target listing without size calculation
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
- Docker cleanup runs only when docker is requested and `--current` is not used

Help displays via:

```sh
prf --help
prf scan --help
prf run --help
```
