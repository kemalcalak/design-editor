#!/bin/sh
set -eu
LC_ALL=C
export LC_ALL

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
global_config="$script_dir/global-nginx.conf"
cn_config="$script_dir/cn-federation-nginx.conf"
cn_wss_config="$script_dir/../collab-relay/nginx.conf"
relay_dockerfile="$script_dir/../collab-relay/Dockerfile"
global_compose="$script_dir/compose.global.yaml"
cn_compose="$script_dir/compose.cn.yaml"
crl_rotation="$script_dir/rotate-cn-crl.sh"
nft_reference="$script_dir/global-new-connection-rate.nft"
nft_installer="$script_dir/install-global-new-connection-rate.sh"
nft_verifier="$script_dir/verify-global-new-connection-rate.sh"
rate_rules_verifier="$script_dir/verify-rate-rules.py"
global_deployer="$script_dir/deploy-global.sh"
global_service="$script_dir/openpencil-collab-relay-global.service.example"

require_literal() {
    pattern=$1
    file=$2
    grep -F -- "$pattern" "$file" >/dev/null
}

require_single_upstream_server() {
    upstream=$1
    expected=$2
    file=$3
    [ "$(grep -Ec \
        "^[[:space:]]*upstream[[:space:]][[:space:]]*$upstream[[:space:]]*\\{" \
        "$file" || true)" -eq 1 ]
    block=$(awk -v expected_name="$upstream" '
        $1 == "upstream" && $2 == expected_name && $3 == "{" {
            inside = 1
            next
        }
        inside && /^[[:space:]]*}/ {
            exit
        }
        inside {
            print
        }
    ' "$file")
    [ "$(printf '%s\n' "$block" |
        grep -Ec '^[[:space:]]*server[[:space:]]+' || true)" -eq 1 ]
    printf '%s\n' "$block" | grep -F "server $expected;" >/dev/null
    if printf '%s\n' "$block" |
        grep -Eq '^[[:space:]]*include[[:space:]]+'
    then
        echo "upstream include directives are forbidden" >&2
        exit 1
    fi
}

validation_mode=${OPENPENCIL_RELAY_EDGE_VALIDATION_MODE:-scaffold}
case "$validation_mode" in
    scaffold)
        expected_cn_upstream=192.0.2.10:9443
        expected_inner_upstream=192.0.2.20:8444
        expected_outer_name=cn-federation.example.cn
        ;;
    production)
        : "${OPENPENCIL_RELAY_EDGE_EXPECTED_CN_FEDERATION_UPSTREAM:?set fixed CN federation IPv4:port}"
        : "${OPENPENCIL_RELAY_EDGE_EXPECTED_CN_INNER_WSS_UPSTREAM:?set fixed CN inner WSS IPv4:port}"
        : "${OPENPENCIL_RELAY_EDGE_EXPECTED_CN_FEDERATION_NAME:?set outer TLS DNS name}"
        expected_cn_upstream=$OPENPENCIL_RELAY_EDGE_EXPECTED_CN_FEDERATION_UPSTREAM
        expected_inner_upstream=$OPENPENCIL_RELAY_EDGE_EXPECTED_CN_INNER_WSS_UPSTREAM
        expected_outer_name=$OPENPENCIL_RELAY_EDGE_EXPECTED_CN_FEDERATION_NAME
        for endpoint in "$expected_cn_upstream" "$expected_inner_upstream"; do
            if ! printf '%s\n' "$endpoint" |
                grep -Eq '^([0-9]{1,3}\.){3}[0-9]{1,3}:[1-9][0-9]{0,4}$'
            then
                echo "production upstreams must be fixed IPv4:port literals" >&2
                exit 1
            fi
        done
        if ! printf '%s\n' "$expected_outer_name" |
            grep -Eq '^[a-z0-9]([a-z0-9.-]*[a-z0-9])$' ||
            printf '%s\n' "$expected_outer_name" |
                grep -Eq '(^|\.)example\.(com|cn|net|org)$'
        then
            echo "production outer TLS name must be lowercase and non-example" >&2
            exit 1
        fi
        ;;
    *)
        echo "OPENPENCIL_RELAY_EDGE_VALIDATION_MODE must be scaffold or production" >&2
        exit 2
        ;;
esac

for pattern in \
    'listen 8443;' \
    'limit_conn global_clients 32;' \
    'proxy_pass cn_federation_listener;' \
    'proxy_ssl on;' \
    'proxy_ssl_verify on;' \
    "proxy_ssl_name $expected_outer_name;" \
    'proxy_ssl_certificate /run/secrets/global-edge-client-cert.pem;' \
    'proxy_ssl_certificate_key /run/secrets/global-edge-client-key.pem;' \
    'proxy_ssl_trusted_certificate /run/secrets/cn-federation-ca.pem;' \
    'proxy_ssl_session_reuse off;' \
    'proxy_next_upstream off;' \
    'access_log off;'
do
    require_literal "$pattern" "$global_config"
done
for pattern in \
    'listen 9443 ssl;' \
    'limit_conn trusted_global_edges 512;' \
    'ssl_verify_client on;' \
    'ssl_client_certificate /run/secrets/global-edge-client-ca.pem;' \
    'ssl_crl /run/secrets/global-edge-client-crl.pem;' \
    'ssl_handshake_timeout 5s;' \
    'ssl_session_cache off;' \
    'proxy_pass cn_inner_wss;' \
    'proxy_next_upstream off;' \
    'access_log off;'
do
    require_literal "$pattern" "$cn_config"
done

require_single_upstream_server \
    cn_federation_listener "$expected_cn_upstream" "$global_config"
require_single_upstream_server \
    cn_inner_wss "$expected_inner_upstream" "$cn_config"
for config in "$global_config" "$cn_config"; do
    [ "$(grep -Ec '^[[:space:]]*proxy_pass[[:space:]]+' "$config" || true)" -eq 1 ]
    if grep -Eq '^[[:space:]]*include[[:space:]]+' "$config"; then
        echo "Nginx include directives are forbidden in fixed edge configs" >&2
        exit 1
    fi
done
if grep -Eq \
    'proxy_pass[[:space:]]+\$|proxy_next_upstream[[:space:]]+(on|error|timeout|http_)' \
    "$global_config" "$cn_config"
then
    echo "dynamic or fallback upstream is forbidden" >&2
    exit 1
fi
if grep -Eq \
    'ssl_preread[[:space:]]+on|listen[[:space:]]+8443[[:space:]]+ssl|proxy_protocol|set_real_ip_from' \
    "$global_config" "$cn_config"
then
    echo "Global must pass through inner TLS without inspecting source metadata" >&2
    exit 1
fi

require_literal 'location = /v1/tunnel {' "$cn_wss_config"
require_literal 'proxy_set_header Authorization $http_authorization;' "$cn_wss_config"
require_literal 'proxy_pass_header OpenPencil-Relay-Challenge;' "$cn_wss_config"
[ "$(grep -Fc 'client_header_buffer_size 64k;' "$cn_wss_config")" -eq 2 ]
[ "$(grep -Fc 'large_client_header_buffers 2 64k;' "$cn_wss_config")" -eq 2 ]
require_literal \
    'FROM rust:1.94-bookworm@sha256:6ae102bdbf528294bc79ad6e1fae682f6f7c2a6e6621506ba959f9685b308a55 AS build' \
    "$relay_dockerfile"
require_literal \
    'FROM gcr.io/distroless/cc-debian12:nonroot@sha256:fccdbb0a547c14e23fcf4ce8ad62ca5d43b4faae8d22cd292f490fef9946c96e' \
    "$relay_dockerfile"

for compose_file in "$global_compose" "$cn_compose"; do
    for pattern in \
        'user: "101:101"' \
        '@sha256:${OPENPENCIL_RELAY_EDGE_IMAGE_SHA256:?set its reviewed 64-hex digest}' \
        'read_only: true' \
        'cap_drop:' \
        '- ALL' \
        'no-new-privileges:true' \
        'pids_limit:' \
        'mem_limit:' \
        'cpus:'
    do
        require_literal "$pattern" "$compose_file"
    done
    if grep -Eq \
        'privileged:[[:space:]]*true|network_mode:[[:space:]]*host|user:[[:space:]]*(root|"?0)|cap_add:' \
        "$compose_file"
    then
        echo "edge containers must remain non-root and unprivileged" >&2
        exit 1
    fi
done
require_literal 'published: 443' "$global_compose"
require_literal \
    'host_ip: ${OPENPENCIL_RELAY_EDGE_PUBLIC_BIND_IP:?set a dedicated overseas public IPv4}' \
    "$global_compose"
require_literal 'restart: "no"' "$global_compose"
require_literal 'restart: unless-stopped' "$cn_compose"

for pattern in \
    'ip daddr 203.0.113.10 tcp dport 443' \
    'ct state new meter relay_edge_new_v4' \
    'ip saddr timeout 2m limit rate over 60/minute burst 20 packets' \
    'counter drop'
do
    require_literal "$pattern" "$nft_reference"
done
for pattern in \
    'nft --check --file "$staged"' \
    'nft --file "$staged"' \
    '"$verify_script" "$public_ipv4"'
do
    require_literal "$pattern" "$nft_installer"
done
for pattern in \
    'nft --json --numeric list table inet "$table_name"' \
    '"$rules_verifier" --address "$public_ipv4" --rules-json'
do
    require_literal "$pattern" "$nft_verifier"
done
for pattern in \
    'not address.is_global' \
    'len(expressions) != 7' \
    '"rate": 60' \
    '"burst": 20' \
    '"per": "minute"' \
    'expect_equal(expressions[6], {"drop": None}'
do
    require_literal "$pattern" "$rate_rules_verifier"
done
for pattern in \
    '"$script_dir/install-global-new-connection-rate.sh"' \
    'OPENPENCIL_RELAY_EDGE_VALIDATION_MODE=production' \
    '--abort-on-container-exit --exit-code-from global-edge'
do
    require_literal "$pattern" "$global_deployer"
done
for pattern in \
    'Requires=docker.service nftables.service' \
    'After=network-online.target nftables.service docker.service' \
    'ExecStart=/opt/openpencil/deploy/collab-relay-edge/deploy-global.sh' \
    'Restart=always'
do
    require_literal "$pattern" "$global_service"
done
for pattern in \
    'openssl crl' \
    'openssl x509' \
    'candidate CRLNumber must be strictly greater' \
    'delta CRLs are forbidden' \
    'candidate CRL drops an existing revoked certificate serial' \
    'candidate CRL does not revoke the supplied Global edge certificate' \
    'CRL activation requires root' \
    'CRL/CA files must be root:101 mode 0440' \
    'OPENPENCIL_RELAY_EDGE_VALIDATION_MODE=production' \
    'Recheck the final staged inode after ownership and mode changes.' \
    'mv -f "$staged" "$active"' \
    'up -d --no-deps --force-recreate cn-federation' \
    'exec -T cn-federation nginx -t'
do
    require_literal "$pattern" "$crl_rotation"
done

if [ -n "${OPENPENCIL_RELAY_EDGE_IMAGE_SHA256:-}" ] &&
    ! printf '%s\n' "$OPENPENCIL_RELAY_EDGE_IMAGE_SHA256" |
        grep -Eq '^[0-9a-f]{64}$'
then
    echo "OPENPENCIL_RELAY_EDGE_IMAGE_SHA256 must be 64 lowercase hex characters" >&2
    exit 1
fi
if command -v docker >/dev/null 2>&1 &&
    docker compose version >/dev/null 2>&1
then
    test_repository=nginx-unprivileged.invalid/test
    test_digest=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
    OPENPENCIL_RELAY_EDGE_IMAGE_REPOSITORY=$test_repository \
    OPENPENCIL_RELAY_EDGE_IMAGE_SHA256=$test_digest \
    OPENPENCIL_RELAY_EDGE_PUBLIC_BIND_IP=203.0.113.10 \
    OPENPENCIL_RELAY_EDGE_CLIENT_CERT=/dev/null \
    OPENPENCIL_RELAY_EDGE_CLIENT_KEY=/dev/null \
    OPENPENCIL_RELAY_CN_FEDERATION_CA=/dev/null \
        docker compose -f "$global_compose" config -q
    OPENPENCIL_RELAY_EDGE_IMAGE_REPOSITORY=$test_repository \
    OPENPENCIL_RELAY_EDGE_IMAGE_SHA256=$test_digest \
    OPENPENCIL_RELAY_CN_FEDERATION_BIND_IP=10.0.0.1 \
    OPENPENCIL_RELAY_CN_FEDERATION_CERT=/dev/null \
    OPENPENCIL_RELAY_CN_FEDERATION_KEY=/dev/null \
    OPENPENCIL_RELAY_EDGE_CLIENT_CA=/dev/null \
    OPENPENCIL_RELAY_EDGE_CLIENT_CRL=/dev/null \
        docker compose -f "$cn_compose" config -q
fi

echo "nested-TLS collab relay edge validation passed ($validation_mode)"
