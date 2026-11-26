import { createFileRoute, notFound } from '@tanstack/react-router'
import { lazy, Suspense } from 'react'
import { parseDevPlaySearch } from '@/features/browser/dev/playSearch'

// `import.meta.env.DEV` is fixed at build time, so in a production build this is `null` and the dynamic import
// (with the fixture behind it) is dead code that never reaches the bundle.
const DevPlay = import.meta.env.DEV
  ? lazy(() =>
      import('@/features/browser/dev/DevPlayRoute').then((module) => ({
        default: module.DevPlayRoute,
      })),
    )
  : null

export const Route = createFileRoute('/dev/play')({
  validateSearch: parseDevPlaySearch,
  beforeLoad: () => {
    if (DevPlay === null) throw notFound()
  },
  component: () =>
    DevPlay === null ? null : (
      <Suspense fallback={null}>
        <DevPlay />
      </Suspense>
    ),
})
