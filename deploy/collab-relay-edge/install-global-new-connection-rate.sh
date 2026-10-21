#!/bin/sh
set -eu
LC_ALL=C
export LC_ALL

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
verify_script="$script_dir/verify-global-new-connection-rate.sh"
rules_verifier="$script_dir/verify-rate-rules.py"
table_name=openpencil_relay_edge_rate
meter_name=relay_edge_new_v4
rule_comment='OpenPencil relay edge per-source new connections'

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
    command -v "$command_name" >/dev/null 2>&1 || {
        echo "required command is unavailable: $command_name" >&2
        exit 1
    }
done
"$rules_verifier" --address "$public_ipv4" \
    --table "$table_name" --meter "$meter_name" --comment "$rule_comment"

umask 077
staged=$(mktemp /run/openpencil-relay-edge-rate.XXXXXX)
cleanup() {
    rm -f "$staged"
}
trap cleanup EXIT HUP INT TERM
chmod 0600 "$staged"

if nft list table inet "$table_name" >/dev/null 2>&1; then
    printf '%s\n' "delete table inet $table_name" >>"$staged"
fi
printf '%s\n' \
    "table inet $table_name {" \
    '    chain prerouting {' \
    '        type filter hook prerouting priority mangle; policy accept;' \
    "        ip daddr $public_ipv4 tcp dport 443 tcp flags & (fin | syn | rst | ack) == syn ct state new meter $meter_name { ip saddr timeout 2m limit rate over 60/minute burst 20 packets } counter drop comment \"$rule_comment\"" \
    '    }' \
    '}' >>"$staged"

nft --check --file "$staged"
nft --file "$staged"
"$verify_script" "$public_ipv4"
