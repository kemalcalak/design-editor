#!/usr/bin/env bash

# Workflow fixtures intentionally preserve unexpanded shell variables.
# shellcheck disable=SC2016

set -euo pipefail

script_dir=$(CDPATH='' cd "$(dirname "$0")" && pwd)
repo_root=$(CDPATH='' cd "$script_dir/.." && pwd)
bump_source="$script_dir/bump-version.sh"
reader_source="$script_dir/workspace-version.sh"
tmp_base=${TMPDIR:-/tmp}
tmp_dir=$(mktemp -d "${tmp_base%/}/bump-version.XXXXXX")
trap 'rm -rf "$tmp_dir"' 0 HUP INT TERM

test_count=0
case_status=0
case_output=

fail() {
    printf 'not ok %s - %s\n' "$test_count" "$1" >&2
    exit 1
}

pass() {
    test_count=$((test_count + 1))
    printf 'ok %s - %s\n' "$test_count" "$1"
}

assert_status() {
    expected=$1
    label=$2
    if [ "$case_status" -ne "$expected" ]; then
        printf '%s\n' "$case_output" >&2
        fail "$label: expected status $expected, got $case_status"
    fi
}

assert_contains() {
    needle=$1
    label=$2
    case "$case_output" in
        *"$needle"*) ;;
        *)
            printf '%s\n' "$case_output" >&2
            fail "$label: missing output: $needle"
            ;;
    esac
}

assert_file_contains() {
    file=$1
    needle=$2
    label=$3
    if ! grep -Fq -- "$needle" "$file"; then
        fail "$label: $file does not contain: $needle"
    fi
}

assert_file_not_matches() {
    file=$1
    pattern=$2
    label=$3
    if grep -Eq -- "$pattern" "$file"; then
        fail "$label: $file unexpectedly matches: $pattern"
    fi
}

cargo() {
    printf 'cargo:%s:%s\n' "$PWD" "$*" >> "$BUMP_TEST_LOG"
    if [ -e "$PWD/.fail-cargo" ]; then
        printf 'fake cargo failure\n' >&2
        return 31
    fi
    version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$PWD/Cargo.toml")
    printf 'lock-version=%s\n' "$version" > "$PWD/Cargo.lock"
    printf '%s\n' '{"packages":[]}'
}

export -f cargo

new_repo() {
    name=$1
    repo="$tmp_dir/$name"
    mkdir -p "$repo/scripts" "$repo/tools" "$repo/outside"
    ln -s "$bump_source" "$repo/scripts/bump-version.sh"
    ln -s "$reader_source" "$repo/scripts/workspace-version.sh"
    printf '%s\n' \
        '[workspace]' \
        'members = []' \
        '' \
        '[workspace.package]' \
        'version = "2.3.4"' \
        '' \
        '[workspace.dependencies]' \
        'serde = { version = "1" }' > "$repo/Cargo.toml"
    printf '%s\n' 'lock-version=1.0.0' > "$repo/Cargo.lock"

    cat > "$repo/tools/check-version-sync.sh" <<'SCRIPT'
#!/bin/sh
set -eu
repo_root=$(CDPATH= cd "$(dirname "$0")/.." && pwd)
printf 'guard:%s\n' "$PWD" >> "$BUMP_TEST_LOG"
if [ -e "$repo_root/.fail-guard" ]; then
    printf 'fake guard failure\n' >&2
    exit 33
fi
SCRIPT
    chmod +x "$repo/tools/check-version-sync.sh"
    printf '%s\n' "$repo"
}

run_bump() {
    repo=$1
    shift
    case_status=0
    case_output=$(cd "$repo/outside" && env BUMP_TEST_LOG="$repo/calls.log" \
        bash "$repo/scripts/bump-version.sh" "$@" 2>&1) || case_status=$?
}

case_status=0
case_output=$("$bump_source" 1.2.3 4.5.6 2>&1) || case_status=$?
assert_status 2 'argument rejection'
assert_contains 'usage: scripts/bump-version.sh [X.Y.Z]' 'argument rejection'
pass 'bump entrypoint rejects more than one version with actionable usage'

repo=$(new_repo explicit_version)
run_bump "$repo" 3.0.0
assert_status 0 'explicit version'
assert_file_contains "$repo/Cargo.toml" 'version = "3.0.0"' 'explicit version'
assert_file_contains "$repo/Cargo.toml" 'serde = { version = "1" }' 'explicit version'
assert_file_contains "$repo/Cargo.lock" 'lock-version=3.0.0' 'explicit version'
assert_file_contains "$repo/calls.log" "guard:$repo" 'explicit version'
pass 'an explicit version rewrites only [workspace.package].version and refreshes the lock'

repo=$(new_repo invalid_version)
cp "$repo/Cargo.toml" "$repo/Cargo.toml.before"
run_bump "$repo" 3.0
assert_status 1 'invalid version'
assert_contains 'bump-version: 3.0 is not a valid SemVer version; Cargo.toml left unchanged' \
    'invalid version'
if ! cmp -s "$repo/Cargo.toml" "$repo/Cargo.toml.before"; then
    fail 'invalid version modified Cargo.toml'
fi
if [[ -e "$repo/calls.log" ]]; then
    fail 'invalid version invoked Cargo or the version guard'
fi
pass 'invalid versions leave Cargo.toml untouched and stop before Cargo'

repo=$(new_repo reader_failure)
printf '%s\n' \
    '[workspace]' \
    'members = []' \
    '' \
    '[workspace.package]' \
    'version = "not-semver"' > "$repo/Cargo.toml"
run_bump "$repo"
assert_status 1 'canonical reader failure'
assert_contains 'bump-version: canonical version read failed; set a valid SemVer at [workspace.package].version in Cargo.toml' \
    'canonical reader failure'
if [[ -e "$repo/calls.log" ]]; then
    printf '%s\n' "$(cat "$repo/calls.log")" >&2
    fail 'canonical reader failure invoked Cargo or the version guard'
fi
pass 'canonical reader failures stop before Cargo and the version guard'

repo=$(new_repo external_cwd)
run_bump "$repo"
assert_status 0 'external cwd'
assert_file_contains "$repo/calls.log" "cargo:$repo:update --workspace --offline" \
    'external cwd'
assert_file_contains "$repo/calls.log" "cargo:$repo:metadata --locked --no-deps --format-version 1" \
    'external cwd'
assert_file_contains "$repo/calls.log" "guard:$repo" 'external cwd'
assert_file_contains "$repo/Cargo.lock" 'lock-version=2.3.4' 'external cwd'
pass 'bump entrypoint locates the repository and runs every stage from an external cwd'

first_hash=$(shasum "$repo/Cargo.toml" "$repo/Cargo.lock")
run_bump "$repo"
assert_status 0 'idempotent rerun'
second_hash=$(shasum "$repo/Cargo.toml" "$repo/Cargo.lock")
if [ "$first_hash" != "$second_hash" ]; then
    fail 'idempotent rerun changed managed file hashes'
fi
pass 'refreshing twice leaves Cargo.toml and Cargo.lock unchanged'

repo=$(new_repo cargo_failure)
touch "$repo/.fail-cargo"
run_bump "$repo"
assert_status 1 'Cargo stage failure'
assert_contains 'bump-version: Cargo lock refresh failed' 'Cargo stage failure'
pass 'Cargo refresh failures identify the failing stage'

repo=$(new_repo guard_failure)
touch "$repo/.fail-guard"
run_bump "$repo"
assert_status 1 'guard stage failure'
assert_contains 'bump-version: version consistency check failed' 'guard stage failure'
pass 'guard failures identify the final verification stage'

hook="$repo_root/.githooks/pre-commit"
assert_file_contains "$hook" 'tools/check-version-sync.sh' 'pre-commit guard'
assert_file_not_matches "$hook" 'rev-parse --abbrev-ref|grep -oE.*[0-9]|git add|jq --arg|\.tmp.*mv' \
    'pre-commit detect-only policy'
guard_line=$(grep -n 'tools/check-version-sync.sh' "$hook" | head -n 1 | cut -d: -f1)
fmt_line=$(grep -n 'cargo fmt' "$hook" | head -n 1 | cut -d: -f1)
if [ -z "$guard_line" ] || [ -z "$fmt_line" ] || [ "$guard_line" -ge "$fmt_line" ]; then
    fail 'pre-commit guard must run before Rust format and lint checks'
fi
pass 'pre-commit is detect-only and runs the version guard before Rust checks'

workflow="$repo_root/.github/workflows/version-sync.yml"
assert_file_contains "$workflow" 'pull_request:' 'version sync workflow trigger'
assert_file_contains "$workflow" 'push:' 'version sync workflow trigger'
assert_file_contains "$workflow" 'submodules: recursive' 'version sync checkout'
assert_file_contains "$workflow" \
    'actions/checkout@08eba0b27e820071cde6df949e0beb9ba4906955' \
    'version sync immutable checkout'
assert_file_contains "$workflow" \
    'dtolnay/rust-toolchain@4360b52568e2003a75bf9bc1d59f33a8e3fc893c' \
    'version sync immutable Rust setup'
assert_file_contains "$workflow" "toolchain: '1.94'" 'version sync exact Rust toolchain'
assert_file_contains "$workflow" 'tools/pinned-release-tools.test.sh' \
    'version sync pinned tool contract tests'
assert_file_contains "$workflow" \
    'tools/pinned-release-tools.sh ripgrep "$RUNNER_TEMP/ripgrep-15.2.0"' \
    'version sync digest-pinned ripgrep setup'
assert_file_contains "$workflow" 'run: rg --version' 'version sync pinned ripgrep gate'
assert_file_not_matches "$workflow" \
    'uses:[[:space:]]+[^@[:space:]]+@(v[0-9]+|stable|main|master|latest)|setup-bun|apt-get' \
    'version sync immutable tools policy'
assert_file_not_matches "$workflow" 'bun ' 'version sync needs no SDK toolchain'
assert_file_contains "$workflow" 'packages/scripts/workspace-version.mjs' 'version sync SDK reader path'
assert_file_not_matches "$workflow" 'packages/\*\*' 'version sync focused path filters'
for trigger in pull_request push; do
    trigger_block=$(awk -v trigger="$trigger" '
        $0 == "  " trigger ":" { in_trigger = 1; next }
        in_trigger && ($0 ~ /^  [a-z_]+:$/ || $0 ~ /^[^ ]/) { exit }
        in_trigger { print }
    ' "$workflow")
    for required_path in \
        'crates/**/*.rs' \
        'tools/pinned-release-tools.sh' \
        'tools/pinned-release-tools.test.sh'; do
        path_filter_count=$(grep -Fc -- "      - \"$required_path\"" \
            <<< "$trigger_block" || true)
        if [[ "$path_filter_count" -ne 1 ]]; then
            fail "version sync workflow $trigger paths must include $required_path exactly once (found $path_filter_count)"
        fi
    done
done
assert_file_contains "$workflow" 'scripts/workspace-version.test.sh' 'version reader tests'
assert_file_contains "$workflow" 'scripts/bump-version.test.sh' 'version bump tests'
assert_file_contains "$workflow" 'tools/check-version-sync.test.sh' 'version guard tests'
assert_file_contains "$workflow" 'tools/check-version-sync.sh' 'version guard'
pass 'dedicated CI installs prerequisites and runs the focused version checks'

old_guard_name='check-version-'"fixtures"
old_sync_name='sync-'"version"
if rg -n -e "$old_guard_name" -e "$old_sync_name" "$repo_root" --hidden \
    --glob '!.git/**' --glob '!vendor/**' > "$tmp_dir/old-paths"; then
    sed 's/^/  /' "$tmp_dir/old-paths" >&2
    fail 'old version guard paths remain in the repository'
fi
pass 'repository no longer references the retired guard or sync entrypoint names'

printf '1..%s\n' "$test_count"
