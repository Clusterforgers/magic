import tailwindcss from '@tailwindcss/vite'
import { tanstackRouter } from '@tanstack/router-plugin/vite'
import react from '@vitejs/plugin-react'
import path from 'node:path'
import { defineConfig } from 'vitest/config'
import { scaled } from './src/test-utils/loadFactor.ts'

// `pnpm e2e` points this at its own server (docs/web.md), so it never meets a dev server on the default port.
const gameServer = process.env.MTG_SERVER_URL ?? 'http://127.0.0.1:8080'

export default defineConfig({
  plugins: [tanstackRouter({ target: 'react', autoCodeSplitting: true }), react(), tailwindcss()],
  resolve: { alias: { '@': path.resolve(import.meta.dirname, 'src') } },
  // `changeOrigin` stays off on purpose: the proxy then keeps the browser's Host header, and the server only
  // opens a socket whose Origin equals the Host it was asked for. `preview` reuses these rules.
  server: {
    proxy: {
      '/api': { target: gameServer, changeOrigin: false },
      '/ws': { target: gameServer, changeOrigin: false, ws: true },
    },
  },
  // The 5 s default fails the heavy table tests when several test runs share the machine; `scaled` stretches 20 s by the load.
  test: {
    environment: 'jsdom',
    setupFiles: ['./src/test-setup.ts'],
    exclude: ['e2e/**', 'node_modules/**'],
    testTimeout: scaled(20_000),
    hookTimeout: scaled(20_000),
  },
})
