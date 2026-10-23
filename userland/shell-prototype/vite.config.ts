import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { fileURLToPath } from 'node:url';

const repo = fileURLToPath(new URL('../..', import.meta.url));

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      // Wallpapers and logo come straight from design/, never copied.
      '@design': fileURLToPath(new URL('../../design', import.meta.url)),
      '@theme': fileURLToPath(new URL('../theme', import.meta.url)),
    },
  },
  server: { fs: { allow: [repo] } },
  build: { target: 'es2022', assetsInlineLimit: 0 },
});
