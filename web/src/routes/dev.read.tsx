import { createFileRoute, notFound } from '@tanstack/react-router'
import { lazy, Suspense } from 'react'
import { parseDevReadSearch } from '@/features/reads/dev/readSearch'

// `import.meta.env.DEV` is fixed at build time, so in a production build this is `null` and the dynamic import
// (with the fixture behind it) is dead code that never reaches the bundle.
const DevRead = import.meta.env.DEV
  ? lazy(() =>
      import('@/features/reads/dev/DevReadRoute').then((module) => ({
        default: module.DevReadRoute,
      })),
    )
  : null

export const Route = createFileRoute('/dev/read')({
  validateSearch: parseDevReadSearch,
  beforeLoad: () => {
    if (DevRead === null) throw notFound()
  },
  component: () =>
    DevRead === null ? null : (
      <Suspense fallback={null}>
        <DevRead />
      </Suspense>
    ),
})
