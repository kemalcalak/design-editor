#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

# Every check below matches with ripgrep (multiline + PCRE2 lookahead, which
# grep cannot replicate portably). Without this guard a missing `rg` makes each
# call fail as "command not found" and the script reports a policy VIOLATION
# that is not there — which is exactly how it read on CI from 215100234 until
# the runners were given ripgrep. A missing tool must say so.
if ! command -v rg >/dev/null 2>&1; then
  echo "error: ripgrep (rg) is required by this policy check but is not installed" >&2
  exit 127
fi

scan_roots=(crates vendor/anthropic-agent-sdk)

if rg -n 'reqwest::(blocking::)?Client::(new|default)\(\)' "${scan_roots[@]}" -g '*.rs'; then
  echo "error: reqwest Client::new/default would inherit SDK-native TLS; use an explicit rustls builder" >&2
  exit 1
fi

if rg -U -n --pcre2 \
  'reqwest::(blocking::)?Client::builder\(\)(?!\s*\.use_rustls_tls)' \
  "${scan_roots[@]}" -g '*.rs'; then
  echo "error: reqwest builder must call use_rustls_tls() explicitly" >&2
  exit 1
fi

if rg -n 'connect_async(_with_config)?\(' crates/de-acp/src/client.rs; then
  echo "error: WebSocket clients must pass an explicit rustls Connector" >&2
  exit 1
fi

for file in crates/de-acp/src/client.rs; do
  if ! rg -q 'connect_async_tls_with_config' "$file"; then
    echo "error: $file is missing the explicit TLS connector path" >&2
    exit 1
  fi
done

# Public HTTPS dials (user-configured AI endpoints, avatars) must bypass
# ambient proxies and pin the DNS answer they validated, so a hostile resolver
# or proxy cannot redirect a credentialed request.
for literal in '.no_proxy()' '.resolve_to_addrs'; do
  if ! rg -q -F "$literal" crates/de-chat-agent/src/provider_dial.rs; then
    echo "error: crates/de-chat-agent/src/provider_dial.rs must keep $literal" >&2
    exit 1
  fi
done

echo "rustls client policy: ok"
