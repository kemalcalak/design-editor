#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
compose_file="$script_dir/compose.yaml"
locator_global="$script_dir/compose.production.global.yaml"
locator_cn="$script_dir/compose.production.cn.yaml"
dockerfile="$script_dir/Dockerfile"
nginx_config="$script_dir/nginx-location.conf"
nginx_limits="$script_dir/nginx-http-limits.conf"
nginx_direct_location="$script_dir/nginx-location-direct.conf"
nginx_direct_global="$script_dir/nginx-http-direct.conf"
nginx_direct_cn="$script_dir/nginx-http-direct-cn-gateway.conf"
relay_dir="$script_dir/../collab-relay"
relay_compose="$relay_dir/compose.yaml"
relay_production="$relay_dir/compose.production.yaml"
relay_global="$relay_dir/compose.production.global.yaml"
relay_cn="$relay_dir/compose.production.cn.yaml"
relay_direct_global="$relay_dir/nginx-http-direct.conf"
relay_direct_cn="$relay_dir/nginx-http-direct-cn-gateway.conf"
hsm_dir="$script_dir/../collab-relay-locator-hsm"
hsm_compose="$hsm_dir/compose.yaml"
hsm_overlay="$script_dir/compose.hsm.yaml"
hsm_dockerfile="$hsm_dir/Dockerfile"
hsm_tmpfiles="$hsm_dir/openpencil-locator-hsm.conf"
cn_firewall_validator="$script_dir/validate-cn-docker-user-firewall.sh"

require_literal() {
    pattern=$1
    file=$2
    grep -F "$pattern" "$file" >/dev/null
}

sh "$cn_firewall_validator" "$script_dir"

for pattern in \
    'ENTRYPOINT ["/usr/local/bin/op-collab-relay-locator-server", "--production"]' \
    'FROM rust:1.94-bookworm@sha256:6ae102bdbf528294bc79ad6e1fae682f6f7c2a6e6621506ba959f9685b308a55 AS build' \
    'FROM gcr.io/distroless/cc-debian12:nonroot@sha256:fccdbb0a547c14e23fcf4ce8ad62ca5d43b4faae8d22cd292f490fef9946c96e' \
    'cargo build --locked --release -p op-collab-relay-locator-server'
do
    require_literal "$pattern" "$dockerfile"
done

for pattern in \
    'cargo build --locked --release -p op-collab-relay-locator-hsm' \
    'USER ${SIGNER_UID}:${SHARED_GID}' \
    'HEALTHCHECK --interval=30s' \
    'CMD ["serve", "--config", "/run/openpencil-config/locator-hsm.json"]'
do
    require_literal "$pattern" "$hsm_dockerfile"
done

for pattern in \
    'network_mode: none' \
    'read_only: true' \
    'no-new-privileges:true' \
    'target: /run/secrets/locator-hsm-pin' \
    'target: /var/lib/openpencil-softhsm/tokens' \
    'target: /run/openpencil-hsm'
do
    require_literal "$pattern" "$hsm_compose"
done

require_literal 'condition: service_healthy' "$hsm_overlay"
require_literal 'd /run/openpencil/locator-hsm 0770 root 65532 -' "$hsm_tmpfiles"

for pattern in \
    'read_only: true' \
    'cap_drop:' \
    'no-new-privileges:true' \
    'OPENPENCIL_COLLAB_LOCATOR_TICKET_POLICY_FILE: /run/secrets/collab-policy.json' \
    'OPENPENCIL_COLLAB_LOCATOR_HSM_SOCKET: /run/openpencil-hsm/signer.sock'
do
    require_literal "$pattern" "$compose_file"
done

require_literal 'OPENPENCIL_COLLAB_LOCATOR_HOME_REGION: global' "$locator_global"
require_literal 'OPENPENCIL_COLLAB_LOCATOR_CLIENT_RATE_PER_SECOND: "100"' \
    "$locator_global"
require_literal 'host_ip: 127.0.0.1' "$locator_global"
require_literal 'target: 8092' "$locator_global"
require_literal 'published: "8092"' "$locator_global"
require_literal 'OPENPENCIL_COLLAB_LOCATOR_HOME_REGION: cn' "$locator_cn"
require_literal 'OPENPENCIL_COLLAB_LOCATOR_CLIENT_RATE_PER_SECOND: "100"' \
    "$locator_cn"
require_literal 'host_ip: 10.0.0.10' "$locator_cn"
require_literal 'target: 8092' "$locator_cn"
require_literal 'published: "8092"' "$locator_cn"

require_literal 'OPENPENCIL_COLLAB_RELAY_HOME_REGION: global' "$relay_global"
require_literal 'OPENPENCIL_COLLAB_RELAY_MAX_PENDING_PER_SOURCE: "1024"' \
    "$relay_global"
require_literal 'host_ip: 127.0.0.1' "$relay_global"
require_literal 'target: 8091' "$relay_global"
require_literal 'published: "8091"' "$relay_global"
require_literal 'OPENPENCIL_COLLAB_RELAY_HOME_REGION: cn' "$relay_cn"
require_literal 'OPENPENCIL_COLLAB_RELAY_MAX_PENDING_PER_SOURCE: "1024"' \
    "$relay_cn"
require_literal 'host_ip: 10.0.0.10' "$relay_cn"
require_literal 'target: 8091' "$relay_cn"
require_literal 'published: "8091"' "$relay_cn"

for pattern in \
    'location = /v1/locator {' \
    'if ($request_uri != "/v1/locator") {' \
    'if ($http_host = "") {' \
    'client_max_body_size 191;' \
    'proxy_set_header Authorization $http_authorization;' \
    'proxy_set_header Host $http_host;' \
    'proxy_buffering off;' \
    'location = /healthz {' \
    'access_log off;' \
    'location / {' \
    'return 404;'
do
    require_literal "$pattern" "$nginx_config"
done

require_literal 'proxy_pass http://openpencil_collab_locator/v1/locator;' \
    "$nginx_direct_location"
require_literal 'server 127.0.0.1:8092;' "$nginx_direct_global"
require_literal 'server 10.0.0.10:8092;' "$nginx_direct_cn"
require_literal 'server 127.0.0.1:8091;' "$relay_direct_global"
require_literal 'server 10.0.0.10:8091;' "$relay_direct_cn"

for pattern in \
    'limit_req_zone $binary_remote_addr zone=openpencil_locator_per_source:10m rate=10r/s;' \
    'limit_conn_zone $binary_remote_addr zone=openpencil_locator_connections:10m;' \
    'client_header_buffer_size 64k;' \
    'large_client_header_buffers 2 64k;'
do
    require_literal "$pattern" "$nginx_limits"
done

for pattern in \
    'limit_req zone=openpencil_locator_per_source burst=20 nodelay;' \
    'limit_req_status 429;' \
    'limit_conn openpencil_locator_connections 16;' \
    'limit_conn_status 429;'
do
    require_literal "$pattern" "$nginx_config"
done

if grep -Eq \
    '^[[:space:]]*(privileged:|network_mode:[[:space:]]*host)' \
    "$compose_file"
then
    echo "locator container must not gain host privileges" >&2
    exit 1
fi

if grep -Eq '^[[:space:]]*ports:' "$compose_file" "$relay_compose" \
    "$relay_production" ||
    grep -Eq 'OPENPENCIL_COLLAB_(LOCATOR|RELAY)_(HOST_BIND|HOME_REGION)' \
        "$compose_file" "$relay_compose" "$relay_production" ||
    grep -Eq \
        'OPENPENCIL_COLLAB_LOCATOR_CLIENT_RATE_PER_SECOND|OPENPENCIL_COLLAB_RELAY_MAX_PENDING_PER_SOURCE' \
        "$compose_file" "$relay_compose" "$relay_production" ||
    grep -F '${' "$locator_global" "$locator_cn" "$relay_global" \
        "$relay_cn" >/dev/null
then
    echo "common Compose must not publish ports; regional overlays must be immutable" >&2
    exit 1
fi

if grep -Eq 'host_ip:[[:space:]]*(0\.0\.0\.0|::)' \
    "$locator_global" "$locator_cn" "$relay_global" "$relay_cn"
then
    echo "regional production overlays must not bind wildcard addresses" >&2
    exit 1
fi

if grep -Eq \
    'server[[:space:]]+(relay|locator):809[12]|proxy_pass[[:space:]]+http://(relay|locator):809[12]' \
    "$nginx_direct_location" "$nginx_direct_global" "$nginx_direct_cn" \
    "$relay_direct_global" "$relay_direct_cn"
then
    echo "direct-host Nginx must not use Compose-only service DNS" >&2
    exit 1
fi

if grep -Eq \
    'proxy_pass[[:space:]]+\$|proxy_next_upstream[[:space:]]+(on|error|timeout|http_)' \
    "$nginx_config"
then
    echo "dynamic or fallback locator upstream is forbidden" >&2
    exit 1
fi

if command -v docker >/dev/null 2>&1 &&
    docker compose version >/dev/null 2>&1
then
    OPENPENCIL_COLLAB_HSM_CONFIG_HOST_FILE=/dev/null \
    OPENPENCIL_COLLAB_HSM_PIN_HOST_FILE=/dev/null \
    OPENPENCIL_COLLAB_HSM_TOKEN_HOST_DIR=/tmp \
    OPENPENCIL_COLLAB_HSM_SOCKET_HOST_DIR=/tmp \
        docker compose -f "$hsm_compose" config -q

    for region in global cn
    do
        case $region in
            global)
                bind=127.0.0.1
                hostile_region=cn
                locator_overlay=$locator_global
                relay_overlay=$relay_global
                ;;
            cn)
                bind=10.0.0.10
                hostile_region=global
                locator_overlay=$locator_cn
                relay_overlay=$relay_cn
                ;;
        esac
        locator_config=$(
            OPENPENCIL_COLLAB_LOCATOR_HOME_REGION=$hostile_region \
            OPENPENCIL_COLLAB_LOCATOR_HOST_BIND=0.0.0.0 \
            OPENPENCIL_COLLAB_LOCATOR_CLIENT_RATE_PER_SECOND=1 \
            OPENPENCIL_COLLAB_LOCATOR_HSM_KEY_ID=locator-prod-2026-07 \
            OPENPENCIL_COLLAB_LOCATOR_HSM_PEER_UID=65533 \
            OPENPENCIL_COLLAB_LOCATOR_HSM_PEER_GID=65532 \
            OPENPENCIL_COLLAB_POLICY_HOST_FILE=/dev/null \
            OPENPENCIL_COLLAB_HSM_SOCKET_HOST_DIR=/tmp \
            OPENPENCIL_COLLAB_HSM_CONFIG_HOST_FILE=/dev/null \
            OPENPENCIL_COLLAB_HSM_PIN_HOST_FILE=/dev/null \
            OPENPENCIL_COLLAB_HSM_TOKEN_HOST_DIR=/tmp \
                docker compose \
                -f "$hsm_compose" -f "$compose_file" \
                -f "$locator_overlay" -f "$hsm_overlay" config
        )
        printf '%s\n' "$locator_config" | grep -F "host_ip: $bind" >/dev/null
        printf '%s\n' "$locator_config" | grep -F 'target: 8092' >/dev/null
        printf '%s\n' "$locator_config" | grep -F 'published: "8092"' >/dev/null
        printf '%s\n' "$locator_config" |
            grep -F "OPENPENCIL_COLLAB_LOCATOR_HOME_REGION: $region" >/dev/null
        printf '%s\n' "$locator_config" |
            grep -F 'OPENPENCIL_COLLAB_LOCATOR_CLIENT_RATE_PER_SECOND: "100"' \
                >/dev/null
        if [ "$(printf '%s\n' "$locator_config" |
            grep -Ec '^[[:space:]]+host_ip:')" -ne 1 ] ||
            printf '%s\n' "$locator_config" |
                grep -E '^[[:space:]]+host_ip:' |
                grep -Fv "host_ip: $bind" >/dev/null
        then
            echo "resolved locator bind is not the one immutable regional address" >&2
            exit 1
        fi

        relay_config=$(
            OPENPENCIL_COLLAB_RELAY_HOME_REGION=$hostile_region \
            OPENPENCIL_COLLAB_RELAY_HOST_BIND=0.0.0.0 \
            OPENPENCIL_COLLAB_RELAY_MAX_PENDING_PER_SOURCE=1 \
            OPENPENCIL_COLLAB_POLICY_HOST_FILE=/dev/null \
            OPENPENCIL_RELAY_LOCATOR_KEYS_HOST_FILE=/dev/null \
            OPENPENCIL_RELAY_X25519_KEYS_HOST_FILE=/dev/null \
                docker compose \
                -f "$relay_compose" -f "$relay_production" \
                -f "$relay_overlay" config
        )
        printf '%s\n' "$relay_config" | grep -F "host_ip: $bind" >/dev/null
        printf '%s\n' "$relay_config" | grep -F 'target: 8091' >/dev/null
        printf '%s\n' "$relay_config" | grep -F 'published: "8091"' >/dev/null
        printf '%s\n' "$relay_config" |
            grep -F "OPENPENCIL_COLLAB_RELAY_HOME_REGION: $region" >/dev/null
        printf '%s\n' "$relay_config" |
            grep -F 'OPENPENCIL_COLLAB_RELAY_MAX_PENDING_PER_SOURCE: "1024"' \
                >/dev/null
        if [ "$(printf '%s\n' "$relay_config" |
            grep -Ec '^[[:space:]]+host_ip:')" -ne 1 ] ||
            printf '%s\n' "$relay_config" |
                grep -E '^[[:space:]]+host_ip:' |
                grep -Fv "host_ip: $bind" >/dev/null
        then
            echo "resolved relay bind is not the one immutable regional address" >&2
            exit 1
        fi
    done
fi

echo "collab relay locator deployment validation passed"
