#!/bin/sh
set -eu
LC_ALL=C
export LC_ALL

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
verify_script="$script_dir/verify-global-new-connection-rate.sh"
rules_verifier="$script_dir/../collab-relay-edge/verify-rate-rules.py"

if [ "$#" -ne 1 ]; then
    echo "usage: $0 DEDICATED_PUBLIC_IPV4" >&2
    exit 2
fi
public_ipv4=$1
if [ "$(id -u)" -ne 0 ]; then
    echo "installing host nftables rules requires root" >&2
    exit 1
fi
for command_name in nft mktemp chmod python3; do
    if ! command -v "$command_name" >/dev/null 2>&1; then
        echo "required command is unavailable: $command_name" >&2
        exit 1
    fi
done
"$rules_verifier" --address "$public_ipv4"

# Generate the only accepted ruleset from a strictly parsed IPv4 argument.
# Never ask root to reopen or execute a caller-controlled nftables file.
umask 077
staged=$(mktemp /run/openpencil-locator-edge-rate.XXXXXX)
cleanup() {
    rm -f "$staged"
}
trap cleanup EXIT HUP INT TERM
chmod 0600 "$staged"

if nft list table inet openpencil_locator_edge_rate >/dev/null 2>&1; then
    printf '%s\n' \
        'delete table inet openpencil_locator_edge_rate' >>"$staged"
fi
printf '%s\n' \
    'table inet openpencil_locator_edge_rate {' \
    '    chain prerouting {' \
    '        type filter hook prerouting priority mangle; policy accept;' \
    "        ip daddr $public_ipv4 tcp dport 443 tcp flags & (fin | syn | rst | ack) == syn ct state new meter locator_edge_new_v4 { ip saddr timeout 2m limit rate over 60/minute burst 20 packets } counter drop comment \"OpenPencil locator edge per-source new connections\"" \
    '    }' \
    '}' >>"$staged"

# Check and apply the same root-owned immutable bytes. nft applies a file as
# one transaction, including replacement of our dedicated table.
nft --check --file "$staged"
nft --file "$staged"
"$verify_script" "$public_ipv4"
