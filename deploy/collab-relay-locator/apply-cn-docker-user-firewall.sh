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

if [ "$#" -ne 0 ]; then
    echo "usage: $0" >&2
    exit 2
fi
if [ "$(id -u)" -ne 0 ]; then
    echo "CN firewall: applying host rules requires root" >&2
    exit 1
fi
for command_name in stat ip iptables iptables-save iptables-restore \
    mktemp chmod flock awk
do
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

cn_firewall_load_config "$config_file" strict
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

case $(iptables --version) in
    *nf_tables*) cn_fw_iptables_backend=nft ;;
    *legacy*) cn_fw_iptables_backend=legacy ;;
    *) cn_firewall_fail "unrecognized iptables backend"; exit 1 ;;
esac
case $(iptables-restore --version) in
    *nf_tables*) cn_fw_restore_backend=nft ;;
    *legacy*) cn_fw_restore_backend=legacy ;;
    *) cn_firewall_fail "unrecognized iptables-restore backend"; exit 1 ;;
esac
if [ "$cn_fw_iptables_backend" != "$cn_fw_restore_backend" ]; then
    cn_firewall_fail "iptables and iptables-restore backends differ"
    exit 1
fi

exec 9>/run/openpencil-collab-cn-firewall.lock
flock -x 9

filter_snapshot=$(iptables-save -t filter)
if printf '%s\n' "$filter_snapshot" |
    grep -F ":$chain_name " >/dev/null
then
    cn_fw_chain_exists=1
else
    cn_fw_chain_exists=0
fi
if printf '%s\n' "$filter_snapshot" |
    grep -F ':DOCKER-USER ' >/dev/null
then
    cn_fw_docker_user_exists=1
else
    cn_fw_docker_user_exists=0
fi

set -- $(printf '%s\n' "$filter_snapshot" | awk \
    -v managed_chain="$chain_name" -v anchor_comment="$anchor_comment" \
    -f "$reference_checker")
cn_fw_target_references=$1
cn_fw_canonical_jumps=$2
cn_fw_foreign_references=$4
if [ "$cn_fw_target_references" -ne "$cn_fw_canonical_jumps" ] ||
    [ "$cn_fw_foreign_references" -ne 0 ]
then
    cn_firewall_fail \
        "foreign jump or goto reference into the managed chain is forbidden"
    exit 1
fi

cn_fw_managed_summary=$(printf '%s\n' "$filter_snapshot" | awk \
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
if [ "$cn_fw_chain_exists" -eq 1 ] && [ "$cn_fw_canonical_jumps" -eq 0 ] &&
    [ "$cn_fw_managed_summary" != '5 5 5 0' ]
then
    cn_firewall_fail \
        "existing unanchored managed-chain name is not an exact prior OpenPencil chain"
    exit 1
fi

umask 077
staged=$(mktemp /run/openpencil-collab-cn-firewall.XXXXXX)
cleanup() {
    rm -f "$staged"
}
trap cleanup EXIT HUP INT TERM
chmod 0600 "$staged"

{
    printf '%s\n' '*filter'
    if [ "$cn_fw_docker_user_exists" -eq 0 ]; then
        printf '%s\n' ':DOCKER-USER - [0:0]'
    fi
    if [ "$cn_fw_chain_exists" -eq 0 ]; then
        printf ':%s - [0:0]\n' "$chain_name"
    fi
    printf '%s\n' "-F $chain_name"
    cn_fw_jump_index=0
    while [ "$cn_fw_jump_index" -lt "$cn_fw_canonical_jumps" ]; do
        printf '%s\n' \
            "-D DOCKER-USER -m comment --comment $anchor_comment -j $chain_name"
        cn_fw_jump_index=$((cn_fw_jump_index + 1))
    done
    printf '%s\n' \
        "-I DOCKER-USER 1 -m comment --comment $anchor_comment -j $chain_name" \
        "-A $chain_name -i $OPENPENCIL_CN_INGRESS_INTERFACE -s $OPENPENCIL_CN_GATEWAY_SOURCE_IPV4/32 -p tcp -m conntrack --ctdir ORIGINAL --ctorigdst $OPENPENCIL_CN_SERVICE_IPV4/32 --ctorigdstport 8091 -m comment --comment $allow_relay_comment -j RETURN" \
        "-A $chain_name -p tcp -m conntrack --ctdir ORIGINAL --ctorigdst $OPENPENCIL_CN_SERVICE_IPV4/32 --ctorigdstport 8091 -m comment --comment $drop_relay_comment -j DROP" \
        "-A $chain_name -i $OPENPENCIL_CN_INGRESS_INTERFACE -s $OPENPENCIL_CN_GATEWAY_SOURCE_IPV4/32 -p tcp -m conntrack --ctdir ORIGINAL --ctorigdst $OPENPENCIL_CN_SERVICE_IPV4/32 --ctorigdstport 8092 -m comment --comment $allow_locator_comment -j RETURN" \
        "-A $chain_name -p tcp -m conntrack --ctdir ORIGINAL --ctorigdst $OPENPENCIL_CN_SERVICE_IPV4/32 --ctorigdstport 8092 -m comment --comment $drop_locator_comment -j DROP" \
        "-A $chain_name -m comment --comment $fallthrough_comment -j RETURN" \
        'COMMIT'
} >"$staged"

# --noflush preserves every unrelated filter rule. Each restore is one kernel
# transaction, so the managed chain and its first-position anchor change as a
# unit or remain at their previous state on failure.
iptables-restore --wait 10 --noflush --test <"$staged"
iptables-restore --wait 10 --noflush <"$staged"

echo "CN Docker ingress firewall reconciled"
