# OpenPencil Global ingress to CN home relay

This directory provides a production-oriented, nested-TLS deployment scaffold
for overseas users connecting to a China home collaboration relay.

```text
client
  └─ inner TLS/WSS (untouched)
       └─ Global Nginx L4 stream proxy
            └─ outer mTLS backhaul
                 └─ private CN federation listener
                      └─ unwrap outer mTLS only
                           └─ normal CN TLS/WSS terminator
                                └─ strict GET /v1/tunnel
                                     └─ CN relay authentication + pairing
                                          └─ end-to-end Noise
```

The Global process never terminates the client's inner TLS and therefore
cannot read or log Authorization, the signed locator, bearer capability,
WebSocket headers, or collaboration frames. The CN federation process sees
only the still-encrypted inner TLS byte stream after unwrapping outer mTLS.
Only the normal CN WSS terminator can read the HTTP upgrade, and it must keep
the exact `/v1/tunnel` location policy from `deploy/collab-relay/nginx.conf`.

The signed locator's `home_region=cn` remains authoritative. Neither stream
proxy parses or rewrites it. The CN production authenticator remains the final
anchor and rejects other home regions.

## Fixed routing and fail-closed placeholders

Both Nginx configs intentionally use TEST-NET addresses:

- `global-nginx.conf`: replace `192.0.2.10:9443` with exactly one private or
  tightly firewalled CN federation listener address, and replace
  `cn-federation.example.cn` with its outer-mTLS certificate identity.
- `cn-federation-nginx.conf`: replace `192.0.2.20:8444` with exactly one
  private normal CN WSS terminator's dedicated federation listener address.
  The checked-in terminator config gives this listener a 512-connection
  aggregate ceiling; do not send the shared federation source through the
  public port's per-client eight-connection limit.

Do not use request-derived variables, locator fields, an open forward proxy,
dynamic target selection, alternate upstreams, or `proxy_next_upstream`.
Independent relay replicas do not share pairing state; arbitrary fallback can
send owner and guest to different tables.

The CN federation port is not a public WSS endpoint. Bind it to a private
address with `OPENPENCIL_RELAY_CN_FEDERATION_BIND_IP`, firewall it to approved
Global egress addresses, and require the dedicated Global edge client CA plus
its current CRL. Outer mTLS authenticates the backhaul hop but never replaces
the signed locator, bearer ticket, ticket-to-DH binding, per-connection
challenge-bound X25519 PoP v2, or Noise.

## Inner TLS identity

Clients still perform TLS directly with the normal CN WSS terminator through
both byte proxies. The hostname in the client's `wss://` URL must therefore be
valid on that inner certificate. For a Global-specific DNS name, include that
name on the CN WSS certificate and route its address to the Global ingress.
The Global host itself has no client-facing TLS private key.

## Secrets and container confinement

All outer-mTLS material is mounted from external files:

- Global: its dedicated mTLS client certificate/key and a narrowly pinned CN
  federation server CA.
- CN federation: its outer server certificate/key, the trusted Global edge
  client CA, and a current PEM CRL issued by that CA.

No key belongs in Git, an image layer, an environment value, or Nginx config.
Issue the CN certificate with the fixed federation DNS SAN and `serverAuth`
EKU, and each Global identity with `clientAuth` EKU from a CA used only for
these ingress clients. Rotate identities independently. The checked-in CN
listener requires `/run/secrets/global-edge-client-crl.pem` and disables TLS
session caching. Nginx reads `ssl_crl` when it creates the SSL context, and a
file-backed Compose secret keeps its mounted inode. Replacing the host file
alone does not activate a new CRL.

Both Compose definitions run UID/GID 101, drop every capability, set
`no-new-privileges`, use a read-only root filesystem, and expose only a small
no-exec `/tmp`. Supply an audited Nginx-unprivileged build with the stream,
stream SSL, and stream limit-connection modules, pinned by digest.

Docker Compose file-backed secrets are bind mounts and do not honor
service-level `uid`, `gid`, or `mode` metadata. Before `up`, make every mounted
file actually owned/readable by the dedicated host UID/GID mapped to container
`101:101`; keep private keys at `0400` (or reviewed group-readable `0440`).
The CRL rotation helper specifically requires the active client CA and CRL to
be root-owned, group `101`, and mode `0440` in root-owned non-writable
directories. After container creation, inspect
the effective mount owner/mode and run `nginx -t` as UID 101. A read failure
must stop deployment. Compose builds the image reference from a separate
repository plus a mandatory `sha256:` digest, so a mutable tag cannot satisfy
the deployment contract.

## Required Global IPv4 rate gate

The checked-in Global listener supports one dedicated public IPv4 on fixed
port `443`. IPv6 publication is forbidden until a reviewed `ip6 saddr` meter
exists. Before starting the edge, install and independently verify the exact
per-source initial-SYN meter:

```sh
export OPENPENCIL_RELAY_EDGE_PUBLIC_BIND_IP='<real-dedicated-public-ipv4>'
sudo deploy/collab-relay-edge/install-global-new-connection-rate.sh \
  "$OPENPENCIL_RELAY_EDGE_PUBLIC_BIND_IP"
sudo deploy/collab-relay-edge/verify-global-new-connection-rate.sh \
  "$OPENPENCIL_RELAY_EDGE_PUBLIC_BIND_IP"
```

The installer accepts only a canonical globally routable IPv4, generates one
fixed nftables transaction in a root-owned temporary file, and verifies the
kernel JSON model. It never executes caller-supplied rules. The data relay
uses `openpencil_relay_edge_rate`; the locator uses a separate table.

Persist the table through the host nftables service and restore it before
Docker. Global Compose deliberately has `restart: "no"`. Install the reviewed
`openpencil-collab-relay-global.service.example` after replacing its immutable
release path; it orders nftables before Docker, then reinstalls and verifies
the table before every initial start or crash restart. Direct
`docker compose up` for Global is not a production path.

## Deploy Global

```sh
export OPENPENCIL_RELAY_EDGE_IMAGE_REPOSITORY='registry.example/nginx-unprivileged'
export OPENPENCIL_RELAY_EDGE_IMAGE_SHA256='<reviewed-64-lowercase-hex-digest>'
export OPENPENCIL_RELAY_EDGE_PUBLIC_BIND_IP='<real-dedicated-public-ipv4>'
export OPENPENCIL_RELAY_EDGE_CLIENT_CERT=/secure/global/cn-client.pem
export OPENPENCIL_RELAY_EDGE_CLIENT_KEY=/secure/global/cn-client-key.pem
export OPENPENCIL_RELAY_CN_FEDERATION_CA=/secure/global/cn-federation-ca.pem
export OPENPENCIL_RELAY_EDGE_EXPECTED_CN_FEDERATION_UPSTREAM=10.0.8.12:9443
export OPENPENCIL_RELAY_EDGE_EXPECTED_CN_INNER_WSS_UPSTREAM=10.0.8.13:8444
export OPENPENCIL_RELAY_EDGE_EXPECTED_CN_FEDERATION_NAME=relay-federation.collab.your-company.cn

deploy/collab-relay-edge/deploy-global.sh
```

Replace the angle-bracket and `your-company` examples. The wrapper stays in
the foreground for supervision.

## Deploy the private CN federation listener

```sh
export OPENPENCIL_RELAY_EDGE_IMAGE_REPOSITORY='registry.example/nginx-unprivileged'
export OPENPENCIL_RELAY_EDGE_IMAGE_SHA256='<reviewed-64-lowercase-hex-digest>'
export OPENPENCIL_RELAY_CN_FEDERATION_BIND_IP=10.0.8.12
export OPENPENCIL_RELAY_CN_FEDERATION_CERT=/secure/cn/federation-server.pem
export OPENPENCIL_RELAY_CN_FEDERATION_KEY=/secure/cn/federation-server-key.pem
export OPENPENCIL_RELAY_EDGE_CLIENT_CA=/secure/cn/global-edge-client-ca.pem
export OPENPENCIL_RELAY_EDGE_CLIENT_CRL=/secure/cn/global-edge-client-crl.pem

docker compose \
  -f deploy/collab-relay-edge/compose.cn.yaml \
  config -q
docker compose \
  -f deploy/collab-relay-edge/compose.cn.yaml \
  up -d
```

Verify the effective bind-mount ownership/mode and key readability as UID 101.
Do not work around a key-read failure with root, added capabilities, or a
writable root filesystem.

## Rotate or revoke a Global edge

Generate a newly signed complete CRL at a different absolute path. Its
`CRLNumber` must be strictly greater than the active CRL, its `lastUpdate` and
`nextUpdate` must be current, and it must include the compromised edge
certificate serial. Load the same deployment environment used for the CN
federation service (or its protected `.env` file), including:

```sh
export OPENPENCIL_RELAY_EDGE_IMAGE_REPOSITORY='registry.example/nginx-unprivileged'
export OPENPENCIL_RELAY_EDGE_IMAGE_SHA256='<reviewed-64-lowercase-hex-digest>'
export OPENPENCIL_RELAY_CN_FEDERATION_BIND_IP=10.0.8.12
export OPENPENCIL_RELAY_CN_FEDERATION_CERT=/secure/cn/federation-server.pem
export OPENPENCIL_RELAY_CN_FEDERATION_KEY=/secure/cn/federation-server-key.pem
export OPENPENCIL_RELAY_EDGE_CLIENT_CRL=/secure/cn/global-edge-client-crl.pem
export OPENPENCIL_RELAY_EDGE_CLIENT_CA=/secure/cn/global-edge-client-ca.pem
export OPENPENCIL_RELAY_EDGE_EXPECTED_CN_FEDERATION_UPSTREAM=10.0.8.12:9443
export OPENPENCIL_RELAY_EDGE_EXPECTED_CN_INNER_WSS_UPSTREAM=10.0.8.13:8444
export OPENPENCIL_RELAY_EDGE_EXPECTED_CN_FEDERATION_NAME=relay-federation.collab.your-company.cn
bash deploy/collab-relay-edge/rotate-cn-crl.sh \
  /secure/cn/global-edge-client-crl.next.pem \
  /secure/cn/revoked-global-edge-client-cert.pem
```

The helper first validates the complete Compose environment, then verifies the
candidate signature and target certificate exclusively against the dedicated
client CA. It also requires a monotonic CRL number and current time window,
rejects delta CRLs, preserves every previously revoked serial, and confirms
the newly revoked serial. It preserves the active file's owner and mode,
atomically renames the candidate into place, and force-recreates only the
federation container so both the bind mount and Nginx SSL context load the new
CRL. It rolls the file back and recreates the previous container if activation
fails. On success it intentionally leaves
`global-edge-client-crl.pem.previous`. Before declaring revocation complete,
connect to the private listener with the revoked client certificate and
confirm the TLS handshake is rejected; also confirm an unrevoked edge still
connects. Delete the retained backup only after both probes pass.

## Limits, logging, and source attribution

Access logging is off on both stream proxies. The declared redacted log format
contains only byte counts, status, and duration and is not enabled. Never add
`ssl_preread` payload capture, raw stream dumps, or debug logging in production.

Both hops have one static upstream, disabled retry, bounded file descriptors,
worker connections, buffers, bandwidth, connection counts, and 180-second
idle timeouts. The Global per-source ceiling is 32 because one owner keeps
four warm relay lanes and many users may share an enterprise/CGNAT address;
client heartbeat traffic inside inner TLS keeps a healthy stream active.

L4 stream Nginx has no HTTP request-rate limiter. The checked-in host
new-connection meter and supervised startup gate are therefore required in
addition to the concurrent-connection ceiling. Do not TLS-terminate there.

Without PROXY protocol, the normal CN WSS terminator sees the CN federation
proxy as the TCP source. `deploy/collab-relay/nginx.conf` therefore provides a
separate private `8444` inner-TLS listener with aggregate handshake and
connection ceilings, while public `443` retains its per-client-IP limits.
Global Nginx still sees the real client IP for its connection cap. Keep 8444
private/firewalled to the federation proxy and retain upstream
account/ticket/device rate limits. Do not casually add PROXY protocol: both the
federation egress and inner WSS listener must be configured as a trusted pair
or untrusted clients can spoof source addresses.

## Acceptance checks

1. Run the scaffold validator, then replace the static targets/name, set the
   three `OPENPENCIL_RELAY_EDGE_EXPECTED_*` values, and rerun with
   `OPENPENCIL_RELAY_EDGE_VALIDATION_MODE=production`.
2. The exact nftables table names the Compose IPv4, rejects a single-source
   connection churn test, and is active before the supervised service after a
   reboot.
3. Both Nginx configs pass `nginx -t` with their mounted certificates.
4. Global cannot connect when its client certificate is absent, expired, or
   signed by an untrusted CA. After the documented atomic CRL rotation and
   container recreation, a revoked certificate is rejected while an unrevoked
   edge still connects.
5. Global rejects a CN federation certificate with a wrong name or chain.
6. Direct public traffic cannot reach the CN federation listener.
7. Packet/log inspection at Global reveals only opaque inner TLS bytes, never
   Bearer/capability material.
8. The normal CN terminator still rejects every path except its strict
   `GET /v1/tunnel` WebSocket-upgrade boundary.
9. Overseas owner and guest reach the same CN in-memory pairing table and
   complete the existing end-to-end Noise handshake.
10. A signed non-CN home region remains rejected at the CN authenticator.
