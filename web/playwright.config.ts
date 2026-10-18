import { defineConfig } from '@playwright/test'
import type { PlaywrightTestConfig } from '@playwright/test'
import {
  ACCOUNTS_DATABASE_URL,
  ACCOUNTS_SERVER_PORT,
  ACCOUNTS_SERVER_URL,
  ACCOUNTS_WEB_PORT,
  ACCOUNTS_WEB_URL,
  SERVER_PORT,
  SERVER_URL,
  WEB_PORT,
  WEB_URL,
} from './e2e/ports'

// Both servers are started here and are new for every run (in-memory store, built-in demo decks). Set E2E_REUSE=1 to
// keep servers that are already listening on the two ports, which saves the build while a spec is being written.
const reuseExistingServer = process.env.E2E_REUSE === '1'

// With `E2E_DATABASE_URL` set to an empty scratch database a second pair starts: a server on that database (accounts
// need Postgres) and a web preview in front of it, which `e2e/accounts.spec.ts` uses. They start after the first
// pair, so the preview finds the build the first one made. Without it nothing here touches a database.
const accountServers: Extract<PlaywrightTestConfig['webServer'], unknown[]> =
  ACCOUNTS_DATABASE_URL === undefined
    ? []
    : [
        {
          command: 'cargo run --quiet -p mtg-server',
          cwd: '..',
          env: {
            MTG_BIND: `127.0.0.1:${ACCOUNTS_SERVER_PORT}`,
            RUST_LOG: 'warn',
            DATABASE_URL: ACCOUNTS_DATABASE_URL,
          },
          url: `${ACCOUNTS_SERVER_URL}/healthz`,
          reuseExistingServer,
          gracefulShutdown: { signal: 'SIGTERM' as const, timeout: 5_000 },
          timeout: 600_000,
        },
        {
          command: `pnpm exec vite preview --host 127.0.0.1 --port ${ACCOUNTS_WEB_PORT} --strictPort`,
          env: { MTG_SERVER_URL: ACCOUNTS_SERVER_URL },
          url: ACCOUNTS_WEB_URL,
          reuseExistingServer,
          timeout: 60_000,
        },
      ]

export default defineConfig({
  testDir: './e2e',
  timeout: 60_000,
  workers: 1,
  reporter: [['list']],
  use: {
    baseURL: WEB_URL,
    viewport: { width: 1600, height: 1000 },
    actionTimeout: 15_000,
    // NixOS cannot run the browser Playwright downloads; docs/web.md says which chromium to point this at.
    launchOptions: { executablePath: process.env.CHROMIUM_PATH },
  },
  webServer: [
    {
      command: 'cargo run --quiet -p mtg-server',
      cwd: '..',
      // An empty DATABASE_URL beats the default in .cargo/config.toml: the e2e server keeps its games in memory.
      env: { MTG_BIND: `127.0.0.1:${SERVER_PORT}`, RUST_LOG: 'warn', DATABASE_URL: '' },
      url: `${SERVER_URL}/healthz`,
      reuseExistingServer,
      gracefulShutdown: { signal: 'SIGTERM', timeout: 5_000 },
      // A cold build of the workspace is slow; a warm one answers within a second.
      timeout: 600_000,
    },
    {
      command: `pnpm build && pnpm exec vite preview --host 127.0.0.1 --port ${WEB_PORT} --strictPort`,
      // The preview server keeps the browser's Host header when it proxies /api and /ws to this server.
      env: { MTG_SERVER_URL: SERVER_URL },
      url: WEB_URL,
      reuseExistingServer,
      timeout: 180_000,
    },
    ...accountServers,
  ],
})
