import { createMemoryHistory, createRouter, RouterProvider } from '@tanstack/react-router'
import { act, render } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { StrictMode } from 'react'
import { vi } from 'vitest'
import { AuthProvider } from '@/features/auth/AuthProvider'
import { createAuthStore } from '@/features/auth/store'
import type { AuthStore } from '@/features/auth/store'
import { routeTree } from '@/routeTree.gen'
import { joinFetch, startAccounts } from './accountsServer'
import type { AccountsScript, AccountsServer } from './accountsServer'
import { startServer } from './fakeServer'
import type { FakeServer, ServerScript } from './fakeServer'

/** A browser WebSocket that only records what the page asked for. */
export class RecordingSocket {
  static opened: RecordingSocket[] = []
  readonly url: string
  closed: { code: number; reason: string } | null = null
  onopen: (() => void) | null = null
  onmessage: ((event: { data: unknown }) => void) | null = null
  onclose: ((event: { code: number; reason: string }) => void) | null = null

  constructor(url: string) {
    this.url = url
    RecordingSocket.opened.push(this)
  }

  send(): void {}

  close(code: number, reason: string): void {
    this.closed = { code, reason }
  }
}

/** What `renderApp` can be told. */
export interface AppOptions {
  /** How the fake account server behaves. */
  accounts?: AccountsScript
  /** How the fake game server (decks, creating a game, looking one up) answers. */
  rest?: ServerScript
  /** Wrap everything in `StrictMode`. */
  strict?: boolean
}

/** The app as a browser would run it, on fake servers; undo its globals with `vi.unstubAllGlobals()` afterwards. */
export interface App {
  router: ReturnType<typeof createRouter<typeof routeTree>>
  store: AuthStore
  accounts: AccountsServer
  rest: FakeServer
  /** The sockets the page opened, oldest first. */
  sockets: RecordingSocket[]
  user: ReturnType<typeof userEvent.setup>
  container: HTMLElement
  /** Lets pending promises settle. */
  settle(): Promise<void>
}

/**
 * Renders the real route tree (header, pages, sign-in store) at `url` on a memory history. `fetch` is answered by an
 * in-memory account server for `/api/auth/*` and `/api/me/*` and by the game server fake for the rest, and `WebSocket`
 * only records its addresses.
 */
export async function renderApp(url: string, options: AppOptions = {}): Promise<App> {
  const accounts = startAccounts(options.accounts)
  const rest = startServer(options.rest)
  RecordingSocket.opened = []
  vi.stubGlobal('fetch', joinFetch(accounts.fetch, rest.fetch))
  vi.stubGlobal('WebSocket', RecordingSocket)

  const store = createAuthStore({ api: accounts.api })
  const router = createRouter({ routeTree, history: createMemoryHistory({ initialEntries: [url] }) })
  const tree = (
    <AuthProvider store={store}>
      <RouterProvider router={router} />
    </AuthProvider>
  )
  const view = render(options.strict === true ? <StrictMode>{tree}</StrictMode> : tree)
  const settle = async (): Promise<void> => {
    await act(async () => {
      await Promise.resolve()
    })
  }
  await router.load()
  await settle()
  return {
    router,
    store,
    accounts,
    rest,
    sockets: RecordingSocket.opened,
    user: userEvent.setup(),
    container: view.container,
    settle,
  }
}
