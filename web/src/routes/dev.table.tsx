import { createFileRoute, notFound } from '@tanstack/react-router'
import { lazy, Suspense } from 'react'
import { parseDevSearch } from '@/features/table/devSearch'

// `import.meta.env.DEV` is fixed at build time, so in a production build this is `null` and the dynamic import
// (with the fixture behind it) is dead code that never reaches the bundle.
const DevTable = import.meta.env.DEV
  ? lazy(() =>
      import('@/features/table/dev/DevTableRoute').then((module) => ({ default: module.DevTableRoute })),
    )
  : null

export const Route = createFileRoute('/dev/table')({
  validateSearch: parseDevSearch,
  beforeLoad: () => {
    if (DevTable === null) throw notFound()
  },
  component: () =>
    DevTable === null ? null : (
      <Suspense fallback={null}>
        <DevTable />
      </Suspense>
    ),
})
