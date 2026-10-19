import { createFileRoute, notFound } from '@tanstack/react-router'
import { lazy, Suspense } from 'react'
import { parseDevBuilderSearch } from '@/features/builder/dev/builderSearch'

// `import.meta.env.DEV` is fixed at build time, so in a production build this is `null` and the dynamic import
// (with the fixture behind it) is dead code that never reaches the bundle.
const DevBuilder = import.meta.env.DEV
  ? lazy(() =>
      import('@/features/builder/dev/DevBuilderRoute').then((module) => ({
        default: module.DevBuilderRoute,
      })),
    )
  : null

export const Route = createFileRoute('/dev/builder')({
  validateSearch: parseDevBuilderSearch,
  beforeLoad: () => {
    if (DevBuilder === null) throw notFound()
  },
  component: () =>
    DevBuilder === null ? null : (
      <Suspense fallback={null}>
        <DevBuilder />
      </Suspense>
    ),
})
