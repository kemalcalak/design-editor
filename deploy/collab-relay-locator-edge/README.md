# Overseas ingress to the CN relay locator

This directory is the control-plane companion to
[`deploy/collab-relay-edge`](../collab-relay-edge/README.md). It lets an
overseas owner publish a CN-home relay locator without turning the Global site
into a locator issuer, TLS terminator, or open proxy.

Use a dedicated overseas public IP on port `443`, separate from the
collaboration-tunnel IP:

```text
owner POST inner HTTPS /v1/locator
  -> dedicated Global public IP:443
  -> Global raw L4 stream proxy (inner TLS untouched)
  -> fixed outer-mTLS connection
  -> private CN locator-federation listener
  -> unwrap outer mTLS only
  -> fixed private CN inner-HTTPS listener
  -> exact-host policy; exact POST /v1/locator, /v1/pairing-code,
     /v1/pairing-code/claim
  -> existing CN locator service :8092
  -> existing ticket verification and HSM locator signing
```

The Global process cannot read the ticket, request, response, or signed
locator. It has one static CN upstream, no `ssl_preread`, no TLS termination,
and no fallback. The CN federation process sees only the still-encrypted inner
TLS stream. The dedicated CN HTTPS terminator is the first hop that can inspect
HTTP; it strips unrelated request headers and accepts only the exact locator
publication path before forwarding to the existing bounded locator service.

This is not a second home region. The locator service still issues only its
configured `home_region=cn`, and the desktop still verifies that signature
before using the result.

## Why a separate public IP

Both the relay tunnel and locator clients use port `443`, but the Global layer
must not inspect SNI or HTTP to choose an upstream. Give the locator ingress a
different public IP and bind it with
`OPENPENCIL_LOCATOR_EDGE_PUBLIC_BIND_IP`. Its listener always reaches the one
CN locator federation target. The relay data-plane listener remains a
different deployment and a different fixed target.

Do not combine these listeners behind request-derived routing, an open
forward proxy, a dynamic resolver, alternate upstreams, or
`proxy_next_upstream`. Do not reuse independent relay/locator replicas without
an explicit stable routing design.

## Fixed placeholders and inner TLS identity

Replace all checked-in fail-closed placeholders:

- Global `192.0.2.30:9543` -> one private/static CN locator-federation address;
- outer identity `locator-cn-federation.example.cn` -> the exact DNS SAN on
  the CN outer-mTLS server certificate;
- CN federation `192.0.2.40:8445` -> one private/static dedicated CN locator
  HTTPS listener;
- inner host `locator.example.cn` -> the hostname in the desktop's HTTPS URL.

The client performs inner TLS directly with the CN HTTPS terminator through
both byte proxies. The CN inner certificate must therefore cover the
client-visible overseas locator hostname. Inner TLS SNI and HTTP `Host` must
both be that same lowercase hostname; missing or different values fail closed.
The Global host has no client-facing TLS private key.

For an overseas desktop:

```sh
export OPENPENCIL_COLLAB_RELAY_LOCATOR_URL='https://locator.example.cn/v1/locator'
```

GeoDNS may resolve that hostname to the dedicated Global locator IP overseas
and the ordinary CN locator ingress domestically. The URL must retain the
exact `/v1/locator` path.

## Separate outer-mTLS trust domain

Provision a locator-edge PKI independent from the collaboration-tunnel edge:

- Global locator edge: a dedicated `clientAuth` certificate/key and a narrowly
  pinned CN locator-federation server CA;
- CN locator federation: a dedicated `serverAuth` certificate/key, the
  dedicated Global locator-edge client CA, and its current complete PEM CRL;
- CN inner HTTPS terminator: the ordinary client-visible locator certificate
  and key.

Do not reuse the relay data-plane edge client certificate, client CA, CRL, or
rotation job. Revoking a locator edge must recreate
`locator-cn-federation`; recreating only the data relay federation does
nothing for this listener.

No private key belongs in Git, an image layer, an environment value, or an
Nginx config. All three Compose definitions run as UID/GID `101`, drop every
capability, use `no-new-privileges`, and have read-only root filesystems.
Supply an audited Nginx-unprivileged image by repository plus reviewed digest.

Docker Compose file-backed secrets are bind mounts and ignore service-level
`uid`, `gid`, and `mode`. Make every mounted file actually readable by the
host identity mapped to container `101:101`; keep private keys at `0400` (or a
reviewed `0440`) and parent directories non-writable by group/other. Inspect
the effective mounts and run `nginx -t` as UID 101 after container creation.

## Deploy the existing CN locator service

Start the existing locator under a deterministic project name so its private
network can be joined by the dedicated HTTPS terminator:

```sh
docker compose \
  -p openpencil-collab-locator-cn \
  -f deploy/collab-relay-locator/compose.yaml \
  up -d --build

export OPENPENCIL_LOCATOR_BACKEND_NETWORK='openpencil-collab-locator-cn_default'
```

Keep this backend network dedicated. Do not publish the Rust service's port
`8092`.

## Deploy the private CN inner-HTTPS listener

```sh
export OPENPENCIL_LOCATOR_EDGE_IMAGE_REPOSITORY='registry.example/nginx-unprivileged'
export OPENPENCIL_LOCATOR_EDGE_IMAGE_SHA256='<reviewed-64-lowercase-hex-digest>'
export OPENPENCIL_LOCATOR_CN_HTTPS_BIND_IP='10.0.8.23'
export OPENPENCIL_LOCATOR_BACKEND_NETWORK='openpencil-collab-locator-cn_default'
export OPENPENCIL_LOCATOR_CN_INNER_FULLCHAIN='/secure/cn/locator-inner-fullchain.pem'
export OPENPENCIL_LOCATOR_CN_INNER_PRIVKEY='/secure/cn/locator-inner-privkey.pem'

docker compose \
  -f deploy/collab-relay-locator-edge/compose.cn-https.yaml \
  config -q
docker compose \
  -f deploy/collab-relay-locator-edge/compose.cn-https.yaml \
  up -d
```

Bind/firewall port `8445` only to the CN locator-federation proxy.

## Deploy the private CN outer-mTLS listener

```sh
export OPENPENCIL_LOCATOR_CN_FEDERATION_BIND_IP='10.0.8.22'
export OPENPENCIL_LOCATOR_CN_FEDERATION_CERT='/secure/cn/locator-federation-server.pem'
export OPENPENCIL_LOCATOR_CN_FEDERATION_KEY='/secure/cn/locator-federation-server-key.pem'
export OPENPENCIL_LOCATOR_EDGE_CLIENT_CA='/secure/cn/locator-edge-client-ca.pem'
export OPENPENCIL_LOCATOR_EDGE_CLIENT_CRL='/secure/cn/locator-edge-client-crl.pem'

docker compose \
  -f deploy/collab-relay-locator-edge/compose.cn.yaml \
  config -q
docker compose \
  -f deploy/collab-relay-locator-edge/compose.cn.yaml \
  up -d
```

Bind/firewall port `9543` only to approved Global locator-edge egress
addresses.

## Required Global IPv4 rate gate

The checked-in Global ingress currently supports one dedicated **IPv4**
public bind only. IPv6 publication is forbidden until an equivalent `ip6
saddr` meter is added and reviewed.

Before starting Compose, install the generated per-source SYN/new-connection
meter for the exact public bind. The root helper accepts only a canonical IPv4
argument and generates the fixed nftables transaction itself; it never
executes a caller-supplied rules file:

```sh
export OPENPENCIL_LOCATOR_EDGE_PUBLIC_BIND_IP='<real-dedicated-public-ipv4>'
sudo deploy/collab-relay-locator-edge/install-global-new-connection-rate.sh \
  "$OPENPENCIL_LOCATOR_EDGE_PUBLIC_BIND_IP"
sudo deploy/collab-relay-locator-edge/verify-global-new-connection-rate.sh \
  "$OPENPENCIL_LOCATOR_EDGE_PUBLIC_BIND_IP"
```

Install this table through the host's boot-persistent nftables configuration
and order its restore before Docker. The Global Compose service deliberately
has `restart: "no"` so Docker can never republish it before that gate. Install
the reviewed
`openpencil-collab-locator-global.service.example` as a host unit after
replacing its immutable release path. That unit orders nftables before Docker,
and its supervised foreground wrapper reinstalls and independently verifies
the exact table before every initial start or crash restart. Reboot once
during acceptance and prove the verifier succeeds before the service starts.
Direct `docker compose up` for the Global file is not a production path.

## Deploy Global

```sh
export OPENPENCIL_LOCATOR_EDGE_PUBLIC_BIND_IP='<real-dedicated-public-ipv4>'
export OPENPENCIL_LOCATOR_EDGE_CLIENT_CERT='/secure/global/locator-edge-client.pem'
export OPENPENCIL_LOCATOR_EDGE_CLIENT_KEY='/secure/global/locator-edge-client-key.pem'
export OPENPENCIL_LOCATOR_CN_FEDERATION_CA='/secure/global/locator-federation-ca.pem'
export OPENPENCIL_LOCATOR_EDGE_EXPECTED_CN_FEDERATION_UPSTREAM='10.0.8.22:9543'
export OPENPENCIL_LOCATOR_EDGE_EXPECTED_CN_INNER_HTTPS_UPSTREAM='10.0.8.23:8445'
export OPENPENCIL_LOCATOR_EDGE_EXPECTED_CN_FEDERATION_NAME='locator-federation.collab.your-company.cn'
export OPENPENCIL_LOCATOR_EDGE_EXPECTED_INNER_HOST='locator.collab.your-company.cn'

deploy/collab-relay-locator-edge/deploy-global.sh
```

Replace the angle-bracket and `your-company` examples before running. The
wrapper is a foreground supervised process; use the checked-in systemd unit
template rather than backgrounding it.

## CRL activation and rotation

Nginx reads `ssl_crl` when it builds the TLS context, while a Compose
file-backed secret retains its mounted inode. Replacing the host CRL file or
reloading only Nginx is insufficient. Use the independent
`rotate-cn-crl.sh` helper to validate and atomically install a complete,
monotonically numbered CRL, then force-recreate
`locator-cn-federation`. The helper preserves a `.previous` file for rollback.

Set `OPENPENCIL_LOCATOR_EDGE_CLIENT_CRL` and
`OPENPENCIL_LOCATOR_EDGE_CLIENT_CA` to the active absolute locator-specific
files. Both active files must be `root:101` mode `0440` in root-owned secure
directories. Also load the same root-owned deployment environment containing
all four audited `OPENPENCIL_LOCATOR_EDGE_EXPECTED_*` values used by
production validation. Then run the helper as root with a candidate complete CRL and the
certificate being revoked. It freezes the candidate bytes, verifies the
dedicated CA and target certificate, requires a strictly increasing
`CRLNumber`, current validity, no delta indicator, preservation of every old
revocation, and the new serial. It atomically publishes the read-only file,
force-recreates only `locator-cn-federation`, runs `nginx -t`, and rolls back
on failure.

After every rotation, prove that the revoked locator-edge certificate is
rejected and an unrevoked locator-edge certificate still connects. Remove the
backup only after both probes pass.

## Limits and logging

Both stream hops have one upstream, disabled retries, bounded connections,
buffers, bandwidth, file descriptors, and 30-second idle timeouts. Access
logging is disabled; do not enable raw stream dumps or debug payload logging.
The host nftables new-connection limiter is a required deployment gate because
L4 Nginx has no HTTP request-rate limiter.

The private HTTPS listener applies an aggregate 100 requests/second and 128
connection ceiling because it sees the federation proxy as one source. The
existing Rust locator retains process-wide request/authentication limits and
ticket verification, but it does not recover a public per-account or
per-device source limiter behind this federation hop. Do not trust public
`X-Forwarded-For` or add PROXY protocol without configuring both ends as a
reviewed trusted pair.

## Validation and acceptance

Run the scaffold validator before replacing its fail-closed examples:

```sh
bash deploy/collab-relay-locator-edge/validate.sh
```

For production, replace the static targets/names in the three Nginx files,
set all four `OPENPENCIL_LOCATOR_EDGE_EXPECTED_*` values shown above, and run
with `OPENPENCIL_LOCATOR_EDGE_VALIDATION_MODE=production`. Production mode
rejects TEST-NET upstreams and example DNS names instead of hard-coding the
scaffold values.

Then verify:

1. All three Nginx configs pass `nginx -t` with real mounted files.
2. The active nftables table names the exact Compose IPv4 bind, survives a
   reboot before Docker startup, and rejects a single-source connection flood.
3. The dedicated Global public IP reaches only the fixed CN locator path.
4. Global cannot connect without its locator-specific client certificate or
   with an expired, untrusted, or activated-revoked certificate.
5. Global rejects a CN outer certificate with the wrong name or chain.
6. Neither private CN listener is reachable from the public Internet.
7. Missing/wrong Host, wrong SNI, methods, query strings, percent-encoded path
   aliases, oversized/short/chunked/compressed bodies, redirects, and extra
   response data are rejected.
8. Packet/log inspection at Global reveals only opaque inner TLS bytes.
9. An overseas owner receives a valid signed `home_region=cn` locator and an
   overseas/domestic guest reaches the same CN relay route.
10. The normal direct CN locator endpoint still works independently.

This creates a cross-border control-plane path carrying an encrypted
authorization request and signed locator response. Complete the applicable
transfer, reachability, latency, and availability review before production.
