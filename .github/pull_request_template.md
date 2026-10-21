## Summary

<!-- What changes for the user or the codebase, and why. 1-3 sentences. -->

Closes #

## Changes

<!-- One bullet per file group: what changed and why. -->

-

## Type

<!-- Must match the commit type. Check one. -->

- [ ] `feat` — new feature
- [ ] `fix` — bug fix
- [ ] `refactor` — no behaviour change
- [ ] `perf` — performance
- [ ] `style` — formatting only
- [ ] `docs` — documentation or design frames
- [ ] `test` — tests only
- [ ] `chore` — build, CI, tooling, renames

## Verification

<!-- Paste real output or "n/a: <reason>". Unchecked boxes need a reason. -->

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test` for the touched crates (native host crates with `--features gl-host`)
- [ ] wasm check if `de-editor-ui` or `de-host-web` changed: `cargo check --target wasm32-unknown-unknown -p de-host-web --no-default-features --features web`
- [ ] `bash tools/check-widget-boundary.sh` and no `.rs` file over 800 lines
- [ ] Visible change: screenshots in both themes, or n/a

## Review

- [ ] Reviewed per axis: correctness, performance, consistency, readability, spec
- [ ] Commits follow Conventional Commits with a repo scope, one commit per phase
- [ ] No unrelated changes
