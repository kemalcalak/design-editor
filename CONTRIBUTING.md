# Contributing

Thanks for your interest in Design Editor.

## Before you start

- Look for an existing [issue](https://github.com/kemalcalak/design-editor/issues) or open one with the bug or feature form. Every issue carries a type, a priority and a size label.
- Building and the crate map: [Build from source](./docs/guide/building.md). Conventions: [`CLAUDE.md`](./CLAUDE.md) and [`crates/CLAUDE.md`](./crates/CLAUDE.md).

## Pull requests

- One topic per PR, branched from `main` and named `<issue>-<slug>`.
- Commits follow [Conventional Commits](https://www.conventionalcommits.org/) with a repo scope, e.g. `fix(canvas): ...`.
- PR titles follow `type(scope): subject (#issue)`; CI checks it. PRs are squash-merged.
- Fill in the PR template: run `cargo fmt`, `cargo clippy -- -D warnings` and the tests of the crates you touched, and add screenshots for visible changes.
- Keep files under 800 lines and add tests with the change.

## Reporting security issues

Please don't open a public issue; see [SECURITY.md](./SECURITY.md).
