# Inspect an iptables-save filter-table snapshot for every reference to the
# managed chain. Output: total canonical first-position foreign.
$1 == "-A" {
    source_chain = $2
    if (source_chain == "DOCKER-USER") docker_user_rules++
    reference_kind = ""
    reference_target = ""
    found_comment = ""
    for (i = 3; i <= NF; i++) {
        if (($i == "-j" || $i == "--jump" || $i == "-g" ||
             $i == "--goto") && i < NF) {
            reference_kind = $i
            reference_target = $(i + 1)
        }
        if ($i == "--comment" && i < NF) {
            found_comment = $(i + 1)
            gsub(/^"|"$/, "", found_comment)
        }
    }
    if (reference_target == managed_chain) {
        total++
        if (source_chain == "DOCKER-USER" && reference_kind == "-j" &&
            NF == 8 && $3 == "-m" && $4 == "comment" &&
            $5 == "--comment" && found_comment == anchor_comment &&
            $7 == "-j" && $8 == managed_chain) {
            canonical++
            if (docker_user_rules == 1) first++
        } else {
            foreign++
        }
    }
}
END {
    print total + 0, canonical + 0, first + 0, foreign + 0
}
