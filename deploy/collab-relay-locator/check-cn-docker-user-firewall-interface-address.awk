# Count exact configured IPv4 assignments in `ip -4 -o addr show dev` output.
$3 == "inet" {
    split($4, address_and_prefix, "/")
    if (address_and_prefix[1] == expected_ipv4 &&
        address_and_prefix[2] ~ /^[0-9]+$/) {
        matches++
    }
}
END {
    print matches + 0
}
