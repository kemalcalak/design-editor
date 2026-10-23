import { describe, it, expect } from 'vitest';
import { VERSION } from '../src/index.js';
import { execFileSync } from 'node:child_process';
import { resolve } from 'node:path';

// Importing packages/scripts/workspace-version.mjs fails under the jsdom
// environment ("The URL must be of scheme file"), so read the Cargo
// workspace version through the shell reader from the package directory.
const workspaceVersion = () =>
  execFileSync('sh', [resolve(process.cwd(), '../../scripts/workspace-version.sh')], {
    encoding: 'utf8',
  }).trim();

describe('package scaffold', () => {
  it('exposes a VERSION string', () => {
    expect(typeof VERSION).toBe('string');
  });

  it('takes VERSION from the Cargo workspace version', () => {
    expect(VERSION).toBe(workspaceVersion());
  });
});
