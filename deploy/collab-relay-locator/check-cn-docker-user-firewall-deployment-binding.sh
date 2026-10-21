#!/bin/sh
set -eu
LC_ALL=C
export LC_ALL

usage() {
    echo "usage: $0 [--syntax-only] INVENTORY LOCATOR_CN_OVERLAY RELAY_CN_OVERLAY" >&2
    exit 2
}

ownership_mode=strict
if [ "${1-}" = --syntax-only ]; then
    ownership_mode=syntax-only
    shift
fi
[ "$#" -eq 3 ] || usage
inventory_file=$1
locator_overlay=$2
relay_overlay=$3

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
. "$script_dir/cn-docker-user-firewall-common.sh"
cn_firewall_load_config "$inventory_file" "$ownership_mode"

extract_overlay_bind() {
    cn_fw_overlay=$1
    cn_fw_expected_port=$2
    if [ ! -f "$cn_fw_overlay" ] || [ -L "$cn_fw_overlay" ] ||
        grep -F '${' "$cn_fw_overlay" >/dev/null
    then
        cn_firewall_fail "CN Compose overlay is missing, symlinked, or mutable"
        return 1
    fi
    cn_fw_host_count=$(grep -Ec '^[[:space:]]+host_ip:[[:space:]]+[0-9.]+$' \
        "$cn_fw_overlay" || true)
    cn_fw_target_count=$(grep -Ec \
        "^[[:space:]]+-[[:space:]]+target:[[:space:]]+$cn_fw_expected_port$" \
        "$cn_fw_overlay" || true)
    cn_fw_published_count=$(grep -Ec \
        "^[[:space:]]+published:[[:space:]]+\"$cn_fw_expected_port\"$" \
        "$cn_fw_overlay" || true)
    cn_fw_protocol_count=$(grep -Ec \
        '^[[:space:]]+protocol:[[:space:]]+tcp$' "$cn_fw_overlay" || true)
    cn_fw_ports_count=$(grep -Ec \
        '^[[:space:]]+ports:[[:space:]]*$' "$cn_fw_overlay" || true)
    cn_fw_list_item_count=$(grep -Ec \
        '^[[:space:]]+-[[:space:]]+' "$cn_fw_overlay" || true)
    cn_fw_all_hosts=$(grep -Ec \
        '^[[:space:]]+host_ip:' "$cn_fw_overlay" || true)
    cn_fw_all_targets=$(grep -Ec \
        '^[[:space:]]+-[[:space:]]+target:' "$cn_fw_overlay" || true)
    cn_fw_all_published=$(grep -Ec \
        '^[[:space:]]+published:' "$cn_fw_overlay" || true)
    cn_fw_all_protocols=$(grep -Ec \
        '^[[:space:]]+protocol:' "$cn_fw_overlay" || true)
    if [ "$cn_fw_host_count" -ne 1 ] || [ "$cn_fw_target_count" -ne 1 ] ||
        [ "$cn_fw_published_count" -ne 1 ] ||
        [ "$cn_fw_protocol_count" -ne 1 ] || [ "$cn_fw_ports_count" -ne 1 ] ||
        [ "$cn_fw_list_item_count" -ne 1 ] || [ "$cn_fw_all_hosts" -ne 1 ] ||
        [ "$cn_fw_all_targets" -ne 1 ] || [ "$cn_fw_all_published" -ne 1 ] ||
        [ "$cn_fw_all_protocols" -ne 1 ]
    then
        cn_firewall_fail \
            "CN Compose overlay must publish exactly its protected IPv4 TCP port"
        return 1
    fi
    cn_fw_extracted_bind=$(awk '$1 == "host_ip:" { print $2 }' \
        "$cn_fw_overlay")
    cn_firewall_validate_ipv4 "$cn_fw_extracted_bind" || return 1
    printf '%s\n' "$cn_fw_extracted_bind"
}

locator_bind=$(extract_overlay_bind "$locator_overlay" 8092)
relay_bind=$(extract_overlay_bind "$relay_overlay" 8091)
if [ "$locator_bind" != "$relay_bind" ] ||
    [ "$locator_bind" != "$OPENPENCIL_CN_SERVICE_IPV4" ]
then
    cn_firewall_fail \
        "inventory service IPv4 must equal both immutable CN Compose host_ip values"
    exit 1
fi

printf '%s\n' "$locator_bind"
