#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 || -z "$1" ]]; then
    echo "Usage: verify-release-version.sh vX.Y.Z" >&2
    exit 2
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repository_root="$(cd "${script_dir}/.." && pwd)"
package_version="$(
    awk '
        $0 == "[package]" {
            in_package = 1
            next
        }
        /^\[/ {
            in_package = 0
        }
        in_package && /^[[:space:]]*version[[:space:]]*=/ {
            value = $0
            sub(/^[^"]*"/, "", value)
            sub(/".*$/, "", value)
            print value
            exit
        }
    ' "${repository_root}/Cargo.toml"
)"

if [[ -z "${package_version}" ]]; then
    echo "Unable to read the package version from Cargo.toml" >&2
    exit 1
fi

expected_tag="v${package_version}"
if [[ "$1" != "${expected_tag}" ]]; then
    echo "Release tag '$1' does not match package version '${expected_tag}'" >&2
    exit 1
fi

echo "Release tag '${expected_tag}' matches Cargo.toml"
