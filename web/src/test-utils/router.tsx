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
import { PlayStub } from './PlayStub'

/** Options for `renderInRouter`. */
export interface RouterOptions {
  /** Where the memory history starts; `/` by default. */
  url?: string
  /** Wrap everything in `StrictMode`, which mounts, unmounts and mounts every effect again. */
  strict?: boolean
  /**
   * Mount `ui` at `/game/$gameId` and a stand-in for Play at `/` (a heading and the shell's notice), so a test can see
   * where the page sends the visitor. Without it `ui` is the page at `/` and `/game/$gameId` is the stub.
   */
  routed?: boolean
}

/** Renders `ui` at `/` in a real router on a memory history; `/game/$gameId` is a stub that shows where it was reached. */
export async function renderInRouter(ui: ReactElement, options: RouterOptions = {}) {
  const root = createRootRoute()
  const routed = options.routed === true
  const home = createRoute({
    getParentRoute: () => root,
    path: '/',
    component: routed ? PlayStub : () => ui,
  })
  const game = createRoute({
    getParentRoute: () => root,
    path: '/game/$gameId',
    component: routed ? () => ui : GameStub,
  })
  const router = createRouter({
    routeTree: root.addChildren([home, game]),
    history: createMemoryHistory({ initialEntries: [options.url ?? '/'] }),
  })
  const tree = <RouterProvider router={router} />
  const view = render(options.strict === true ? <StrictMode>{tree}</StrictMode> : tree)
  await router.load()
  return { ...view, router }
}
