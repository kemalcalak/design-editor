#!/bin/sh
set -eu
LC_ALL=C
export LC_ALL

if [ "$#" -ne 1 ]; then
    echo "usage: $0 DEDICATED_PUBLIC_IPV4" >&2
    exit 2
fi
public_ipv4=$1
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
rules_verifier="$script_dir/../collab-relay-edge/verify-rate-rules.py"
for command_name in nft python3; do
    command -v "$command_name" >/dev/null 2>&1 || {
        echo "required command is unavailable: $command_name" >&2
        exit 1
    }
done
"$rules_verifier" --address "$public_ipv4"
nft --json --numeric list table inet openpencil_locator_edge_rate |
    "$rules_verifier" --address "$public_ipv4" --rules-json

echo "active locator-edge IPv4 new-connection limiter verified"
