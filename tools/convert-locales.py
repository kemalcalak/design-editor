#!/usr/bin/env python3
"""Retired locale converter.

The TypeScript locale source under ``apps/web`` was removed with the retired
TypeScript application. Rust files in ``crates/de-i18n/src/i18n`` are now the
canonical, hand-maintained translation catalog.

This command intentionally fails instead of silently skipping missing inputs
or overwriting the canonical Rust tables. Catalog integrity is checked by the
``de-i18n`` test suite.
"""

import sys


def main() -> int:
    print(
        "tools/convert-locales.py is retired: "
        "edit crates/de-i18n/src/i18n/*.rs directly and run "
        "`cargo test -p de-i18n`.",
        file=sys.stderr,
    )
    return 2


if __name__ == "__main__":
    sys.exit(main())
