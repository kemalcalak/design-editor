import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

// The product version lives only in [workspace.package].version of the root
// Cargo.toml. SDK builds and tests read it through the same shell reader the
// release tooling uses, so there is one parser and no committed copy.
const reader = fileURLToPath(new URL('../../scripts/workspace-version.sh', import.meta.url));

export function workspaceVersion() {
  return execFileSync('sh', [reader], { encoding: 'utf8' }).trim();
}

// `define` entry that replaces the `__DE_VERSION__` global in SDK sources.
export function versionDefine() {
  return { __DE_VERSION__: JSON.stringify(workspaceVersion()) };
}
