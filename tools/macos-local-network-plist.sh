#!/bin/sh
# Canonical macOS local-network privacy metadata for Design Editor bundles.
#
# Keep the user-facing explanation free of account, document, device, or
# hostname data. Both bundle implementations source this file so their final
# Info.plist values cannot drift.

DESIGN_EDITOR_LOCAL_NETWORK_USAGE_DESCRIPTION='Design Editor uses your local network to discover and connect to collaboration sessions you start or join.'
DESIGN_EDITOR_BONJOUR_SERVICE='_design-editor-collab._tcp'

design_editor_apply_macos_local_network_plist() (
  plist_path=$1

  /usr/libexec/PlistBuddy \
    -c "Delete :NSLocalNetworkUsageDescription" \
    "$plist_path" 2>/dev/null || true
  /usr/libexec/PlistBuddy \
    -c "Add :NSLocalNetworkUsageDescription string $DESIGN_EDITOR_LOCAL_NETWORK_USAGE_DESCRIPTION" \
    "$plist_path"

  /usr/libexec/PlistBuddy \
    -c "Delete :NSBonjourServices" \
    "$plist_path" 2>/dev/null || true
  /usr/libexec/PlistBuddy \
    -c "Add :NSBonjourServices array" \
    -c "Add :NSBonjourServices:0 string $DESIGN_EDITOR_BONJOUR_SERVICE" \
    "$plist_path"
)
