#!/usr/bin/env python3
"""Fail-closed verifier for a dedicated OpenPencil edge nftables table."""

from __future__ import annotations

import argparse
import ipaddress
import json
import sys
from typing import Any


def public_ipv4(value: str) -> str:
    try:
        address = ipaddress.ip_address(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("expected a canonical public IPv4") from error
    if (
        address.version != 4
        or str(address) != value
        or not address.is_global
        or address.is_multicast
    ):
        raise argparse.ArgumentTypeError("expected a canonical public IPv4")
    return value


def only(items: list[dict[str, Any]], kind: str) -> dict[str, Any]:
    matches = [item[kind] for item in items if set(item) == {kind}]
    if len(matches) != 1:
        raise ValueError(f"expected exactly one {kind}")
    return matches[0]


def expect_equal(actual: Any, expected: Any, label: str) -> None:
    if actual != expected:
        raise ValueError(f"unexpected {label}")


def bitmask_terms(value: Any) -> set[int]:
    """Normalize nft JSON's version-dependent bitwise-OR representations."""
    if isinstance(value, int):
        return {value}
    if isinstance(value, list) and all(isinstance(item, int) for item in value):
        return set(value)
    if isinstance(value, dict) and set(value) == {"|"}:
        operands = value["|"]
        if isinstance(operands, list) and len(operands) == 2:
            return bitmask_terms(operands[0]) | bitmask_terms(operands[1])
    raise ValueError("unexpected TCP flags mask")


def verify_rules(
    document: Any,
    expected_address: str,
    table_name: str,
    meter_name: str,
    comment: str,
) -> None:
    if not isinstance(document, dict) or set(document) != {"nftables"}:
        raise ValueError("unexpected nftables JSON envelope")
    items = document["nftables"]
    if not isinstance(items, list):
        raise ValueError("nftables JSON entries must be a list")
    semantic = [item for item in items if "metainfo" not in item]
    if len(semantic) != 3:
        raise ValueError("dedicated table must contain only one table, chain, and rule")

    table = only(semantic, "table")
    expect_equal(table.get("family"), "inet", "table family")
    expect_equal(table.get("name"), table_name, "table name")

    chain = only(semantic, "chain")
    for key, expected in {
        "family": "inet",
        "table": table_name,
        "name": "prerouting",
        "type": "filter",
        "hook": "prerouting",
        "prio": -150,
        "policy": "accept",
    }.items():
        expect_equal(chain.get(key), expected, f"chain {key}")

    rule = only(semantic, "rule")
    for key, expected in {
        "family": "inet",
        "table": table_name,
        "chain": "prerouting",
        "comment": comment,
    }.items():
        expect_equal(rule.get(key), expected, f"rule {key}")
    expressions = rule.get("expr")
    if not isinstance(expressions, list) or len(expressions) != 7:
        raise ValueError("expected one exact seven-expression drop rule")

    expect_equal(
        expressions[0],
        {
            "match": {
                "op": "==",
                "left": {"payload": {"protocol": "ip", "field": "daddr"}},
                "right": expected_address,
            }
        },
        "IPv4 destination match",
    )
    expect_equal(
        expressions[1],
        {
            "match": {
                "op": "==",
                "left": {"payload": {"protocol": "tcp", "field": "dport"}},
                "right": 443,
            }
        },
        "TCP destination-port match",
    )
    flags_match = expressions[2].get("match")
    if not isinstance(flags_match, dict):
        raise ValueError("expected initial SYN match")
    expect_equal(flags_match.get("op"), "==", "TCP flags operator")
    expect_equal(flags_match.get("right"), 2, "TCP flags value")
    flags_and = flags_match.get("left")
    if not isinstance(flags_and, dict) or set(flags_and) != {"&"}:
        raise ValueError("expected TCP flags mask")
    flags_operands = flags_and["&"]
    if not isinstance(flags_operands, list) or len(flags_operands) != 2:
        raise ValueError("expected two TCP flags-mask operands")
    expect_equal(
        flags_operands[0],
        {"payload": {"protocol": "tcp", "field": "flags"}},
        "TCP flags payload",
    )
    expect_equal(
        bitmask_terms(flags_operands[1]),
        {1, 2, 4, 16},
        "TCP flags mask",
    )
    expect_equal(
        expressions[3],
        {
            "match": {
                "op": "in",
                "left": {"ct": {"key": "state"}},
                "right": 8,
            }
        },
        "connection state match",
    )

    meter = expressions[4].get("meter") if isinstance(expressions[4], dict) else None
    if not isinstance(meter, dict):
        raise ValueError("expected per-source meter")
    expect_equal(meter.get("name"), meter_name, "meter name")
    expect_equal(
        meter.get("key"),
        {
            "elem": {
                "val": {"payload": {"protocol": "ip", "field": "saddr"}},
                "timeout": 120,
            }
        },
        "meter key and timeout",
    )
    expect_equal(
        meter.get("stmt"),
        {
            "limit": {
                "rate": 60,
                "burst": 20,
                "per": "minute",
                "inv": True,
            }
        },
        "meter rate",
    )
    if set(meter) - {"key", "stmt", "size", "name"}:
        raise ValueError("unexpected meter behavior")

    counter = expressions[5].get("counter") if isinstance(expressions[5], dict) else None
    if not isinstance(counter, dict) or set(counter) - {"packets", "bytes"}:
        raise ValueError("unexpected counter expression")
    expect_equal(expressions[6], {"drop": None}, "terminal drop")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--address", required=True, type=public_ipv4)
    parser.add_argument(
        "--table",
        default="openpencil_locator_edge_rate",
    )
    parser.add_argument(
        "--meter",
        default="locator_edge_new_v4",
    )
    parser.add_argument(
        "--comment",
        default="OpenPencil locator edge per-source new connections",
    )
    parser.add_argument("--rules-json", action="store_true")
    arguments = parser.parse_args()
    for label, value in {
        "table": arguments.table,
        "meter": arguments.meter,
    }.items():
        if not value.replace("_", "").isalnum() or len(value) > 64:
            parser.error(f"{label} must be a bounded identifier")
    if not arguments.comment or len(arguments.comment) > 128:
        parser.error("comment must be 1..=128 characters")
    if arguments.rules_json:
        verify_rules(
            json.load(sys.stdin),
            arguments.address,
            arguments.table,
            arguments.meter,
            arguments.comment,
        )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (json.JSONDecodeError, ValueError) as error:
        print(f"invalid edge rate table: {error}", file=sys.stderr)
        raise SystemExit(1) from error
