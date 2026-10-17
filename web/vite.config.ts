import tailwindcss from '@tailwindcss/vite'
import { tanstackRouter } from '@tanstack/router-plugin/vite'
import react from '@vitejs/plugin-react'
import path from 'node:path'
import { defineConfig } from 'vitest/config'

const gameServer = 'http://127.0.0.1:8080'

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
  test: { environment: 'jsdom', setupFiles: ['./src/test-setup.ts'], exclude: ['e2e/**', 'node_modules/**'] },
})
