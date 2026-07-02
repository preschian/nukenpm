// @ts-check
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'astro/config';

import cloudflare from '@astrojs/cloudflare';
import sitemap from '@astrojs/sitemap';

const root = dirname(fileURLToPath(import.meta.url));
const cargo = readFileSync(resolve(root, '../cli/Cargo.toml'), 'utf8');
const version = cargo.match(/^version\s*=\s*"(.+)"\s*$/m)?.[1] ?? '0.0.0';

// https://astro.build/config
export default defineConfig({
  site: 'https://nukenpm.avalix.dev',
  adapter: cloudflare(),
  integrations: [sitemap()],
  vite: {
    define: {
      __NUKENPM_VERSION__: JSON.stringify(version),
    },
  },
});