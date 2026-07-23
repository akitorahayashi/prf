# prf Development Notes

## Project Summary
`prf` is a Rust CLI that scans and cleans macOS caches. The binary exposes two primary
subcommands:
- `scan`: dry-run discovery of reclaimable disk space per target.
- `run`: deletion workflow (interactive by default, supports `--type`, `--all`, `-y`).

## Key Modules
- `src/cli/` – Clap command structs and conversion into app options.
- `src/app/scan.rs` / `src/app/run.rs` – Use-case orchestration for scan and deletion flows.
- `src/cleanup/` – Target contracts, discovery, candidates, action application, and reports.
- `src/targets/` – Declarative target definitions, the registry, and target-specific inspection.
- `src/fs/` – Root resolution, size measurement, and deletion mechanics.
- `src/output/` – Byte/path display, report rendering, progress styles, and prompts.

## Coding Guidelines
- Keep output human-friendly: use output-layer format and report modules for user-facing rendering.
- Standard discovery behavior stays in `src/cleanup/discovery.rs`; target modules declare its rules.
- Target defaults, ordering, and CLI resolution come from `src/targets/registry.rs`.
- Target-specific protocols and parsers stay private to their target modules.
- Desktop-focused safety: defaults to ~/Desktop scanning to avoid system areas.
- Prefer small, testable components. Unit tests can live alongside modules, while high-level CLI
  flows belong in `tests/`.
- Avoid deleting files that were not surfaced by the scan report.

## Testing & Tooling
- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-targets --all-features`

Integration tests in `tests/` configure `HOME` and `XDG_CONFIG_HOME` to temporary directories to
keep the host environment untouched.
