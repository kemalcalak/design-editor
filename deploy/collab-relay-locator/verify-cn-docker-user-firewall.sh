#!/bin/sh
set -eu
LC_ALL=C
PATH=/usr/sbin:/usr/bin:/sbin:/bin
export LC_ALL PATH

runtime_dir=/usr/local/libexec/openpencil-collab-cn-firewall
config_file=/etc/openpencil/collab-cn-firewall.env
binding_file=/etc/openpencil/collab-cn-firewall-compose.env
chain_name=OPENPENCIL-CN-INGRESS
anchor_comment=openpencil-cn-ingress-v1
allow_relay_comment=openpencil-cn-allow-8091-v1
drop_relay_comment=openpencil-cn-drop-8091-v1
allow_locator_comment=openpencil-cn-allow-8092-v1
drop_locator_comment=openpencil-cn-drop-8092-v1
fallthrough_comment=openpencil-cn-fallthrough-v1

if [ "$(id -u)" -ne 0 ]; then
    echo "CN firewall: kernel verification requires root" >&2
    exit 1
fi
for command_name in stat ip iptables-save awk flock; do
    command -v "$command_name" >/dev/null 2>&1 || {
        echo "CN firewall: required command is unavailable: $command_name" >&2
        exit 1
    }
done

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
common_file="$script_dir/cn-docker-user-firewall-common.sh"
reference_checker="$script_dir/check-cn-docker-user-firewall-references.awk"
address_checker="$script_dir/check-cn-docker-user-firewall-interface-address.awk"
managed_chain_checker="$script_dir/check-cn-docker-user-firewall-managed-chain.awk"
if [ "$script_dir" != "$runtime_dir" ] || [ -L "$script_dir" ] ||
    [ "$(stat -c '%u:%g:%a' -- "$script_dir")" != '0:0:755' ] ||
    [ -L "$0" ] || [ "$(stat -c '%u:%g:%a' -- "$0")" != '0:0:755' ] ||
    [ ! -f "$common_file" ] || [ -L "$common_file" ] ||
    [ "$(stat -c '%u:%g:%a' -- "$common_file")" != '0:0:644' ] ||
    [ ! -f "$reference_checker" ] || [ -L "$reference_checker" ] ||
    [ "$(stat -c '%u:%g:%a' -- "$reference_checker")" != '0:0:644' ] ||
    [ ! -f "$address_checker" ] || [ -L "$address_checker" ] ||
    [ "$(stat -c '%u:%g:%a' -- "$address_checker")" != '0:0:644' ] ||
    [ ! -f "$managed_chain_checker" ] || [ -L "$managed_chain_checker" ] ||
    [ "$(stat -c '%u:%g:%a' -- "$managed_chain_checker")" != '0:0:644' ]
then
    echo "CN firewall: installed runtime must be immutable root-owned files" >&2
    exit 1
fi
. "$common_file"

case ${1-} in
    '') cn_fw_verify_mode=kernel; cn_fw_verify_config=$config_file ;;
    --pre-docker)
        if [ "$#" -ne 1 ]; then
            echo "usage: $0 [--pre-docker|--config-only ROOT_OWNED_INVENTORY_FILE]" >&2
            exit 2
        fi
        cn_fw_verify_mode=pre-docker
        cn_fw_verify_config=$config_file
        ;;
    --config-only)
        if [ "$#" -ne 2 ]; then
            echo "usage: $0 [--pre-docker|--config-only ROOT_OWNED_INVENTORY_FILE]" >&2
            exit 2
        fi
        cn_fw_verify_mode=config-only
        cn_fw_verify_config=$2
        ;;
    *)
        echo "usage: $0 [--pre-docker|--config-only ROOT_OWNED_INVENTORY_FILE]" >&2
        exit 2
        ;;
esac

cn_firewall_load_config "$cn_fw_verify_config" strict
if [ "$cn_fw_verify_mode" = config-only ]; then
    echo "root-owned CN firewall inventory is valid"
    exit 0
fi
cn_firewall_load_deployment_binding "$binding_file"
if ! ip link show dev "$OPENPENCIL_CN_INGRESS_INTERFACE" >/dev/null 2>&1; then
    cn_firewall_fail "configured ingress interface does not exist"
    exit 1
fi
cn_fw_interface_addresses=$(ip -4 -o addr show dev \
    "$OPENPENCIL_CN_INGRESS_INTERFACE") || {
    cn_firewall_fail "could not inspect configured ingress-interface addresses"
    exit 1
}
cn_fw_service_assignments=$(printf '%s\n' "$cn_fw_interface_addresses" | awk \
    -v expected_ipv4="$OPENPENCIL_CN_SERVICE_IPV4" -f "$address_checker")
if [ "$cn_fw_service_assignments" -ne 1 ]; then
    cn_firewall_fail \
        "configured service IPv4 must be assigned exactly once to the ingress interface"
    exit 1
fi

exec 9>/run/openpencil-collab-cn-firewall.lock
flock -s 9
filter_snapshot=$(iptables-save -t filter)
anchor_summary=$(printf '%s\n' "$filter_snapshot" | awk \
    -v managed_chain="$chain_name" -v anchor_comment="$anchor_comment" \
    -f "$reference_checker")
if [ "$anchor_summary" != '1 1 1 0' ]; then
    cn_firewall_fail \
        "managed chain requires one first DOCKER-USER jump and no foreign jump/goto references"
    exit 1
fi

if [ "$cn_fw_verify_mode" = kernel ]; then
    forward_summary=$(printf '%s\n' "$filter_snapshot" | awk '
        $1 == "-A" && $2 == "FORWARD" {
            forward_rules++;
            jump = "";
            for (i = 3; i <= NF; i++) {
                if ($i == "-j" && i < NF) jump = $(i + 1);
            }
            if (jump == "DOCKER-USER") {
                targets++;
                if (NF == 4 && $3 == "-j" && $4 == "DOCKER-USER") {
                    canonical++;
                }
                if (forward_rules == 1) first = 1;
            }
        }
        END { print targets + 0, canonical + 0, first + 0 }
    ')
    if [ "$forward_summary" != '1 1 1' ]; then
        cn_firewall_fail \
            "Docker must place one first-position DOCKER-USER jump in FORWARD"
        exit 1
    fi
fi

managed_summary=$(printf '%s\n' "$filter_snapshot" | awk \
    -v managed_chain="$chain_name" \
    -v ingress_interface="$OPENPENCIL_CN_INGRESS_INTERFACE" \
    -v gateway_ipv4="$OPENPENCIL_CN_GATEWAY_SOURCE_IPV4" \
    -v service_ipv4="$OPENPENCIL_CN_SERVICE_IPV4" \
    -v allow_relay_comment="$allow_relay_comment" \
    -v drop_relay_comment="$drop_relay_comment" \
    -v allow_locator_comment="$allow_locator_comment" \
    -v drop_locator_comment="$drop_locator_comment" \
    -v fallthrough_comment="$fallthrough_comment" \
    -f "$managed_chain_checker")
if [ "$managed_summary" != '5 5 5 0' ]; then
    cn_firewall_fail "managed chain predicates, order, or rule count are not exact"
    exit 1
fi

echo "active CN Docker ingress firewall verified"
