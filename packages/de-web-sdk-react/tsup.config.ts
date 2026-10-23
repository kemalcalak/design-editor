import { defineConfig } from 'tsup';
import { versionDefine } from '../scripts/workspace-version.mjs';
export default defineConfig({ entry: ['src/index.ts'], format: ['esm', 'cjs'], dts: true, clean: true, sourcemap: true, define: versionDefine(), external: ['react', 'react-dom', '@kemalcalak/de-web-sdk'] });
