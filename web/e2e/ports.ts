/** Where `pnpm e2e` puts its servers: away from the defaults (web 5173, server 8080) so a dev stack can run beside it. */
export const WEB_PORT = Number(process.env.E2E_WEB_PORT ?? 5301)
export const SERVER_PORT = Number(process.env.E2E_SERVER_PORT ?? 8301)

export const WEB_URL = `http://127.0.0.1:${WEB_PORT}`
export const SERVER_URL = `http://127.0.0.1:${SERVER_PORT}`

/**
 * An empty scratch Postgres database for the account spec, from `E2E_DATABASE_URL`. Only when it is set does the
 * config start a second server on it (the server migrates it on boot) and a second web preview in front of that;
 * without it `pnpm e2e` stays hermetic, with no database at all, and the account spec is skipped.
 */
export const ACCOUNTS_DATABASE_URL: string | undefined =
  process.env.E2E_DATABASE_URL === '' ? undefined : process.env.E2E_DATABASE_URL

/** The second pair of ports, used only with `E2E_DATABASE_URL`. */
export const ACCOUNTS_WEB_PORT = Number(process.env.E2E_ACCOUNTS_WEB_PORT ?? 5302)
export const ACCOUNTS_SERVER_PORT = Number(process.env.E2E_ACCOUNTS_SERVER_PORT ?? 8302)

export const ACCOUNTS_WEB_URL = `http://127.0.0.1:${ACCOUNTS_WEB_PORT}`
export const ACCOUNTS_SERVER_URL = `http://127.0.0.1:${ACCOUNTS_SERVER_PORT}`
