import { createFileRoute, notFound } from '@tanstack/react-router'
import { lazy, Suspense } from 'react'
import { parseDevUnvalidatedSearch } from '@/features/decks/unvalidated/dev/unvalidatedSearch'

// `import.meta.env.DEV` is fixed at build time, so in a production build this is `null` and the dynamic import
// (with the fixture behind it) is dead code that never reaches the bundle.
const DevUnvalidated = import.meta.env.DEV
  ? lazy(() =>
      import('@/features/decks/unvalidated/dev/DevUnvalidatedRoute').then((module) => ({
        default: module.DevUnvalidatedRoute,
      })),
    )
  : null

export const Route = createFileRoute('/dev/unvalidated')({
  validateSearch: parseDevUnvalidatedSearch,
  beforeLoad: () => {
    if (DevUnvalidated === null) throw notFound()
  },
  component: () =>
    DevUnvalidated === null ? null : (
      <Suspense fallback={null}>
        <DevUnvalidated />
      </Suspense>
    ),
})
