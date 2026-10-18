/** Where `pnpm e2e` puts its servers: away from the defaults (web 5173, server 8080) so a dev stack can run beside it. */
export const WEB_PORT = Number(process.env.E2E_WEB_PORT ?? 5301)
export const SERVER_PORT = Number(process.env.E2E_SERVER_PORT ?? 8301)

export const WEB_URL = `http://127.0.0.1:${WEB_PORT}`
export const SERVER_URL = `http://127.0.0.1:${SERVER_PORT}`
