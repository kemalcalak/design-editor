#!/bin/sh
set -eu
LC_ALL=C
export LC_ALL

if [ "$#" -ne 1 ]; then
    echo "usage: $0 INVENTORY_FILE" >&2
    exit 2
fi

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
# This helper performs syntax checks only for release validation. The installed
# mutating and kernel-verification paths independently require root ownership.
. "$script_dir/cn-docker-user-firewall-common.sh"
cn_firewall_load_config "$1" syntax-only

echo "CN firewall inventory syntax is valid"
