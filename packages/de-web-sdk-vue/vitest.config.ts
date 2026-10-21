import { defineConfig } from 'vitest/config';
import vue from '@vitejs/plugin-vue';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';
const here = fileURLToPath(new URL('.', import.meta.url));
export default defineConfig({
  plugins: [vue()],
  test: { environment: 'jsdom', globals: true },
  resolve: { alias: {
    '@kemalcalak/de-web-sdk': resolve(here, '../de-web-sdk/src/index.ts'),
    'virtual:de_web_sdk_wasm': resolve(here, '../de-web-sdk/test/wasm-stub.ts'),
  } },
});
