import { createFileRoute, notFound } from '@tanstack/react-router'
import { lazy, Suspense } from 'react'
import { parseDevLobbySearch } from '@/features/game/dev/lobbySearch'

// `import.meta.env.DEV` is fixed at build time, so in a production build this is `null` and the dynamic import
// (with the fixture behind it) is dead code that never reaches the bundle.
const DevLobby = import.meta.env.DEV
  ? lazy(() =>
      import('@/features/game/dev/DevLobbyRoute').then((module) => ({ default: module.DevLobbyRoute })),
    )
  : null

export const Route = createFileRoute('/dev/lobby')({
  validateSearch: parseDevLobbySearch,
  beforeLoad: () => {
    if (DevLobby === null) throw notFound()
  },
  component: () =>
    DevLobby === null ? null : (
      <Suspense fallback={null}>
        <DevLobby />
      </Suspense>
    ),
})
