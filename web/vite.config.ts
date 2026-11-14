import tailwindcss from '@tailwindcss/vite'
import { tanstackRouter } from '@tanstack/router-plugin/vite'
import react from '@vitejs/plugin-react'
import { readFile, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { gzipSync } from 'node:zlib'
import type { Plugin } from 'vite'
import { defineConfig } from 'vitest/config'
import { scaled } from './src/test-utils/loadFactor.ts'

// `pnpm e2e` points this at its own server (docs/web.md), so it never meets a dev server on the default port.
const gameServer = process.env.MTG_SERVER_URL ?? 'http://127.0.0.1:8080'

/** The libraries every page loads, kept in the `vendor` chunk. */
const VENDOR =
  /node_modules\/(?:\.pnpm\/[^/]+\/node_modules\/)?(?:react|react-dom|scheduler|@tanstack\/(?:react-router|router-core|history|store|react-store))\//

/** Files of the build that nginx serves precompressed (`gzip_static`, `deploy/nginx.conf`). */
const COMPRESSIBLE = /\.(?:js|css|html|svg|json)$/

/** Writes a `.gz` at the highest level beside each text file of a production build, so nginx sends it without work. */
function precompress(): Plugin {
  return {
    name: 'precompress',
    apply: 'build',
    async writeBundle(options, bundle) {
      const dir = options.dir ?? 'dist'
      const names = Object.keys(bundle).filter((name) => COMPRESSIBLE.test(name))
      await Promise.all(
        names.map(async (name) => {
          const file = path.join(dir, name)
          await writeFile(`${file}.gz`, gzipSync(await readFile(file), { level: 9 }))
        }),
      )
    },
  }
}

/** The font file of the Latin letters, which every page draws. */
const LATIN_FONT = /^assets\/geist-latin-wght-normal-[\w-]+\.woff2$/

/** Preloads the Latin font from the page, so it arrives with the scripts instead of after the first render asks for it. */
function preloadFont(): Plugin {
  return {
    name: 'preload-font',
    apply: 'build',
    transformIndexHtml: {
      order: 'post',
      handler(_html, context) {
        const font = Object.keys(context.bundle ?? {}).find((name) => LATIN_FONT.test(name))
        if (font === undefined) return []
        const attrs = { rel: 'preload', href: `/${font}`, as: 'font', type: 'font/woff2', crossorigin: '' }
        return [{ tag: 'link', attrs, injectTo: 'head' }]
      },
    },
  }
}

export default defineConfig({
  plugins: [
    tanstackRouter({ target: 'react', autoCodeSplitting: true }),
    react(),
    tailwindcss(),
    preloadFont(),
    precompress(),
  ],
  resolve: { alias: { '@': path.resolve(import.meta.dirname, 'src') } },
  // React and the router load on every page and change only with a dependency bump: in a chunk of their own its
  // name survives a release, so a returning browser keeps it cached instead of fetching it inside the new entry.
  build: {
    rolldownOptions: {
      output: { codeSplitting: { groups: [{ name: 'vendor', test: VENDOR }] } },
    },
  },
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
