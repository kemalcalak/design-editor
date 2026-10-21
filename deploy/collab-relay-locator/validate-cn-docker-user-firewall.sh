#!/bin/sh
set -eu
LC_ALL=C
export LC_ALL

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
artifact_dir=${1:-$script_dir}
release_artifact_dir=$artifact_dir
if [ "$#" -gt 1 ]; then
    echo "usage: $0 [ARTIFACT_DIRECTORY]" >&2
    exit 2
fi

common_file=cn-docker-user-firewall-common.sh
reference_checker=check-cn-docker-user-firewall-references.awk
address_checker=check-cn-docker-user-firewall-interface-address.awk
managed_chain_checker=check-cn-docker-user-firewall-managed-chain.awk
deployment_checker=check-cn-docker-user-firewall-deployment-binding.sh
config_checker=check-cn-docker-user-firewall-config.sh
config_example=cn-docker-user-firewall.env.example
apply_file=apply-cn-docker-user-firewall.sh
verify_file=verify-cn-docker-user-firewall.sh
install_file=install-cn-docker-user-firewall.sh
unit_file=openpencil-collab-cn-firewall.service
dropin_file=openpencil-collab-cn-firewall-docker.conf

require_literal() {
    cn_fw_pattern=$1
    cn_fw_file=$2
    grep -F -- "$cn_fw_pattern" "$artifact_dir/$cn_fw_file" >/dev/null || {
        echo "CN firewall validation: missing '$cn_fw_pattern' in $cn_fw_file" >&2
        return 1
    }
}

validate_artifacts() {
    cn_fw_dir=$1
    artifact_dir=$cn_fw_dir
    for cn_fw_file in "$common_file" "$reference_checker" "$address_checker" \
        "$managed_chain_checker" "$deployment_checker" "$config_checker" \
        "$config_example" \
        "$apply_file" "$verify_file" "$install_file" "$unit_file" \
        "$dropin_file"
    do
        [ -f "$artifact_dir/$cn_fw_file" ] || {
            echo "CN firewall validation: missing $cn_fw_file" >&2
            return 1
        }
    done
    sh -n "$artifact_dir/$common_file" "$artifact_dir/$config_checker" \
        "$artifact_dir/$deployment_checker" \
        "$artifact_dir/$apply_file" "$artifact_dir/$verify_file" \
        "$artifact_dir/$install_file" "$artifact_dir/$unit_file" \
        "$artifact_dir/$dropin_file"

    for cn_fw_pattern in \
        'cn_firewall_load_config "$config_file" strict' \
        'cn_firewall_load_deployment_binding "$binding_file"' \
        'configured service IPv4 must be assigned exactly once to the ingress interface' \
        'foreign jump or goto reference into the managed chain is forbidden' \
        'existing unanchored managed-chain name is not an exact prior OpenPencil chain' \
        'iptables-restore --wait 10 --noflush --test' \
        'iptables-restore --wait 10 --noflush' \
        '-I DOCKER-USER 1' \
        '-F $chain_name' \
        '--ctdir ORIGINAL' \
        '--ctorigdst $OPENPENCIL_CN_SERVICE_IPV4/32' \
        '--ctorigdstport 8091' \
        '--ctorigdstport 8092' \
        '-j DROP' \
        "$fallthrough_literal"
    do
        require_literal "$cn_fw_pattern" "$apply_file"
    done
    if [ "$(grep -Fc -- '--ctorigdstport 8091' \
        "$artifact_dir/$apply_file")" -ne 2 ] ||
        [ "$(grep -Fc -- '--ctorigdstport 8092' \
            "$artifact_dir/$apply_file")" -ne 2 ] ||
        [ "$(grep -Fc -- '-j DROP' "$artifact_dir/$apply_file")" -ne 2 ]
    then
        echo "CN firewall validation: protected-port allow/drop rules are not exact" >&2
        return 1
    fi

    for cn_fw_pattern in \
        'cn_firewall_load_deployment_binding "$binding_file"' \
        'configured service IPv4 must be assigned exactly once to the ingress interface' \
        'managed chain requires one first DOCKER-USER jump and no foreign jump/goto references' \
        'Docker must place one first-position DOCKER-USER jump in FORWARD' \
        'managed chain predicates, order, or rule count are not exact' \
        'filter_snapshot=$(iptables-save -t filter)'
    do
        require_literal "$cn_fw_pattern" "$verify_file"
    done
    if grep -F 'iptables-restore' "$artifact_dir/$verify_file" >/dev/null; then
        echo "CN firewall validation: read-only verifier must not restore rules" >&2
        return 1
    fi

    for cn_fw_pattern in \
        '$i == "-j"' \
        '$i == "-g"' \
        '$i == "--goto"' \
        'source_chain == "DOCKER-USER"' \
        'foreign++'
    do
        require_literal "$cn_fw_pattern" "$reference_checker"
    done

    for cn_fw_pattern in \
        '$3 == "inet"' \
        'address_and_prefix[1] == expected_ipv4'
    do
        require_literal "$cn_fw_pattern" "$address_checker"
    done
    for cn_fw_pattern in \
        '$1 == "-A" && $2 == managed_chain' \
        'exact_singleton_ipv4(original_destination, expected_destination)' \
        'source_ipv4 == expected_source' \
        'print total + 0, valid + 0, ordered + 0, invalid + 0'
    do
        require_literal "$cn_fw_pattern" "$managed_chain_checker"
    done
    for cn_fw_pattern in \
        'cn_firewall_load_config "$inventory_file" "$ownership_mode"' \
        'CN Compose overlay must publish exactly its protected IPv4 TCP port' \
        'inventory service IPv4 must equal both immutable CN Compose host_ip values' \
        'extract_overlay_bind "$locator_overlay" 8092' \
        'extract_overlay_bind "$relay_overlay" 8091'
    do
        require_literal "$cn_fw_pattern" "$deployment_checker"
    done

    for cn_fw_pattern in \
        'secure data must have the required root:root ownership and mode' \
        'configuration must contain each required key exactly once' \
        'IPv4 values must use canonical dotted-decimal notation' \
        'IPv4 octets must contain at most three digits' \
        'IPv4 value is not canonical dotted-decimal' \
        'inventory service IPv4 differs from the installed Compose binding' \
        'ingress interface exceeds Linux IFNAMSIZ'
    do
        require_literal "$cn_fw_pattern" "$common_file"
    done
    if grep -Eq '^[[:space:]]*(eval|source|\.)[[:space:]]' \
        "$artifact_dir/$common_file"
    then
        echo "CN firewall validation: inventory must never be shell-evaluated" >&2
        return 1
    fi

    for cn_fw_pattern in \
        'OPENPENCIL_CN_INGRESS_INTERFACE=eth0' \
        'OPENPENCIL_CN_GATEWAY_SOURCE_IPV4=198.51.100.10' \
        'OPENPENCIL_CN_SERVICE_IPV4=203.0.113.10'
    do
        require_literal "$cn_fw_pattern" "$config_example"
    done

    for cn_fw_pattern in \
        'inventory must be root:root mode 0600 and not a symlink' \
        'cn_firewall_require_secure_config "$inventory_file"' \
        'check-cn-docker-user-firewall-references.awk' \
        'check-cn-docker-user-firewall-deployment-binding.sh' \
        'staged inventory changed during deployment binding' \
        'existing /etc/openpencil directory is not root-owned and safe' \
        'systemctl enable openpencil-collab-cn-firewall.service' \
        'docker_pid_before=$(systemctl show --property MainPID --value docker.service)' \
        'docker_pid_after=$(systemctl show --property MainPID --value docker.service)' \
        'active Docker PID/state changed during direct reconciliation' \
        'Docker was not restarted'
    do
        require_literal "$cn_fw_pattern" "$install_file"
    done
    if grep -Eq 'systemctl[[:space:]]+(restart|stop|try-restart)' \
        "$artifact_dir/$install_file"
    then
        echo "CN firewall validation: installer must not restart or stop a unit" >&2
        return 1
    fi

    for cn_fw_pattern in \
        'Before=docker.service' \
        'PartOf=docker.service' \
        'ExecStartPost=/usr/local/libexec/openpencil-collab-cn-firewall/verify-cn-docker-user-firewall.sh --pre-docker' \
        'WantedBy=multi-user.target'
    do
        require_literal "$cn_fw_pattern" "$unit_file"
    done
    for cn_fw_pattern in \
        'Requires=openpencil-collab-cn-firewall.service' \
        'After=openpencil-collab-cn-firewall.service' \
        'ExecStartPost=/usr/local/libexec/openpencil-collab-cn-firewall/apply-cn-docker-user-firewall.sh' \
        'ExecStartPost=/usr/local/libexec/openpencil-collab-cn-firewall/verify-cn-docker-user-firewall.sh'
    do
        require_literal "$cn_fw_pattern" "$dropin_file"
    done

    if grep -Eq '(^|[^0-9])(10\.|192\.168\.|172\.(1[6-9]|2[0-9]|3[01])\.)' \
        "$artifact_dir/$common_file" "$artifact_dir/$reference_checker" \
        "$artifact_dir/$address_checker" "$artifact_dir/$managed_chain_checker" \
        "$artifact_dir/$deployment_checker" \
        "$artifact_dir/$config_checker" \
        "$artifact_dir/$config_example" "$artifact_dir/$apply_file" \
        "$artifact_dir/$verify_file" "$artifact_dir/$install_file" \
        "$artifact_dir/$unit_file" "$artifact_dir/$dropin_file"
    then
        echo "CN firewall validation: private inventory addresses must not be checked in" >&2
        return 1
    fi
}

assert_reference_guard() {
    cn_fw_reference_dir=$1
    cn_fw_canonical='-A DOCKER-USER -m comment --comment openpencil-cn-ingress-v1 -j OPENPENCIL-CN-INGRESS'
    cn_fw_summary=$(printf '%s\n' '*filter' ':DOCKER-USER - [0:0]' \
        ':OPENPENCIL-CN-INGRESS - [0:0]' "$cn_fw_canonical" 'COMMIT' |
        awk -v managed_chain=OPENPENCIL-CN-INGRESS \
            -v anchor_comment=openpencil-cn-ingress-v1 \
            -f "$cn_fw_reference_dir/$reference_checker")
    [ "$cn_fw_summary" = '1 1 1 0' ] || return 1

    for cn_fw_foreign_rule in \
        '-A FORWARD -j OPENPENCIL-CN-INGRESS' \
        '-A INPUT -j OPENPENCIL-CN-INGRESS' \
        '-A FOREIGN-CUSTOM -j OPENPENCIL-CN-INGRESS' \
        '-A DOCKER-USER -g OPENPENCIL-CN-INGRESS' \
        '-A FOREIGN-CUSTOM --goto OPENPENCIL-CN-INGRESS'
    do
        cn_fw_summary=$(printf '%s\n' '*filter' ':DOCKER-USER - [0:0]' \
            ':OPENPENCIL-CN-INGRESS - [0:0]' \
            ':FOREIGN-CUSTOM - [0:0]' "$cn_fw_canonical" \
            "$cn_fw_foreign_rule" 'COMMIT' |
            awk -v managed_chain=OPENPENCIL-CN-INGRESS \
                -v anchor_comment=openpencil-cn-ingress-v1 \
                -f "$cn_fw_reference_dir/$reference_checker")
        [ "$cn_fw_summary" = '2 1 1 1' ] || return 1
    done
}

assert_address_guard() {
    cn_fw_address_dir=$1
    cn_fw_fake_ip_output=$(printf '%s\n' \
        '2: eth0    inet 203.0.113.10/24 brd 203.0.113.255 scope global eth0' \
        '2: eth0    inet 203.0.113.20/24 brd 203.0.113.255 scope global secondary eth0')
    cn_fw_summary=$(printf '%s\n' "$cn_fw_fake_ip_output" | awk \
        -v expected_ipv4=203.0.113.10 \
        -f "$cn_fw_address_dir/$address_checker")
    [ "$cn_fw_summary" = 1 ] || return 1
    cn_fw_summary=$(printf '%s\n' "$cn_fw_fake_ip_output" | awk \
        -v expected_ipv4=203.0.113.11 \
        -f "$cn_fw_address_dir/$address_checker")
    [ "$cn_fw_summary" = 0 ] || return 1
    cn_fw_duplicate_output=$(printf '%s\n%s\n' \
        '2: eth0    inet 203.0.113.10/24 scope global eth0' \
        '2: eth0    inet 203.0.113.10/32 scope global secondary eth0')
    cn_fw_summary=$(printf '%s\n' "$cn_fw_duplicate_output" | awk \
        -v expected_ipv4=203.0.113.10 \
        -f "$cn_fw_address_dir/$address_checker")
    [ "$cn_fw_summary" = 2 ] || return 1
}

managed_chain_summary() {
    cn_fw_managed_dir=$1
    cn_fw_managed_snapshot=$2
    printf '%s\n' "$cn_fw_managed_snapshot" | awk \
        -v managed_chain=OPENPENCIL-CN-INGRESS \
        -v ingress_interface=eth0 -v gateway_ipv4=198.51.100.10 \
        -v service_ipv4=203.0.113.10 \
        -v allow_relay_comment=openpencil-cn-allow-8091-v1 \
        -v drop_relay_comment=openpencil-cn-drop-8091-v1 \
        -v allow_locator_comment=openpencil-cn-allow-8092-v1 \
        -v drop_locator_comment=openpencil-cn-drop-8092-v1 \
        -v fallthrough_comment=openpencil-cn-fallthrough-v1 \
        -f "$cn_fw_managed_dir/$managed_chain_checker"
}

assert_managed_chain_guard() {
    cn_fw_managed_dir=$1
    cn_fw_valid_managed=$(printf '%s\n' \
        '-A OPENPENCIL-CN-INGRESS -i eth0 -s 198.51.100.10/32 -p tcp -m conntrack --ctdir ORIGINAL --ctorigdst 203.0.113.10/32 --ctorigdstport 8091 -m comment --comment openpencil-cn-allow-8091-v1 -j RETURN' \
        '-A OPENPENCIL-CN-INGRESS -p tcp -m conntrack --ctdir ORIGINAL --ctorigdst 203.0.113.10/32 --ctorigdstport 8091 -m comment --comment openpencil-cn-drop-8091-v1 -j DROP' \
        '-A OPENPENCIL-CN-INGRESS -i eth0 -s 198.51.100.10/32 -p tcp -m conntrack --ctdir ORIGINAL --ctorigdst 203.0.113.10/32 --ctorigdstport 8092 -m comment --comment openpencil-cn-allow-8092-v1 -j RETURN' \
        '-A OPENPENCIL-CN-INGRESS -p tcp -m conntrack --ctdir ORIGINAL --ctorigdst 203.0.113.10/32 --ctorigdstport 8092 -m comment --comment openpencil-cn-drop-8092-v1 -j DROP' \
        '-A OPENPENCIL-CN-INGRESS -m comment --comment openpencil-cn-fallthrough-v1 -j RETURN')
    [ "$(managed_chain_summary "$cn_fw_managed_dir" \
        "$cn_fw_valid_managed")" = '5 5 5 0' ] || return 1
    cn_fw_live_serialized=$(printf '%s\n' \
        '-A OPENPENCIL-CN-INGRESS -s 198.51.100.10/32 -i eth0 -p tcp -m conntrack --ctorigdst 203.0.113.10 --ctorigdstport 8091 --ctdir ORIGINAL -m comment --comment openpencil-cn-allow-8091-v1 -j RETURN' \
        '-A OPENPENCIL-CN-INGRESS -p tcp -m conntrack --ctorigdst 203.0.113.10 --ctorigdstport 8091 --ctdir ORIGINAL -m comment --comment openpencil-cn-drop-8091-v1 -j DROP' \
        '-A OPENPENCIL-CN-INGRESS -s 198.51.100.10/32 -i eth0 -p tcp -m conntrack --ctorigdst 203.0.113.10 --ctorigdstport 8092 --ctdir ORIGINAL -m comment --comment openpencil-cn-allow-8092-v1 -j RETURN' \
        '-A OPENPENCIL-CN-INGRESS -p tcp -m conntrack --ctorigdst 203.0.113.10 --ctorigdstport 8092 --ctdir ORIGINAL -m comment --comment openpencil-cn-drop-8092-v1 -j DROP' \
        '-A OPENPENCIL-CN-INGRESS -m comment --comment openpencil-cn-fallthrough-v1 -j RETURN')
    [ "$(managed_chain_summary "$cn_fw_managed_dir" \
        "$cn_fw_live_serialized")" = '5 5 5 0' ] || return 1
    cn_fw_unowned='-A OPENPENCIL-CN-INGRESS -j RETURN'
    [ "$(managed_chain_summary "$cn_fw_managed_dir" "$cn_fw_unowned")" != \
        '5 5 5 0' ] || return 1
    cn_fw_weakened=$(printf '%s\n' "$cn_fw_valid_managed" |
        sed '1s/198\.51\.100\.10\/32/198.51.100.11\/32/')
    [ "$(managed_chain_summary "$cn_fw_managed_dir" "$cn_fw_weakened")" != \
        '5 5 5 0' ] || return 1
    cn_fw_wide_destination=$(printf '%s\n' "$cn_fw_live_serialized" |
        sed '1s/203\.0\.113\.10/203.0.113.10\/31/')
    [ "$(managed_chain_summary "$cn_fw_managed_dir" \
        "$cn_fw_wide_destination")" != '5 5 5 0' ] || return 1
    cn_fw_wrong_destination=$(printf '%s\n' "$cn_fw_live_serialized" |
        sed '1s/203\.0\.113\.10/203.0.113.11/')
    [ "$(managed_chain_summary "$cn_fw_managed_dir" \
        "$cn_fw_wrong_destination")" != '5 5 5 0' ] || return 1
}

fallthrough_literal='-A $chain_name -m comment --comment $fallthrough_comment -j RETURN'
validate_artifacts "$artifact_dir"
assert_reference_guard "$artifact_dir" || {
    echo "CN firewall validation: foreign-reference guard rejected its test model" >&2
    exit 1
}
assert_address_guard "$artifact_dir" || {
    echo "CN firewall validation: fake ip-address guard rejected its test model" >&2
    exit 1
}
assert_managed_chain_guard "$artifact_dir" || {
    echo "CN firewall validation: managed-chain ownership guard rejected its test model" >&2
    exit 1
}
sh "$artifact_dir/$config_checker" "$artifact_dir/$config_example" >/dev/null

mutation_dir=$(mktemp -d "${TMPDIR:-/tmp}/openpencil-cn-firewall.XXXXXX")
cleanup() {
    rm -rf "$mutation_dir"
}
trap cleanup EXIT HUP INT TERM
for cn_fw_file in "$common_file" "$reference_checker" "$address_checker" \
    "$managed_chain_checker" "$deployment_checker" "$config_checker" \
    "$config_example" \
    "$apply_file" "$verify_file" "$install_file" "$unit_file" "$dropin_file"
do
    cp "$artifact_dir/$cn_fw_file" "$mutation_dir/$cn_fw_file"
done

expect_config_failure() {
    cn_fw_label=$1
    if sh "$mutation_dir/$config_checker" "$mutation_dir/mutated.env" \
        >/dev/null 2>&1
    then
        echo "CN firewall validation: accepted mutation: $cn_fw_label" >&2
        exit 1
    fi
}

sed 's/OPENPENCIL_CN_INGRESS_INTERFACE=eth0/OPENPENCIL_CN_INGRESS_INTERFACE=eth0;id/' \
    "$artifact_dir/$config_example" >"$mutation_dir/mutated.env"
expect_config_failure 'shell metacharacter in interface'
sed 's/198\.51\.100\.10/gateway.internal/' "$artifact_dir/$config_example" \
    >"$mutation_dir/mutated.env"
expect_config_failure 'hostname in numeric IPv4 field'
sed 's/203\.0\.113\.10/203.0.113.010/' "$artifact_dir/$config_example" \
    >"$mutation_dir/mutated.env"
expect_config_failure 'non-canonical IPv4 octet'
sed 's/198\.51\.100\.10/198.51.100.10./' "$artifact_dir/$config_example" \
    >"$mutation_dir/mutated.env"
expect_config_failure 'trailing empty IPv4 octet'
sed 's/198\.51\.100\.10/999999999999999999999999999999/' \
    "$artifact_dir/$config_example" >"$mutation_dir/mutated.env"
expect_config_failure 'overflowing IPv4 octet'
{
    sed -n '1,$p' "$artifact_dir/$config_example"
    printf '%s\n' 'OPENPENCIL_CN_SERVICE_IPV4=203.0.113.11'
} >"$mutation_dir/mutated.env"
expect_config_failure 'duplicate required key'

printf '%s\n' \
    'services:' \
    '  locator:' \
    '    ports:' \
    '      - target: 8092' \
    '        published: "8092"' \
    '        host_ip: 203.0.113.10' \
    '        protocol: tcp' >"$mutation_dir/locator.cn.yaml"
printf '%s\n' \
    'services:' \
    '  relay:' \
    '    ports:' \
    '      - target: 8091' \
    '        published: "8091"' \
    '        host_ip: 203.0.113.10' \
    '        protocol: tcp' >"$mutation_dir/relay.cn.yaml"
sh "$artifact_dir/$deployment_checker" --syntax-only \
    "$artifact_dir/$config_example" "$mutation_dir/locator.cn.yaml" \
    "$mutation_dir/relay.cn.yaml" >/dev/null
sed 's/203\.0\.113\.10/203.0.113.11/' "$mutation_dir/relay.cn.yaml" \
    >"$mutation_dir/relay.cn.mismatch.yaml"
if sh "$artifact_dir/$deployment_checker" --syntax-only \
    "$artifact_dir/$config_example" "$mutation_dir/locator.cn.yaml" \
    "$mutation_dir/relay.cn.mismatch.yaml" >/dev/null 2>&1
then
    echo "CN firewall validation: mismatched CN overlay bind was accepted" >&2
    exit 1
fi

actual_locator_overlay="$script_dir/compose.production.cn.yaml"
actual_relay_overlay="$script_dir/../collab-relay/compose.production.cn.yaml"
actual_service_ipv4=$(awk '$1 == "host_ip:" { print $2 }' \
    "$actual_locator_overlay")
sed "s/OPENPENCIL_CN_SERVICE_IPV4=203\.0\.113\.10/OPENPENCIL_CN_SERVICE_IPV4=$actual_service_ipv4/" \
    "$artifact_dir/$config_example" >"$mutation_dir/actual.env"
sh "$artifact_dir/$deployment_checker" --syntax-only \
    "$mutation_dir/actual.env" "$actual_locator_overlay" \
    "$actual_relay_overlay" >/dev/null

sed 's/$i == "-g"/$i == "-x"/' \
    "$artifact_dir/$reference_checker" >"$mutation_dir/$reference_checker.tmp"
mv "$mutation_dir/$reference_checker.tmp" "$mutation_dir/$reference_checker"
if assert_reference_guard "$mutation_dir" >/dev/null 2>&1; then
    echo "CN firewall validation: lost goto-reference check was accepted" >&2
    exit 1
fi
cp "$artifact_dir/$reference_checker" "$mutation_dir/$reference_checker"

sed 's/address_and_prefix\[1\] == expected_ipv4/address_and_prefix[1] != expected_ipv4/' \
    "$artifact_dir/$address_checker" >"$mutation_dir/$address_checker.tmp"
mv "$mutation_dir/$address_checker.tmp" "$mutation_dir/$address_checker"
if assert_address_guard "$mutation_dir" >/dev/null 2>&1; then
    echo "CN firewall validation: lost exact live-address check was accepted" >&2
    exit 1
fi
cp "$artifact_dir/$address_checker" "$mutation_dir/$address_checker"

sed 's/source_ipv4 == expected_source/source_ipv4 != expected_source/' \
    "$artifact_dir/$managed_chain_checker" \
    >"$mutation_dir/$managed_chain_checker.tmp"
mv "$mutation_dir/$managed_chain_checker.tmp" \
    "$mutation_dir/$managed_chain_checker"
if assert_managed_chain_guard "$mutation_dir" >/dev/null 2>&1; then
    echo "CN firewall validation: weakened dormant-chain ownership check was accepted" >&2
    exit 1
fi
cp "$artifact_dir/$managed_chain_checker" \
    "$mutation_dir/$managed_chain_checker"

sed 's/value == expected_ipv4 || value == expected_ipv4 "\/32"/value == expected_ipv4 "\/32"/' \
    "$artifact_dir/$managed_chain_checker" \
    >"$mutation_dir/$managed_chain_checker.tmp"
mv "$mutation_dir/$managed_chain_checker.tmp" \
    "$mutation_dir/$managed_chain_checker"
if assert_managed_chain_guard "$mutation_dir" >/dev/null 2>&1; then
    echo "CN firewall validation: lost xtables singleton normalization was accepted" >&2
    exit 1
fi
cp "$artifact_dir/$managed_chain_checker" \
    "$mutation_dir/$managed_chain_checker"

{
    sed -n '1,$p' "$artifact_dir/$install_file"
    printf '%s\n' 'systemctl restart docker.service'
} >"$mutation_dir/$install_file"
if validate_artifacts "$mutation_dir" >/dev/null 2>&1; then
    echo "CN firewall validation: active-Docker restart mutation was accepted" >&2
    exit 1
fi
artifact_dir=$release_artifact_dir
cp "$release_artifact_dir/$install_file" "$mutation_dir/$install_file"

sed 's/--ctorigdstport 8092/--dport 8092/g' \
    "$release_artifact_dir/$apply_file" >"$mutation_dir/$apply_file.tmp"
mv "$mutation_dir/$apply_file.tmp" "$mutation_dir/$apply_file"
if validate_artifacts "$mutation_dir" >/dev/null 2>&1; then
    echo "CN firewall validation: lost original-destination match was accepted" >&2
    exit 1
fi

echo "CN Docker ingress firewall artifacts validated"
