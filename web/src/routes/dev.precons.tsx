import { createFileRoute, notFound } from '@tanstack/react-router'
import { lazy, Suspense } from 'react'
import { parseDevPreconsSearch } from '@/features/precons/dev/preconsSearch'

// `import.meta.env.DEV` is fixed at build time, so in a production build this is `null` and the dynamic import
// (with the fixture behind it) is dead code that never reaches the bundle.
const DevPrecons = import.meta.env.DEV
  ? lazy(() =>
      import('@/features/precons/dev/DevPreconsRoute').then((module) => ({
        default: module.DevPreconsRoute,
      })),
    )
  : null

export const Route = createFileRoute('/dev/precons')({
  validateSearch: parseDevPreconsSearch,
  beforeLoad: () => {
    if (DevPrecons === null) throw notFound()
  },
  component: () =>
    DevPrecons === null ? null : (
      <Suspense fallback={null}>
        <DevPrecons />
      </Suspense>
    ),
})
