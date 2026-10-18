import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider,
} from '@tanstack/react-router'
import { render } from '@testing-library/react'
import { StrictMode } from 'react'
import type { ReactElement } from 'react'
import { GameStub } from './GameStub'

/** Options for `renderInRouter`. */
export interface RouterOptions {
  /** Where the memory history starts; `/` by default. */
  url?: string
  /** Wrap everything in `StrictMode`, which mounts, unmounts and mounts every effect again. */
  strict?: boolean
}

/** Renders `ui` at `/` in a real router on a memory history; `/game/$gameId` is a stub that shows where it was reached. */
export async function renderInRouter(ui: ReactElement, options: RouterOptions = {}) {
  const root = createRootRoute()
  const home = createRoute({ getParentRoute: () => root, path: '/', component: () => ui })
  const game = createRoute({ getParentRoute: () => root, path: '/game/$gameId', component: GameStub })
  const router = createRouter({
    routeTree: root.addChildren([home, game]),
    history: createMemoryHistory({ initialEntries: [options.url ?? '/'] }),
  })
  const tree = <RouterProvider router={router} />
  const view = render(options.strict === true ? <StrictMode>{tree}</StrictMode> : tree)
  await router.load()
  return { ...view, router }
}
