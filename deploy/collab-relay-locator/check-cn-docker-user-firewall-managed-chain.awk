# Validate the exact five-rule managed-chain shape from one iptables-save
# snapshot. Output: total valid ordered invalid.
function exact_singleton_ipv4(value, expected_ipv4) {
    # xtables-nft serializes conntrack's exact `/32` original destination as a
    # bare IPv4, while other compatible backends may retain `/32`.
    return value == expected_ipv4 || value == expected_ipv4 "/32"
}

$1 == "-A" && $2 == managed_chain {
    total++
    interface_name = ""
    source_ipv4 = ""
    protocol = ""
    conntrack_modules = 0
    comment_modules = 0
    ct_direction = ""
    original_destination = ""
    original_port = ""
    rule_comment = ""
    target = ""
    unknown = 0
    interface_options = 0
    source_options = 0
    protocol_options = 0
    direction_options = 0
    destination_options = 0
    port_options = 0
    comment_options = 0
    target_options = 0

    for (i = 3; i <= NF; i++) {
        if (($i == "-i" || $i == "-s" || $i == "-p" ||
             $i == "--ctdir" || $i == "--ctorigdst" ||
             $i == "--ctorigdstport" || $i == "--comment" ||
             $i == "-j") && i < NF) {
            option = $i
            value = $(i + 1)
            i++
            if (option == "-i") {
                interface_options++
                interface_name = value
            } else if (option == "-s") {
                source_options++
                source_ipv4 = value
            } else if (option == "-p") {
                protocol_options++
                protocol = value
            } else if (option == "--ctdir") {
                direction_options++
                ct_direction = value
            } else if (option == "--ctorigdst") {
                destination_options++
                original_destination = value
            } else if (option == "--ctorigdstport") {
                port_options++
                original_port = value
            }
            else if (option == "--comment") {
                comment_options++
                rule_comment = value
                gsub(/^"|"$/, "", rule_comment)
            } else if (option == "-j") {
                target_options++
                target = value
            }
        } else if ($i == "-m" && i < NF) {
            module_name = $(i + 1)
            i++
            if (module_name == "conntrack") conntrack_modules++
            else if (module_name == "comment") comment_modules++
            else unknown++
        } else {
            unknown++
        }
    }

    expected_comment = ""
    expected_interface = ""
    expected_source = ""
    expected_protocol = "tcp"
    expected_conntrack_modules = 1
    expected_comment_modules = 1
    expected_direction = "ORIGINAL"
    expected_destination = service_ipv4
    expected_port = ""
    expected_target = ""
    expected_interface_options = 0
    expected_source_options = 0
    expected_protocol_options = 1
    expected_direction_options = 1
    expected_destination_options = 1
    expected_port_options = 1
    expected_comment_options = 1
    expected_target_options = 1
    if (total == 1) {
        expected_comment = allow_relay_comment
        expected_interface = ingress_interface
        expected_source = gateway_ipv4 "/32"
        expected_port = "8091"
        expected_target = "RETURN"
        expected_interface_options = 1
        expected_source_options = 1
    } else if (total == 2) {
        expected_comment = drop_relay_comment
        expected_port = "8091"
        expected_target = "DROP"
    } else if (total == 3) {
        expected_comment = allow_locator_comment
        expected_interface = ingress_interface
        expected_source = gateway_ipv4 "/32"
        expected_port = "8092"
        expected_target = "RETURN"
        expected_interface_options = 1
        expected_source_options = 1
    } else if (total == 4) {
        expected_comment = drop_locator_comment
        expected_port = "8092"
        expected_target = "DROP"
    } else if (total == 5) {
        expected_comment = fallthrough_comment
        expected_protocol = ""
        expected_conntrack_modules = 0
        expected_direction = ""
        expected_destination = ""
        expected_target = "RETURN"
        expected_protocol_options = 0
        expected_direction_options = 0
        expected_destination_options = 0
        expected_port_options = 0
    }

    if (unknown == 0 && rule_comment == expected_comment &&
        interface_name == expected_interface && source_ipv4 == expected_source &&
        protocol == expected_protocol &&
        conntrack_modules == expected_conntrack_modules &&
        comment_modules == expected_comment_modules &&
        ct_direction == expected_direction &&
        ((expected_destination == "" && original_destination == "") ||
         (expected_destination != "" &&
          exact_singleton_ipv4(original_destination, expected_destination))) &&
        original_port == expected_port && target == expected_target &&
        interface_options == expected_interface_options &&
        source_options == expected_source_options &&
        protocol_options == expected_protocol_options &&
        direction_options == expected_direction_options &&
        destination_options == expected_destination_options &&
        port_options == expected_port_options &&
        comment_options == expected_comment_options &&
        target_options == expected_target_options) {
        valid++
        if (total <= 5) ordered++
    } else {
        invalid++
    }
}
END {
    print total + 0, valid + 0, ordered + 0, invalid + 0
}
