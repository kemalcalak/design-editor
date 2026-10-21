# OpenPencil locator SoftHSM signer

This package is the production OPLS Unix-socket adapter for the relay locator.
It is a separate non-root container with no network namespace. Only this
container receives the SoftHSM token directory and user PIN. The locator sees
only `signer.sock` read-only; it cannot read the token store, PIN, or private
key.

The adapter accepts exactly one 339-byte OPLS v1 request per connection,
authenticates the locator UID/GID with Unix peer credentials, requires the
configured active key id, and rejects bytes that are not a canonical locator
for the configured region. This confines the dedicated Ed25519 key to the
locator protocol without changing the existing 268-byte signature input. Each
accepted request uses single-part PKCS#11 EdDSA signing and the returned
signature is verified against the token public key before release.

## Key profile

Configuration contains exactly two public entries: active and next. Object ids
and labels must be unique. Provisioning refuses to overwrite either. Startup
requires one public and one private object for each entry and verifies, among
other attributes:

- `CKK_EC_EDWARDS`, Ed25519 parameters, and locally generated key material;
- private `CKA_SENSITIVE=true`, `CKA_EXTRACTABLE=false`,
  `CKA_ALWAYS_SENSITIVE=true`, and `CKA_NEVER_EXTRACTABLE=true`;
- private sign-only and public verify-only use with the Ed25519 key type;
- token persistence, exact labels/object ids, and no duplicate pair objects.

This deployment profile intentionally limits public key ids and token labels
to ASCII letters, digits, `-`, `_`, and `.`. Migrate any older printable wire
ids containing other punctuation before using this adapter.

Neither `initialize` nor `provision` exports private bytes. `public` emits the
two public Ed25519 records accepted by the relay key-file parser. The signed
desktop bootstrap uses the same base64url public value under its `x` field.

## Host preparation

Use separate directories, token labels, key ids, and object ids in CN and
Global. Concrete public hosts and generated key ids belong in the private
deployment inventory, not in this repository. The numeric identities used by
the checked-in images are locator UID/GID `65532:65532` and signer
`65533:65532`.

Prepare external host paths on each regional Linux host:

```sh
sudo install -d -o 65533 -g 65532 -m 0700 /secure/openpencil/locator-hsm/tokens
sudo install -o 65533 -g 65532 -m 0400 /dev/null /secure/openpencil/locator-hsm/user-pin
sudo install -o 65533 -g 65532 -m 0400 /dev/null /secure/openpencil/locator-hsm/so-pin
sudo install -o root -g 65532 -m 0440 \
  deploy/collab-relay-locator-hsm/config.example.json \
  /secure/openpencil/locator-hsm/config.json
sudo install -o root -g root -m 0644 \
  deploy/collab-relay-locator-hsm/openpencil-locator-hsm.conf \
  /etc/tmpfiles.d/openpencil-locator-hsm.conf
sudo systemd-tmpfiles --create /etc/tmpfiles.d/openpencil-locator-hsm.conf
```

Replace the example region, token label, active/next public key ids, and unique
object ids. Populate both PIN files through the operator secret channel; do not
put a PIN on a command line, in Compose environment, or in shell history.
Docker file-backed mounts preserve host ownership rather than secret `uid` or
`mode` declarations, so verify these numeric modes on the target host.
The checked-in `tmpfiles.d` rule recreates the volatile `/run` socket directory
as `root:65532` mode `0770` after every boot. Install it before Compose; never
let Docker auto-create a missing bind source as `root:root` mode `0755`.

Set the Compose inputs:

```sh
export OPENPENCIL_COLLAB_HSM_CONFIG_HOST_FILE=/secure/openpencil/locator-hsm/config.json
export OPENPENCIL_COLLAB_HSM_PIN_HOST_FILE=/secure/openpencil/locator-hsm/user-pin
export OPENPENCIL_COLLAB_HSM_TOKEN_HOST_DIR=/secure/openpencil/locator-hsm/tokens
export OPENPENCIL_COLLAB_HSM_SOCKET_HOST_DIR=/run/openpencil/locator-hsm
```

## Initialize and provision

Build the image, then initialize the empty regional token. The SO PIN is an
extra one-shot mount and is not part of the running service:

```sh
docker compose \
  -f deploy/collab-relay-locator-hsm/compose.yaml \
  build locator-hsm

docker compose \
  -f deploy/collab-relay-locator-hsm/compose.yaml \
  run --rm \
  -v /secure/openpencil/locator-hsm/so-pin:/run/secrets/locator-hsm-so-pin:ro \
  locator-hsm initialize --config /run/openpencil-config/locator-hsm.json \
  --so-pin-file /run/secrets/locator-hsm-so-pin
```

Provision the two configured key ids one at a time. This makes a partial
failure explicit and never silently reuses or replaces an existing object:

```sh
docker compose -f deploy/collab-relay-locator-hsm/compose.yaml \
  run --rm locator-hsm provision \
  --config /run/openpencil-config/locator-hsm.json --kid replace-with-active-public-kid

docker compose -f deploy/collab-relay-locator-hsm/compose.yaml \
  run --rm locator-hsm provision \
  --config /run/openpencil-config/locator-hsm.json --kid replace-with-next-public-kid
```

Export public records to a candidate file, validate it, then distribute that
same active+next set to the relay and signed desktop bootstrap before starting
issuance:

```sh
docker compose -f deploy/collab-relay-locator-hsm/compose.yaml \
  run --rm locator-hsm public \
  --config /run/openpencil-config/locator-hsm.json > /secure/public/locator-keys.candidate.json
```

Remove the SO PIN mount/file from routine operations after its recovery copy
has been placed in the operator-controlled secret system.

## Start, readiness, and rotation

Start the signer and locator together only after the public policy and key
files are ready:

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
  up --build -d
```

That command is for Global. On CN, replace only the regional overlay with
`compose.production.cn.yaml`. These overlays hard-code the matching home
region and host bind; there is no host-bind variable to override.

Startup and `check` validate both key pairs and perform a locator-shaped HSM
sign-and-software-verify canary with each private key. The container health
check additionally requires the Unix socket and a fresh heartbeat written by
the main serving loop. The locator waits for a healthy signer.

For rotation from A(active)+B(next), first publish A+B everywhere. Prepare a
candidate config with B(active)+C(next), mount it into a one-shot signer,
provision C, and export the B+C candidate. Before promoting, merge the previous
and candidate public files into an A+B+C overlap verifier bundle; the relay
accepts up to 64 pinned keys. For example:

```sh
jq -s '{version: 1, keys: ([.[].keys[]] | unique_by(.kid))}' \
  /secure/public/locator-keys.current.json \
  /secure/public/locator-keys.candidate.json \
  > /secure/public/locator-keys.overlap.json
```

Distribute A+B+C to the relay and signed desktop bootstrap, atomically install
the B+C signer config, and restart signer plus locator. Retain A in verifier
bundles for the maximum locator lifetime and rollback window; only then publish
B+C alone. The signer's `public` command emits its configured pair and does not
perform this verifier-history merge. This adapter intentionally has no
key-destruction command; retirement is a separate audited token-admin
operation.

For every start or promotion,
`OPENPENCIL_COLLAB_LOCATOR_HSM_KEY_ID` on the locator must exactly equal the
signer config's `active_kid`. Exercise one authenticated locator issuance and
verify the returned signature against the newly distributed public key before
opening traffic.

## Verification

The Docker test target runs the integration test against a real SoftHSM module:

```sh
docker build --target test \
  -f deploy/collab-relay-locator-hsm/Dockerfile .
docker build -f deploy/collab-relay-locator-hsm/Dockerfile .
```
