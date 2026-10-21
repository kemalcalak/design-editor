#!/bin/sh
set -eu
LC_ALL=C
export LC_ALL

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
rules_verifier="$script_dir/verify-rate-rules.py"
table_name=openpencil_relay_edge_rate
meter_name=relay_edge_new_v4
rule_comment='OpenPencil relay edge per-source new connections'

if [ "$#" -ne 1 ]; then
    echo "usage: $0 DEDICATED_PUBLIC_IPV4" >&2
    exit 2
fi
public_ipv4=$1
for command_name in nft python3; do
    command -v "$command_name" >/dev/null 2>&1 || {
        echo "required command is unavailable: $command_name" >&2
        exit 1
    }
done
"$rules_verifier" --address "$public_ipv4" \
    --table "$table_name" --meter "$meter_name" --comment "$rule_comment"
nft --json --numeric list table inet "$table_name" |
    "$rules_verifier" --address "$public_ipv4" --rules-json \
        --table "$table_name" --meter "$meter_name" --comment "$rule_comment"

echo "active relay-edge IPv4 new-connection limiter verified"
