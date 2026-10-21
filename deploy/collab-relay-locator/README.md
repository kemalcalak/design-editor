# OpenPencil relay locator service

This directory packages the production `POST /v1/locator` control plane. It is
separate from the blind WebSocket relay. The service verifies a current
device-DH-bound collaboration ticket, applies the configured home-region
policy, and asks an external HSM/KMS process to sign the public locator claims.
It never receives the invite capability and contains no locator private key.

## Network boundary

The Rust service speaks bounded HTTP/1 on port 8092 inside the deployment
network. Terminate public TLS at a trusted ingress, include
`nginx-http-limits.conf` once in its `http` block, and include
`nginx-location.conf` in the exact-host TLS server. The checked-in zones
enforce per-source request and connection limits before the process-wide
aggregate limiter. Docker NAT collapses clients crossing the host-published
port into one locator-visible peer address. The immutable regional overlays
therefore set `OPENPENCIL_COLLAB_LOCATOR_CLIENT_RATE_PER_SECOND=100`, equal to
the global locator ceiling, while the common Compose file sets no production
override. Host Nginx is the trusted real-source boundary and must retain the
checked-in per-source zones. If a load balancer sits in front, restore client
addresses only from its fixed trusted addresses; never trust a public
`X-Forwarded-For`. The ingress checks the raw `$request_uri`, so queries and
percent-encoded path aliases cannot be normalized into the route, and it
requires a real client `Host` header instead of Nginx's fallback host. Only
exact `POST /v1/locator`, `POST /v1/pairing-code`, and
`POST /v1/pairing-code/claim` are accepted. `/healthz` returns an empty 204 and
exposes no configuration or dependency status.

The server enforces:

- at most 32 headers and a 64 KiB HTTP header buffer, enough for the bounded
  48 KiB collaboration-ticket envelope;
- a five-second header timeout and body timeout;
- exactly one strict `Authorization: Bearer` value;
- the collaboration-ticket maximum plus the seven-byte Bearer prefix;
- exact request and response media types;
- an exact 191-byte non-chunked request body;
- per-source limits at the TLS ingress plus bounded process-wide authenticated
  requests per second, connections, requests, and concurrent authentication
  or HSM operations;
- a five-second authentication deadline and bounded graceful shutdown;
- empty, generic failure responses with no credential, locator, socket path,
  or account logging.

The desktop request contains the region, fresh public route id/generation,
owner Noise public key, stable relay discovery id, and lifetime. Its locally
generated 32-byte capability is never sent to this service. The returned
locator is accepted by the desktop only after pinned-signature verification
and exact comparison with that pending request.

## HSM socket

Production signing uses a fixed absolute Unix-domain socket. Every operation
rechecks that the full path contains no symlink, the final node is a
non-world-writable socket owned by the configured peer UID/GID, connects with
a timeout, and authenticates the connected peer using `SO_PEERCRED` on Linux
or `getpeereid` on supported BSD/macOS systems. Unsupported Unix platforms
fail closed.

Each connection carries one fixed request and one fixed response:

```text
request (339 bytes)
  "OPLS" | version=1 | operation=1
  | key_id_len:u8 | key_id:[64, zero padded]
  | locator_canonical_signing_bytes:[268]

response (70 bytes, then EOF)
  "OPLR" | version=1 | status:u8 | signature:[64]
```

Status `0` is success and `1` is rejection. Unknown status, zero signatures,
truncation, trailing bytes, timeout, peer mismatch, and socket replacement all
fail closed. The isolated SoftHSM adapter in
[`deploy/collab-relay-locator-hsm`](../collab-relay-locator-hsm/README.md)
implements this boundary. It authorizes only the configured active key id and
keeps the Ed25519 private key and token PIN outside this container.

## Configuration

Required runtime environment variables (the compose file supplies the two
container paths and listen address):

- `OPENPENCIL_COLLAB_LOCATOR_HOME_REGION=cn|global`;
- `OPENPENCIL_COLLAB_LOCATOR_TICKET_POLICY_FILE`, the absolute container path
  to the public signed collaboration union-policy JSON;
- `OPENPENCIL_COLLAB_LOCATOR_HSM_SOCKET`, the absolute container path to the
  external signer socket;
- `OPENPENCIL_COLLAB_LOCATOR_HSM_KEY_ID`, a public locator key id;
- `OPENPENCIL_COLLAB_LOCATOR_HSM_PEER_UID` and
  `OPENPENCIL_COLLAB_LOCATOR_HSM_PEER_GID`.

The compose wrapper instead requires these host mount inputs:

- `OPENPENCIL_COLLAB_POLICY_HOST_FILE`, an absolute host path to the public
  signed collaboration union-policy JSON;
- `OPENPENCIL_COLLAB_HSM_SOCKET_HOST_DIR`, an absolute host directory
  containing `signer.sock`.

On Unix, the public signed-policy file may be owned either by the locator's
effective UID or by root. It must be a regular non-symlink file and must not be
group- or world-writable. Root-owned mode `0440` (when the container identity
has group read) or `0444` is accepted because the policy contains public
verification material; writable modes fail closed.

The compose file mounts policy and socket inputs read-only, drops all Linux
capabilities, uses the distroless non-root identity, enables a read-only root
filesystem, and publishes no host port. The immutable Global production
overlay sets `home_region=global` and binds `127.0.0.1:8092`; the immutable CN
overlay sets `home_region=cn` and binds `10.0.0.10:8092`. There is no
host-bind variable or wildcard default. Restrict the CN service-host firewall
to the front gateway.

Both Dockerfile base images are pinned by multi-architecture manifest digest,
verified from their upstream registries on 2026-07-29. Base-image updates must
resolve and review new upstream manifests/platforms, replace both
tag-plus-digest `FROM` values, rebuild with `--pull --no-cache`, and rerun the
locator validator and collaboration security gate.

Optional runtime settings are
`OPENPENCIL_COLLAB_LOCATOR_LISTEN` (default `127.0.0.1:8092`; compose uses
`0.0.0.0:8092`),
`OPENPENCIL_COLLAB_LOCATOR_POLICY_MAX_AGE_SECONDS` (`1..=3600`),
`OPENPENCIL_COLLAB_LOCATOR_HSM_TIMEOUT_MS` (`50..=5000`),
`OPENPENCIL_COLLAB_LOCATOR_MAX_AUTH_IN_FLIGHT` (`1..=256`), and
`OPENPENCIL_COLLAB_LOCATOR_RATE_PER_SECOND` (`1..=10000`).
`OPENPENCIL_COLLAB_LOCATOR_CLIENT_RATE_PER_SECOND` (`1..=10000`) is also
optional outside the immutable regional production overlays; those overlays
fix it at `100` because Docker NAT removes usable client identity at the
locator socket. The optional
`OPENPENCIL_COLLAB_LOCATOR_LOG_LEVEL` accepts only `error`, `warn`, `info`, or
`debug` and is scoped to this crate; arbitrary dependency trace filters are
not accepted, so HTTP headers cannot be enabled through logging configuration.

Validate the fail-closed deployment invariants and resolved compose model:

```sh
deploy/collab-relay-locator/validate.sh
```

Start after provisioning the public policy file and signer token:

```sh
export OPENPENCIL_COLLAB_POLICY_HOST_FILE=/secure/public/collab-policy.json
export OPENPENCIL_COLLAB_LOCATOR_HSM_KEY_ID=replace-with-active-public-kid
export OPENPENCIL_COLLAB_LOCATOR_HSM_PEER_UID=65533
export OPENPENCIL_COLLAB_LOCATOR_HSM_PEER_GID=65532
docker compose \
  -f deploy/collab-relay-locator-hsm/compose.yaml \
  -f deploy/collab-relay-locator/compose.yaml \
  -f deploy/collab-relay-locator/compose.production.global.yaml \
  -f deploy/collab-relay-locator/compose.hsm.yaml \
  up --build
```

That command is for Global. On CN, replace only the regional overlay with
`compose.production.cn.yaml`. The base file alone supplies neither a home
region nor a host port, and the two regional overlays must never be combined.

For a CN home session, deploy this service and the blind relay in the CN
region. Any user who selects CN calls the CN application's `/v1/locator` and
`/v1/tunnel` paths directly over TLS/WSS. Global home sessions use the same
two paths on the Global application host. Concrete hosts stay in private
deployment inventory and signed bootstrap metadata; this direct topology does
not require a Global L4 edge. The two Compose projects have independent Docker
networks: Global host Nginx reaches their loopback-published ports, while the
CN front gateway reaches both ports on `10.0.0.10`. Host Nginx must not use
the Compose-only `locator` or `relay` DNS names.

## Persistent CN service-host firewall

The CN host-published relay and locator ports require the checked-in
`DOCKER-USER` boundary. Its private inputs are deployment inventory and must
not be committed. Copy `cn-docker-user-firewall.env.example` to a root-owned,
non-symlink inventory path under root-owned, non-writable directories, replace
the RFC 5737 examples, and keep the file exactly `root:root` mode `0600`:

```sh
sudo install -d -o root -g root -m 0700 /etc/openpencil/inventory
sudo install -o root -g root -m 0600 \
  deploy/collab-relay-locator/cn-docker-user-firewall.env.example \
  /etc/openpencil/inventory/collab-cn-firewall.env
sudoedit /etc/openpencil/inventory/collab-cn-firewall.env
sudo deploy/collab-relay-locator/install-cn-docker-user-firewall.sh \
  /etc/openpencil/inventory/collab-cn-firewall.env
```

The inventory accepts exactly three unquoted data values: the existing Linux
ingress interface, the one gateway source IPv4, and the service host's
original-destination IPv4. Hostnames, CIDRs, shell syntax, leading-zero IPv4
octets, unknown keys, duplicates, symlinks, non-root ownership, and permissive
modes fail closed. The inventory is parsed as data and is never sourced.
Installation also requires that the service IPv4 exactly equals the single
numeric `host_ip` in both immutable CN production overlays: relay TCP `8091`
and locator TCP `8092`. The derived value is retained separately as a
root-owned mode `0400` deployment binding. Every apply and verify then requires
that exact IPv4 to be assigned exactly once on the configured live interface.

The installer atomically replaces `/etc/openpencil/collab-cn-firewall.env`,
installs immutable root-owned helpers, enables a pre-Docker systemd gate, and
adds a Docker drop-in that reapplies and verifies the rules after every daemon
start. When Docker is already active, installation calls the immutable apply
and verify helpers directly and proves its active state and main PID did not
change; it does not start, stop, or restart either unit. During the next
maintenance restart, require both units to remain healthy:

```sh
sudo systemctl restart docker.service
sudo systemctl status --no-pager \
  openpencil-collab-cn-firewall.service docker.service
sudo /usr/local/libexec/openpencil-collab-cn-firewall/verify-cn-docker-user-firewall.sh
```

The atomic `iptables-restore --noflush` transaction places one dedicated chain
at the start of `DOCKER-USER`. For each original-destination TCP port `8091`
and `8092`, it returns only traffic arriving on the configured interface from
the configured source IPv4, then drops every other source to that exact
original destination. The final rule returns all unrelated traffic, so ports
`18080`, `18770`, SSH, and every unrelated host rule remain authoritative and
untouched. The read-only verifier checks the exact rules, their order and
count from one filter-table snapshot, the first-position jumps, the strict
inventory, the immutable Compose binding, and the live interface assignment
without changing kernel state. Apply refuses to flush a pre-existing chain of
the managed name unless the canonical anchor or exact prior five-rule shape
proves ownership; any foreign jump or goto reference from `FORWARD`, `INPUT`,
or another custom chain also fails closed before mutation.

This boundary requires Docker's iptables-compatible `FORWARD` to
`DOCKER-USER` hook. A Docker native-nftables configuration that omits that
hook, a missing interface, or another firewall manager rewriting the managed
chain makes the post-start verifier fail and therefore makes Docker activation
fail. Resolve that host integration before starting the CN Compose projects;
do not bypass the unit or weaken the verifier. This package assumes exclusive
ownership of its dedicated chain. A privileged firewall reload after Docker is
already active is outside the lifecycle hooks and must either invoke the apply
and verify helpers or be prohibited by host policy.

After starting both CN Compose projects, run the read-only verifier again,
inspect the managed-chain counters, and probe both paths once from the approved
gateway and once from a disallowed source. This live acceptance check proves
that the host is actually publishing the reviewed immutable overlays; the
installer deliberately does not start containers or infer permission to
deploy them.
