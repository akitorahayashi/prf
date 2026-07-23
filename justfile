set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

default: help

help:
    @echo "Usage: just [recipe]"
    @echo ""
    @echo "Development tasks for prf CLI:"
    @just --list | tail -n +2 | awk '{printf "  \033[36m%-20s\033[0m %s\n", $1, substr($0, index($0, $2))}'

setup:
    @echo "Installing tools with mise..."
    @mise trust
    @mise install --locked

fix:
    cargo fmt
    just --fmt --unstable

check:
    cargo fmt --check
    cargo clippy --all-targets --all-features -- -D warnings
    just --fmt --check --unstable

test:
    cargo test --all-targets --all-features --quiet

coverage:
    rm -rf target/tarpaulin coverage
    mise exec -- cargo tarpaulin \
        --engine llvm \
        --target-dir target/tarpaulin \
        --packages prf \
        --out Stdout \
        --out Html \
        --output-dir coverage \
        --all-features \
        --exclude-files 'references/*' \
        --fail-under 80

build:
    cargo build

build-release target='':
    @if [[ -n "{{ target }}" ]]; then cargo build --release --target {{ target }}; else cargo build --release; fi
