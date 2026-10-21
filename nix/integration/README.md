# Design Editor Nix integration surface

The Design Editor flake exports a versioned Pkl contract at
`nix/integration/DesignEditor.pkl` and the generated Nix sidecar at
`nix/integration/design-editor.nix` (`lib.integrationManifest`). Downstream flakes
should consume the sidecar rather than duplicating executable names, MCP
transport arguments, or harness config paths.

For each supported system the contract names these package outputs:

- `runtime`: source-built desktop and CLI runtime;
- `runtime-prebuilt`: the matching release binaries, wrapped so `de` can find
  the desktop executable (exports both `design-editor` and `de`);
- `web-server`: the source-built `de-host-web-server` plus its web bundle;
- `skills`: an immutable Skillnet bundle containing `Skillnet.pkl` and the
  Design Editor design skill.

The default MCP adapter is per-session stdio (`--mcp <document>`). The HTTP
adapter is loopback-only and is intended for an explicitly enabled live-canvas
service (`--live-mcp <port>`). Consumers should merge the generated adapter
entry into their harness config atomically and preserve unrelated keys.
