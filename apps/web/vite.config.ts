import { fileURLToPath, URL } from 'node:url'

import tailwindcss from '@tailwindcss/vite'
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import wasm from 'vite-plugin-wasm'

export default defineConfig({
  plugins: [react(), tailwindcss(), wasm()],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src/', import.meta.url)),
    },
  },
  server: {
    proxy: {
      '/api': {
        target: process.env.RIICHI_WEB_PROXY_TARGET ?? 'http://127.0.0.1:3000',
        ws: true,
      },
      '/auth': process.env.RIICHI_WEB_PROXY_TARGET ?? 'http://127.0.0.1:3000',
    },
  },
})
