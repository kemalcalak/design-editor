# Shared, non-executable helpers for the CN service-host firewall scripts.
# Configuration is parsed as data. It is never sourced by a shell.

cn_firewall_fail() {
    echo "CN firewall: $*" >&2
    return 1
}

cn_firewall_validate_ipv4() {
    cn_fw_address=$1
    case $cn_fw_address in
        ''|*[!0-9.]*)
            cn_firewall_fail "IPv4 values must use canonical dotted-decimal notation"
            return 1
            ;;
    esac

    cn_fw_old_ifs=$IFS
    IFS=.
    set -- $cn_fw_address
    IFS=$cn_fw_old_ifs
    if [ "$#" -ne 4 ]; then
        cn_firewall_fail "IPv4 values must contain exactly four octets"
        return 1
    fi
    for cn_fw_octet in "$@"; do
        case $cn_fw_octet in
            ''|*[!0-9]*)
                cn_firewall_fail "IPv4 octets must be decimal integers"
                return 1
                ;;
            0) ;;
            0*)
                cn_firewall_fail "IPv4 octets must not contain leading zeroes"
                return 1
                ;;
        esac
        if [ "${#cn_fw_octet}" -gt 3 ]; then
            cn_firewall_fail "IPv4 octets must contain at most three digits"
            return 1
        fi
        if [ "$cn_fw_octet" -gt 255 ]; then
            cn_firewall_fail "IPv4 octets must be in the range 0 through 255"
            return 1
        fi
    done
    cn_fw_canonical_address=$1.$2.$3.$4
    if [ "$cn_fw_address" != "$cn_fw_canonical_address" ]; then
        cn_firewall_fail "IPv4 value is not canonical dotted-decimal"
        return 1
    fi

    case $cn_fw_address in
        0.0.0.0|255.255.255.255)
            cn_firewall_fail "unspecified and limited-broadcast IPv4 values are forbidden"
            return 1
            ;;
    esac
}

cn_firewall_validate_interface() {
    cn_fw_interface=$1
    case $cn_fw_interface in
        [A-Za-z0-9]*) ;;
        *)
            cn_firewall_fail "ingress interface must start with an ASCII alphanumeric"
            return 1
            ;;
    esac
    case $cn_fw_interface in
        *[!A-Za-z0-9_.:-]*)
            cn_firewall_fail "ingress interface contains a forbidden character"
            return 1
            ;;
    esac
    if [ "${#cn_fw_interface}" -gt 15 ]; then
        cn_firewall_fail "ingress interface exceeds Linux IFNAMSIZ"
        return 1
    fi
}

cn_firewall_mode_is_directory_safe() {
    cn_fw_mode=$1
    case $cn_fw_mode in
        [0-7][0-7][0-7]) ;;
        *) return 1 ;;
    esac
    cn_fw_group_digit=${cn_fw_mode#?}
    cn_fw_group_digit=${cn_fw_group_digit%?}
    cn_fw_other_digit=${cn_fw_mode#??}
    case $cn_fw_group_digit$cn_fw_other_digit in
        *[2367]*) return 1 ;;
    esac
}

cn_firewall_require_secure_parent_directories() {
    cn_fw_secure_path=$1
    cn_fw_parent=$(dirname -- "$cn_fw_secure_path")
    while [ "$cn_fw_parent" != / ]; do
        if [ ! -d "$cn_fw_parent" ] || [ -L "$cn_fw_parent" ] ||
            [ "$(stat -c '%u' -- "$cn_fw_parent")" -ne 0 ] ||
            [ "$(stat -c '%g' -- "$cn_fw_parent")" -ne 0 ] ||
            ! cn_firewall_mode_is_directory_safe \
                "$(stat -c '%a' -- "$cn_fw_parent")"
        then
            cn_firewall_fail \
                "configuration parents must be root-owned, non-symlink, and non-writable by group/other"
            return 1
        fi
        cn_fw_parent=$(dirname -- "$cn_fw_parent")
    done
}

cn_firewall_require_secure_root_file() {
    cn_fw_secure_file=$1
    cn_fw_required_mode=$2
    case $cn_fw_secure_file in
        /*) ;;
        *)
            cn_firewall_fail "secure data path must be absolute"
            return 1
            ;;
    esac
    if [ ! -f "$cn_fw_secure_file" ] || [ -L "$cn_fw_secure_file" ]; then
        cn_firewall_fail "secure data must be a regular non-symlink file"
        return 1
    fi
    if [ "$(stat -c '%u' -- "$cn_fw_secure_file")" -ne 0 ] ||
        [ "$(stat -c '%g' -- "$cn_fw_secure_file")" -ne 0 ] ||
        [ "$(stat -c '%a' -- "$cn_fw_secure_file")" != \
            "$cn_fw_required_mode" ]
    then
        cn_firewall_fail \
            "secure data must have the required root:root ownership and mode"
        return 1
    fi
    cn_firewall_require_secure_parent_directories "$cn_fw_secure_file"
}

cn_firewall_require_secure_config() {
    cn_fw_config=$1
    if ! cn_firewall_require_secure_root_file "$cn_fw_config" 600; then
        return 1
    fi
}

cn_firewall_load_config() {
    cn_fw_config=$1
    cn_fw_ownership_mode=$2
    if [ "$cn_fw_ownership_mode" = strict ]; then
        cn_firewall_require_secure_config "$cn_fw_config" || return 1
    elif [ "$cn_fw_ownership_mode" != syntax-only ]; then
        cn_firewall_fail "internal configuration-validation mode is invalid"
        return 1
    fi
    if [ ! -f "$cn_fw_config" ] || [ -L "$cn_fw_config" ]; then
        cn_firewall_fail "configuration must be a regular non-symlink file"
        return 1
    fi

    OPENPENCIL_CN_INGRESS_INTERFACE=
    OPENPENCIL_CN_GATEWAY_SOURCE_IPV4=
    OPENPENCIL_CN_SERVICE_IPV4=
    cn_fw_seen_interface=0
    cn_fw_seen_source=0
    cn_fw_seen_service=0
    cn_fw_line_number=0

    while IFS= read -r cn_fw_line || [ -n "$cn_fw_line" ]; do
        cn_fw_line_number=$((cn_fw_line_number + 1))
        case $cn_fw_line in
            ''|'#'*) continue ;;
            *=*) ;;
            *)
                cn_firewall_fail \
                    "invalid configuration line $cn_fw_line_number"
                return 1
                ;;
        esac
        cn_fw_key=${cn_fw_line%%=*}
        cn_fw_value=${cn_fw_line#*=}
        if [ -z "$cn_fw_value" ]; then
            cn_firewall_fail \
                "empty configuration value on line $cn_fw_line_number"
            return 1
        fi
        case $cn_fw_key in
            OPENPENCIL_CN_INGRESS_INTERFACE)
                if [ "$cn_fw_seen_interface" -ne 0 ]; then
                    cn_firewall_fail "duplicate ingress-interface key"
                    return 1
                fi
                cn_fw_seen_interface=1
                OPENPENCIL_CN_INGRESS_INTERFACE=$cn_fw_value
                ;;
            OPENPENCIL_CN_GATEWAY_SOURCE_IPV4)
                if [ "$cn_fw_seen_source" -ne 0 ]; then
                    cn_firewall_fail "duplicate gateway-source key"
                    return 1
                fi
                cn_fw_seen_source=1
                OPENPENCIL_CN_GATEWAY_SOURCE_IPV4=$cn_fw_value
                ;;
            OPENPENCIL_CN_SERVICE_IPV4)
                if [ "$cn_fw_seen_service" -ne 0 ]; then
                    cn_firewall_fail "duplicate service-address key"
                    return 1
                fi
                cn_fw_seen_service=1
                OPENPENCIL_CN_SERVICE_IPV4=$cn_fw_value
                ;;
            *)
                cn_firewall_fail \
                    "unknown configuration key on line $cn_fw_line_number"
                return 1
                ;;
        esac
    done <"$cn_fw_config"

    if [ "$cn_fw_seen_interface" -ne 1 ] ||
        [ "$cn_fw_seen_source" -ne 1 ] ||
        [ "$cn_fw_seen_service" -ne 1 ]
    then
        cn_firewall_fail "configuration must contain each required key exactly once"
        return 1
    fi
    cn_firewall_validate_interface "$OPENPENCIL_CN_INGRESS_INTERFACE" ||
        return 1
    cn_firewall_validate_ipv4 "$OPENPENCIL_CN_GATEWAY_SOURCE_IPV4" ||
        return 1
    cn_firewall_validate_ipv4 "$OPENPENCIL_CN_SERVICE_IPV4" || return 1
    if [ "$OPENPENCIL_CN_GATEWAY_SOURCE_IPV4" = \
        "$OPENPENCIL_CN_SERVICE_IPV4" ]
    then
        cn_firewall_fail "gateway source and service destination must differ"
        return 1
    fi
}

cn_firewall_load_deployment_binding() {
    cn_fw_binding_file=$1
    cn_firewall_require_secure_root_file "$cn_fw_binding_file" 400 || return 1
    OPENPENCIL_CN_EXPECTED_SERVICE_IPV4=
    cn_fw_binding_seen=0
    cn_fw_binding_line_number=0
    while IFS= read -r cn_fw_binding_line || [ -n "$cn_fw_binding_line" ]; do
        cn_fw_binding_line_number=$((cn_fw_binding_line_number + 1))
        case $cn_fw_binding_line in
            OPENPENCIL_CN_EXPECTED_SERVICE_IPV4=*)
                if [ "$cn_fw_binding_seen" -ne 0 ]; then
                    cn_firewall_fail "duplicate deployment-binding key"
                    return 1
                fi
                cn_fw_binding_seen=1
                OPENPENCIL_CN_EXPECTED_SERVICE_IPV4=${cn_fw_binding_line#*=}
                ;;
            *)
                cn_firewall_fail \
                    "invalid deployment-binding line $cn_fw_binding_line_number"
                return 1
                ;;
        esac
    done <"$cn_fw_binding_file"
    if [ "$cn_fw_binding_seen" -ne 1 ]; then
        cn_firewall_fail "deployment binding must contain exactly one key"
        return 1
    fi
    cn_firewall_validate_ipv4 "$OPENPENCIL_CN_EXPECTED_SERVICE_IPV4" ||
        return 1
    if [ "$OPENPENCIL_CN_EXPECTED_SERVICE_IPV4" != \
        "$OPENPENCIL_CN_SERVICE_IPV4" ]
    then
        cn_firewall_fail \
            "inventory service IPv4 differs from the installed Compose binding"
        return 1
    fi
}
