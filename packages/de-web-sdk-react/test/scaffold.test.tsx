import { describe, it, expect } from 'vitest';
import { VERSION } from '../src/index.js';
import { execFileSync } from 'node:child_process';
import { resolve } from 'node:path';

// jsdom tests have no file: import.meta.url, so read the Cargo workspace
// version through the shell reader relative to the package directory.
const workspaceVersion = () =>
  execFileSync('sh', [resolve(process.cwd(), '../../scripts/workspace-version.sh')], {
    encoding: 'utf8',
  }).trim();
describe('react adapter scaffold', () => {
  it('exposes VERSION', () => { expect(typeof VERSION).toBe('string'); });
  it('takes VERSION from the Cargo workspace version', () => { expect(VERSION).toBe(workspaceVersion()); });
});
