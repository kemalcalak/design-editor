#!/bin/sh
set -eu
LC_ALL=C
export LC_ALL

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
global_config="$script_dir/global-nginx.conf"
cn_config="$script_dir/cn-federation-nginx.conf"
https_config="$script_dir/cn-locator-https-nginx.conf"
global_compose="$script_dir/compose.global.yaml"
cn_compose="$script_dir/compose.cn.yaml"
https_compose="$script_dir/compose.cn-https.yaml"
nft_reference="$script_dir/global-new-connection-rate.nft"
nft_installer="$script_dir/install-global-new-connection-rate.sh"
nft_verifier="$script_dir/verify-global-new-connection-rate.sh"
rate_rules_verifier="$script_dir/../collab-relay-edge/verify-rate-rules.py"
global_deployer="$script_dir/deploy-global.sh"
global_service="$script_dir/openpencil-collab-locator-global.service.example"
crl_rotation="$script_dir/rotate-cn-crl.sh"

require_literal() {
    pattern=$1
    file=$2
    grep -F -- "$pattern" "$file" >/dev/null
}

require_literal_count() {
    expected=$1
    pattern=$2
    file=$3
    actual=$(grep -Fc -- "$pattern" "$file" || true)
    if [ "$actual" -ne "$expected" ]; then
        printf 'error: expected %s occurrence(s) of %s in %s, found %s\n' \
            "$expected" "$pattern" "$file" "$actual" >&2
        exit 1
    fi
}

require_single_upstream_server() {
    upstream=$1
    expected=$2
    file=$3
    declarations=$(grep -Ec \
        "^[[:space:]]*upstream[[:space:]][[:space:]]*$upstream[[:space:]]*\\{" \
        "$file" || true)
    [ "$declarations" -eq 1 ]
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
    servers=$(printf '%s\n' "$block" |
        grep -Ec '^[[:space:]]*server[[:space:]]+' || true)
    [ "$servers" -eq 1 ]
    printf '%s\n' "$block" |
        grep -F "server $expected;" >/dev/null
    if printf '%s\n' "$block" |
        grep -Eq '^[[:space:]]*include[[:space:]]+'
    then
        echo "upstream include directives are forbidden" >&2
        exit 1
    fi
}

validation_mode=${OPENPENCIL_LOCATOR_EDGE_VALIDATION_MODE:-scaffold}
case "$validation_mode" in
    scaffold)
        expected_cn_upstream=192.0.2.30:9543
        expected_inner_upstream=192.0.2.40:8445
        expected_outer_name=locator-cn-federation.example.cn
        expected_inner_host=locator.example.cn
        ;;
    production)
        : "${OPENPENCIL_LOCATOR_EDGE_EXPECTED_CN_FEDERATION_UPSTREAM:?set fixed CN federation IPv4:port}"
        : "${OPENPENCIL_LOCATOR_EDGE_EXPECTED_CN_INNER_HTTPS_UPSTREAM:?set fixed CN inner HTTPS IPv4:port}"
        : "${OPENPENCIL_LOCATOR_EDGE_EXPECTED_CN_FEDERATION_NAME:?set outer TLS DNS name}"
        : "${OPENPENCIL_LOCATOR_EDGE_EXPECTED_INNER_HOST:?set common client hostname}"
        expected_cn_upstream=$OPENPENCIL_LOCATOR_EDGE_EXPECTED_CN_FEDERATION_UPSTREAM
        expected_inner_upstream=$OPENPENCIL_LOCATOR_EDGE_EXPECTED_CN_INNER_HTTPS_UPSTREAM
        expected_outer_name=$OPENPENCIL_LOCATOR_EDGE_EXPECTED_CN_FEDERATION_NAME
        expected_inner_host=$OPENPENCIL_LOCATOR_EDGE_EXPECTED_INNER_HOST
        for endpoint in "$expected_cn_upstream" "$expected_inner_upstream"; do
            if ! printf '%s\n' "$endpoint" |
                grep -Eq '^([0-9]{1,3}\.){3}[0-9]{1,3}:[1-9][0-9]{0,4}$'
            then
                echo "production upstreams must be fixed IPv4:port literals" >&2
                exit 1
            fi
        done
        for hostname in "$expected_outer_name" "$expected_inner_host"; do
            if ! printf '%s\n' "$hostname" |
                grep -Eq '^[a-z0-9]([a-z0-9.-]*[a-z0-9])$' ||
                printf '%s\n' "$hostname" | grep -Eq '(^|\.)example\.(com|cn)$'
            then
                echo "production TLS names must be lowercase non-example DNS names" >&2
                exit 1
            fi
        done
        ;;
    *)
        echo "OPENPENCIL_LOCATOR_EDGE_VALIDATION_MODE must be scaffold or production" >&2
        exit 2
        ;;
esac

for pattern in \
    'listen 8443;' \
    'limit_conn locator_global_clients 32;' \
    'proxy_pass cn_locator_federation;' \
    'proxy_ssl on;' \
    'proxy_ssl_verify on;' \
    "proxy_ssl_name $expected_outer_name;" \
    'proxy_ssl_trusted_certificate /run/secrets/cn-locator-federation-ca.pem;' \
    'proxy_ssl_certificate /run/secrets/global-locator-edge-client-cert.pem;' \
    'proxy_ssl_certificate_key /run/secrets/global-locator-edge-client-key.pem;' \
    'proxy_ssl_session_reuse off;' \
    'proxy_next_upstream off;' \
    'access_log off;'
do
    require_literal "$pattern" "$global_config"
done

for pattern in \
    'listen 9543 ssl;' \
    'limit_conn trusted_locator_edges 128;' \
    'ssl_verify_client on;' \
    'ssl_client_certificate /run/secrets/global-locator-edge-client-ca.pem;' \
    'ssl_crl /run/secrets/global-locator-edge-client-crl.pem;' \
    'ssl_handshake_timeout 5s;' \
    'ssl_session_cache off;' \
    'ssl_session_tickets off;' \
    'proxy_pass cn_locator_inner_https;' \
    'proxy_next_upstream off;' \
    'access_log off;'
do
    require_literal "$pattern" "$cn_config"
done

for pattern in \
    'listen 8445 ssl;' \
    "server_name $expected_inner_host;" \
    "if (\$ssl_server_name != $expected_inner_host) {" \
    "if (\$http_host != $expected_inner_host) {" \
    'if ($request_uri != "/v1/locator") {' \
    'if ($request_uri != "/v1/pairing-code") {' \
    'if ($request_uri != "/v1/pairing-code/claim") {' \
    'client_header_buffer_size 64k;' \
    'large_client_header_buffers 2 64k;' \
    'keepalive_requests 1;' \
    'keepalive_timeout 0;' \
    'location = /v1/locator {' \
    'location = /v1/pairing-code {' \
    'location = /v1/pairing-code/claim {' \
    'limit_except POST {' \
    'if ($http_transfer_encoding != "") {' \
    'if ($http_content_encoding != "") {' \
    'if ($content_length != "191") {' \
    'if ($content_length != "49") {' \
    'client_max_body_size 191;' \
    'client_max_body_size 624;' \
    'client_max_body_size 49;' \
    'client_body_buffer_size 624;' \
    'client_body_buffer_size 49;' \
    'proxy_pass_request_headers off;' \
    "proxy_set_header Host $expected_inner_host;" \
    'proxy_set_header Authorization $http_authorization;' \
    'proxy_set_header Content-Type $http_content_type;' \
    'proxy_set_header Accept $http_accept;' \
    'proxy_set_header Content-Length 191;' \
    'proxy_set_header Transfer-Encoding "";' \
    'proxy_set_header Content-Encoding "";' \
    'proxy_pass http://openpencil_locator/v1/locator;' \
    'proxy_pass http://openpencil_locator/v1/pairing-code;' \
    'proxy_pass http://openpencil_locator/v1/pairing-code/claim;' \
    'application/vnd.openpencil.relay-pairing-publish-v1' \
    'application/vnd.openpencil.relay-pairing-claim-v1' \
    'application/vnd.openpencil.relay-sealed-invite-v1' \
    'proxy_request_buffering on;' \
    'proxy_buffering off;' \
    'location / {' \
    'return 404;' \
    'access_log off;'
do
    require_literal "$pattern" "$https_config"
done

require_single_upstream_server \
    cn_locator_federation "$expected_cn_upstream" "$global_config"
require_single_upstream_server \
    cn_locator_inner_https "$expected_inner_upstream" "$cn_config"
require_single_upstream_server \
    openpencil_locator 'locator:8092' "$https_config"

for config in "$global_config" "$cn_config"; do
    [ "$(grep -Ec '^[[:space:]]*proxy_pass[[:space:]]+' "$config" || true)" -eq 1 ]
    if grep -Eq '^[[:space:]]*include[[:space:]]+' "$config"; then
        echo "Nginx include directives are forbidden in fixed locator-edge configs" >&2
        exit 1
    fi
done
require_literal_count 3 'location = /v1/' "$https_config"
require_literal_count 3 'limit_except POST {' "$https_config"
require_literal_count 3 'proxy_pass_request_headers off;' "$https_config"
require_literal_count 3 'proxy_set_header Authorization $http_authorization;' "$https_config"
require_literal_count 3 'if ($http_transfer_encoding != "") {' "$https_config"
require_literal_count 3 'if ($http_content_encoding != "") {' "$https_config"
require_literal_count 3 'proxy_request_buffering on;' "$https_config"
require_literal_count 3 'proxy_buffering off;' "$https_config"
if [ "$(grep -Ec '^[[:space:]]*proxy_pass[[:space:]]+' "$https_config" || true)" -ne 3 ]; then
    echo "CN inner-HTTPS locator ingress must use exactly three fixed upstream routes" >&2
    exit 1
fi
if grep -Eq '^[[:space:]]*include[[:space:]]+' "$https_config"; then
    echo "Nginx include directives are forbidden in fixed locator-edge configs" >&2
    exit 1
fi

if grep -Eq \
    'proxy_pass[[:space:]]+\$|proxy_next_upstream[[:space:]]+(on|error|timeout|http_)' \
    "$global_config" "$cn_config" "$https_config"
then
    echo "dynamic or fallback locator upstream is forbidden" >&2
    exit 1
fi
if grep -Eq \
    'ssl_preread[[:space:]]+on|listen[[:space:]]+8443[[:space:]]+ssl|proxy_protocol|set_real_ip_from' \
    "$global_config" "$cn_config"
then
    echo "locator stream proxies must not inspect inner TLS or trust source metadata" >&2
    exit 1
fi
if grep -F '/healthz' "$https_config" >/dev/null; then
    echo "the overseas locator ingress must expose only the three exact POST /v1 routes" >&2
    exit 1
fi

for compose_file in "$global_compose" "$cn_compose" "$https_compose"; do
    for pattern in \
        'user: "101:101"' \
        '@sha256:${OPENPENCIL_LOCATOR_EDGE_IMAGE_SHA256:?set its reviewed 64-hex digest}' \
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
        echo "locator edge containers must remain non-root and unprivileged" >&2
        exit 1
    fi
done
require_literal 'restart: "no"' "$global_compose"
require_literal 'restart: unless-stopped' "$cn_compose"
require_literal 'restart: unless-stopped' "$https_compose"

require_literal \
    'host_ip: ${OPENPENCIL_LOCATOR_EDGE_PUBLIC_BIND_IP:?set a dedicated overseas public IP}' \
    "$global_compose"
require_literal 'published: 443' "$global_compose"
require_literal 'global-locator-edge-client-crl.pem' "$cn_compose"
require_literal 'external: true' "$https_compose"

for pattern in \
    'ip daddr 203.0.113.20 tcp dport 443' \
    'ct state new meter locator_edge_new_v4' \
    'ip saddr timeout 2m limit rate over 60/minute burst 20 packets' \
    'counter drop'
do
    require_literal "$pattern" "$nft_reference"
done
for pattern in \
    'Never ask root to reopen or execute a caller-controlled nftables file.' \
    'nft --check --file "$staged"' \
    'nft --file "$staged"' \
    '"$verify_script" "$public_ipv4"'
do
    require_literal "$pattern" "$nft_installer"
done
for pattern in \
    'nft --json --numeric list table inet openpencil_locator_edge_rate' \
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
    'OPENPENCIL_LOCATOR_EDGE_VALIDATION_MODE=production' \
    '--abort-on-container-exit --exit-code-from global-locator-edge'
do
    require_literal "$pattern" "$global_deployer"
done
for pattern in \
    'Requires=docker.service nftables.service' \
    'After=network-online.target nftables.service docker.service' \
    'ExecStart=/opt/openpencil/deploy/collab-relay-locator-edge/deploy-global.sh' \
    'Restart=always'
do
    require_literal "$pattern" "$global_service"
done
for pattern in \
    'candidate CRLNumber must be strictly greater' \
    'candidate CRL drops an existing revoked certificate serial' \
    'CRL activation requires root' \
    'CRL/CA files must be root:101 mode 0440' \
    'OPENPENCIL_LOCATOR_EDGE_VALIDATION_MODE=production' \
    'Revalidate the exact staged inode after its final ownership/mode changes.' \
    'up -d --no-deps --force-recreate' \
    'exec -T locator-cn-federation nginx -t'
do
    require_literal "$pattern" "$crl_rotation"
done

if [ -n "${OPENPENCIL_LOCATOR_EDGE_IMAGE_SHA256:-}" ] &&
    ! printf '%s\n' "$OPENPENCIL_LOCATOR_EDGE_IMAGE_SHA256" |
        grep -Eq '^[0-9a-f]{64}$'
then
    echo "OPENPENCIL_LOCATOR_EDGE_IMAGE_SHA256 must be 64 lowercase hex characters" >&2
    exit 1
fi

if command -v docker >/dev/null 2>&1 &&
    docker compose version >/dev/null 2>&1
then
    test_repository=nginx-unprivileged.invalid/openpencil-locator-edge
    test_digest=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
    OPENPENCIL_LOCATOR_EDGE_IMAGE_REPOSITORY=$test_repository \
    OPENPENCIL_LOCATOR_EDGE_IMAGE_SHA256=$test_digest \
    OPENPENCIL_LOCATOR_EDGE_PUBLIC_BIND_IP=203.0.113.20 \
    OPENPENCIL_LOCATOR_EDGE_CLIENT_CERT=/dev/null \
    OPENPENCIL_LOCATOR_EDGE_CLIENT_KEY=/dev/null \
    OPENPENCIL_LOCATOR_CN_FEDERATION_CA=/dev/null \
        docker compose -f "$global_compose" config -q
    OPENPENCIL_LOCATOR_EDGE_IMAGE_REPOSITORY=$test_repository \
    OPENPENCIL_LOCATOR_EDGE_IMAGE_SHA256=$test_digest \
    OPENPENCIL_LOCATOR_CN_FEDERATION_BIND_IP=10.0.8.22 \
    OPENPENCIL_LOCATOR_CN_FEDERATION_CERT=/dev/null \
    OPENPENCIL_LOCATOR_CN_FEDERATION_KEY=/dev/null \
    OPENPENCIL_LOCATOR_EDGE_CLIENT_CA=/dev/null \
    OPENPENCIL_LOCATOR_EDGE_CLIENT_CRL=/dev/null \
        docker compose -f "$cn_compose" config -q
    OPENPENCIL_LOCATOR_EDGE_IMAGE_REPOSITORY=$test_repository \
    OPENPENCIL_LOCATOR_EDGE_IMAGE_SHA256=$test_digest \
    OPENPENCIL_LOCATOR_CN_HTTPS_BIND_IP=10.0.8.23 \
    OPENPENCIL_LOCATOR_BACKEND_NETWORK=openpencil-collab-locator-cn_default \
    OPENPENCIL_LOCATOR_CN_INNER_FULLCHAIN=/dev/null \
    OPENPENCIL_LOCATOR_CN_INNER_PRIVKEY=/dev/null \
        docker compose -f "$https_compose" config -q
fi

echo "nested-TLS overseas locator ingress validation passed ($validation_mode)"
