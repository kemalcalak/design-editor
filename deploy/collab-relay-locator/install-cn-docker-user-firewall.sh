#!/bin/sh
set -eu
LC_ALL=C
PATH=/usr/sbin:/usr/bin:/sbin:/bin
export LC_ALL PATH

runtime_dir=/usr/local/libexec/openpencil-collab-cn-firewall
config_dir=/etc/openpencil
config_file=$config_dir/collab-cn-firewall.env
binding_file=$config_dir/collab-cn-firewall-compose.env
unit_file=/etc/systemd/system/openpencil-collab-cn-firewall.service
docker_dropin_dir=/etc/systemd/system/docker.service.d
docker_dropin=$docker_dropin_dir/50-openpencil-collab-cn-firewall.conf

if [ "$#" -ne 1 ]; then
    echo "usage: $0 ROOT_OWNED_INVENTORY_FILE" >&2
    exit 2
fi
if [ "$(id -u)" -ne 0 ]; then
    echo "CN firewall: installation requires root" >&2
    exit 1
fi
for command_name in stat install mktemp chmod mv systemctl flock dirname; do
    command -v "$command_name" >/dev/null 2>&1 || {
        echo "CN firewall: required command is unavailable: $command_name" >&2
        exit 1
    }
done

inventory_file=$1
case $inventory_file in
    /*) ;;
    *)
        echo "CN firewall: inventory path must be absolute" >&2
        exit 1
        ;;
esac
if [ ! -f "$inventory_file" ] || [ -L "$inventory_file" ] ||
    [ "$(stat -c '%u:%g:%a' -- "$inventory_file")" != '0:0:600' ]
then
    echo "CN firewall: inventory must be root:root mode 0600 and not a symlink" >&2
    exit 1
fi

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
locator_cn_overlay="$script_dir/compose.production.cn.yaml"
relay_cn_overlay="$script_dir/../collab-relay/compose.production.cn.yaml"
for source_file in \
    cn-docker-user-firewall-common.sh \
    check-cn-docker-user-firewall-references.awk \
    check-cn-docker-user-firewall-interface-address.awk \
    check-cn-docker-user-firewall-managed-chain.awk \
    check-cn-docker-user-firewall-deployment-binding.sh \
    apply-cn-docker-user-firewall.sh \
    verify-cn-docker-user-firewall.sh \
    openpencil-collab-cn-firewall.service \
    openpencil-collab-cn-firewall-docker.conf
do
    if [ ! -f "$script_dir/$source_file" ] || [ -L "$script_dir/$source_file" ]; then
        echo "CN firewall: release artifact is missing or a symlink: $source_file" >&2
        exit 1
    fi
done

for source_file in "$locator_cn_overlay" "$relay_cn_overlay"; do
    if [ ! -f "$source_file" ] || [ -L "$source_file" ]; then
        echo "CN firewall: immutable CN Compose overlay is missing or symlinked" >&2
        exit 1
    fi
done

if systemctl is-active --quiet docker.service; then
    docker_initially_active=1
    docker_initial_pid=$(systemctl show --property MainPID --value docker.service)
    case $docker_initial_pid in
        ''|0|*[!0-9]*)
            echo "CN firewall: active Docker service has no stable main PID" >&2
            exit 1
            ;;
    esac
else
    docker_initially_active=0
    docker_initial_pid=0
fi

install -d -o root -g root -m 0755 "$runtime_dir" "$docker_dropin_dir"
install -o root -g root -m 0644 \
    "$script_dir/cn-docker-user-firewall-common.sh" \
    "$runtime_dir/cn-docker-user-firewall-common.sh"
install -o root -g root -m 0644 \
    "$script_dir/check-cn-docker-user-firewall-references.awk" \
    "$runtime_dir/check-cn-docker-user-firewall-references.awk"
install -o root -g root -m 0644 \
    "$script_dir/check-cn-docker-user-firewall-interface-address.awk" \
    "$runtime_dir/check-cn-docker-user-firewall-interface-address.awk"
install -o root -g root -m 0644 \
    "$script_dir/check-cn-docker-user-firewall-managed-chain.awk" \
    "$runtime_dir/check-cn-docker-user-firewall-managed-chain.awk"
install -o root -g root -m 0755 \
    "$script_dir/check-cn-docker-user-firewall-deployment-binding.sh" \
    "$runtime_dir/check-cn-docker-user-firewall-deployment-binding.sh"
install -o root -g root -m 0755 \
    "$script_dir/apply-cn-docker-user-firewall.sh" \
    "$runtime_dir/apply-cn-docker-user-firewall.sh"
install -o root -g root -m 0755 \
    "$script_dir/verify-cn-docker-user-firewall.sh" \
    "$runtime_dir/verify-cn-docker-user-firewall.sh"

. "$runtime_dir/cn-docker-user-firewall-common.sh"
# This parent walk happens before the delayed copy. A root-owned 0600 file in
# an attacker-writable directory is rejected, closing pathname replacement.
cn_firewall_require_secure_config "$inventory_file"
if [ ! -e "$config_dir" ]; then
    install -d -o root -g root -m 0755 "$config_dir"
elif [ ! -d "$config_dir" ] || [ -L "$config_dir" ] ||
    [ "$(stat -c '%u:%g' -- "$config_dir")" != '0:0' ] ||
    ! cn_firewall_mode_is_directory_safe "$(stat -c '%a' -- "$config_dir")"
then
    echo "CN firewall: existing /etc/openpencil directory is not root-owned and safe" >&2
    exit 1
fi
expected_service_ipv4=$(
    "$runtime_dir/check-cn-docker-user-firewall-deployment-binding.sh" \
        "$inventory_file" "$locator_cn_overlay" "$relay_cn_overlay"
)

# Copy into a root-only staging file and validate those immutable bytes. The
# inventory is parsed as data and is never sourced or evaluated as shell code.
umask 077
staged_config=$(mktemp "$config_dir/.collab-cn-firewall.env.XXXXXX")
staged_binding=$(mktemp "$config_dir/.collab-cn-firewall-compose.env.XXXXXX")
cleanup() {
    rm -f "$staged_config"
    rm -f "$staged_binding"
}
trap cleanup EXIT HUP INT TERM
chmod 0600 "$staged_config"
install -o root -g root -m 0600 "$inventory_file" "$staged_config"
"$runtime_dir/verify-cn-docker-user-firewall.sh" \
    --config-only "$staged_config"
staged_expected_service_ipv4=$(
    "$runtime_dir/check-cn-docker-user-firewall-deployment-binding.sh" \
        "$staged_config" "$locator_cn_overlay" "$relay_cn_overlay"
)
if [ "$staged_expected_service_ipv4" != "$expected_service_ipv4" ]; then
    echo "CN firewall: staged inventory changed during deployment binding" >&2
    exit 1
fi
printf '%s\n' \
    "OPENPENCIL_CN_EXPECTED_SERVICE_IPV4=$expected_service_ipv4" \
    >"$staged_binding"
chmod 0400 "$staged_binding"
cn_firewall_load_config "$staged_config" strict
cn_firewall_load_deployment_binding "$staged_binding"
mv -f "$staged_config" "$config_file"
mv -f "$staged_binding" "$binding_file"
trap - EXIT HUP INT TERM

install -o root -g root -m 0644 \
    "$script_dir/openpencil-collab-cn-firewall.service" "$unit_file"
install -o root -g root -m 0644 \
    "$script_dir/openpencil-collab-cn-firewall-docker.conf" "$docker_dropin"

systemctl daemon-reload
systemctl enable openpencil-collab-cn-firewall.service
if systemctl is-active --quiet docker.service; then
    docker_pid_before=$(systemctl show --property MainPID --value docker.service)
    case $docker_pid_before in
        ''|0|*[!0-9]*)
            echo "CN firewall: active Docker service has no stable main PID" >&2
            exit 1
            ;;
    esac
    if [ "$docker_initially_active" -eq 1 ] &&
        [ "$docker_pid_before" != "$docker_initial_pid" ]
    then
        echo "CN firewall: Docker changed state during installation" >&2
        exit 1
    fi
    "$runtime_dir/apply-cn-docker-user-firewall.sh"
    "$runtime_dir/verify-cn-docker-user-firewall.sh"
    docker_pid_after=$(systemctl show --property MainPID --value docker.service)
    if ! systemctl is-active --quiet docker.service ||
        [ "$docker_pid_after" != "$docker_pid_before" ]
    then
        echo "CN firewall: active Docker PID/state changed during direct reconciliation" >&2
        exit 1
    fi
else
    if systemctl is-active --quiet openpencil-collab-cn-firewall.service; then
        "$runtime_dir/apply-cn-docker-user-firewall.sh"
        "$runtime_dir/verify-cn-docker-user-firewall.sh" --pre-docker
    else
        systemctl start openpencil-collab-cn-firewall.service
        "$runtime_dir/verify-cn-docker-user-firewall.sh" --pre-docker
    fi
fi

echo "CN Docker ingress firewall installed; Docker was not restarted"
