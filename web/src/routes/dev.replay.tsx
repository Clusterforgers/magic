import { createFileRoute, notFound } from '@tanstack/react-router'
import { lazy, Suspense } from 'react'
import { parseDevReplaySearch } from '@/features/replay/dev/replaySearch'

// `import.meta.env.DEV` is fixed at build time, so in a production build this is `null` and the dynamic import
// (with the fixture behind it) is dead code that never reaches the bundle.
const DevReplay = import.meta.env.DEV
  ? lazy(() =>
      import('@/features/replay/dev/DevReplayRoute').then((module) => ({
        default: module.DevReplayRoute,
      })),
    )
  : null

export const Route = createFileRoute('/dev/replay')({
  validateSearch: parseDevReplaySearch,
  beforeLoad: () => {
    if (DevReplay === null) throw notFound()
  },
  component: () =>
    DevReplay === null ? null : (
      <Suspense fallback={null}>
        <DevReplay />
      </Suspense>
    ),
})
