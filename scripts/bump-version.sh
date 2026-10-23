#!/bin/sh

# The product version lives only in [workspace.package].version of the root
# Cargo.toml; every crate inherits it and every other artifact (SDK bundles,
# Chrome zip, VSIX, macOS plist, Windows installer) reads it at build time.
#
#   scripts/bump-version.sh X.Y.Z   write X.Y.Z into Cargo.toml, then refresh
#   scripts/bump-version.sh         Cargo.toml was edited by hand; refresh only
#
# "Refresh" rewrites the workspace entries of Cargo.lock and runs the version
# guard. Commit Cargo.toml and Cargo.lock together.

set -eu

usage() {
    printf 'usage: scripts/bump-version.sh [X.Y.Z]\n' >&2
}

if [ "$#" -gt 1 ]; then
    usage
    exit 2
fi

script_dir=$(CDPATH= cd "$(dirname "$0")" && pwd)
repo_root=$(CDPATH= cd "$script_dir/.." && pwd)
manifest="$repo_root/Cargo.toml"

if [ "$#" -eq 1 ]; then
    new_version=$1
    candidate="$manifest.bump-version.$$"
    trap 'rm -f "$candidate"' 0 HUP INT TERM
    # Rewrite only the version line inside [workspace.package]; dependency
    # versions elsewhere in the manifest are left alone.
    awk -v version="$new_version" '
        /^[[:space:]]*\[/ {
            in_workspace_package = ($0 ~ /^[[:space:]]*\[[[:space:]]*workspace[[:space:]]*[.][[:space:]]*package[[:space:]]*\]/)
        }
        in_workspace_package && /^[[:space:]]*version[[:space:]]*=/ {
            sub(/"[^"]*"/, "\"" version "\"")
        }
        { print }
    ' "$manifest" > "$candidate"
    if ! written=$("$repo_root/scripts/workspace-version.sh" "$candidate") ||
            [ "$written" != "$new_version" ]; then
        printf 'bump-version: %s is not a valid SemVer version; Cargo.toml left unchanged\n' \
            "$new_version" >&2
        exit 1
    fi
    cat "$candidate" > "$manifest"
fi

if ! canonical_version=$("$repo_root/scripts/workspace-version.sh"); then
    printf 'bump-version: canonical version read failed; set a valid SemVer at [workspace.package].version in Cargo.toml\n' >&2
    exit 1
fi

printf 'bump-version: canonical Cargo workspace version is %s\n' "$canonical_version"

if ! (cd "$repo_root" && \
        cargo update --workspace --offline && \
        cargo metadata --locked --no-deps --format-version 1 >/dev/null); then
    printf 'bump-version: Cargo lock refresh failed\n' >&2
    exit 1
fi

if ! (cd "$repo_root" && "$repo_root/tools/check-version-sync.sh"); then
    printf 'bump-version: version consistency check failed\n' >&2
    exit 1
fi

printf 'bump-version: product version is %s; commit Cargo.toml and Cargo.lock\n' "$canonical_version"
