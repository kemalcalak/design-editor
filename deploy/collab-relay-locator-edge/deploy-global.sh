#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
compose_file="$script_dir/compose.global.yaml"

: "${DESIGN_EDITOR_LOCATOR_EDGE_PUBLIC_BIND_IP:?set the dedicated overseas IPv4}"
: "${DESIGN_EDITOR_LOCATOR_EDGE_EXPECTED_CN_FEDERATION_UPSTREAM:?set the audited fixed host:port}"
: "${DESIGN_EDITOR_LOCATOR_EDGE_EXPECTED_CN_FEDERATION_NAME:?set the audited outer TLS DNS name}"
: "${DESIGN_EDITOR_LOCATOR_EDGE_EXPECTED_INNER_HOST:?set the audited common client hostname}"
: "${DESIGN_EDITOR_LOCATOR_EDGE_EXPECTED_CN_INNER_HTTPS_UPSTREAM:?set the audited fixed inner host:port}"

# This is the supported Global startup path. Reinstall the one canonical table
# before every start, then independently compare the kernel's JSON model.
"$script_dir/install-global-new-connection-rate.sh" \
    "$DESIGN_EDITOR_LOCATOR_EDGE_PUBLIC_BIND_IP"

DESIGN_EDITOR_LOCATOR_EDGE_VALIDATION_MODE=production \
    "$script_dir/validate.sh"
docker compose -f "$compose_file" config -q
exec docker compose -f "$compose_file" up \
    --abort-on-container-exit --exit-code-from global-locator-edge
