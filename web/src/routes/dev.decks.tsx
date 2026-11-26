import { createFileRoute, notFound } from '@tanstack/react-router'
import { lazy, Suspense } from 'react'
import { parseDevDecksSearch } from '@/features/decks/dev/decksSearch'

// `import.meta.env.DEV` is fixed at build time, so in a production build this is `null` and the dynamic import
// (with the fixture behind it) is dead code that never reaches the bundle.
const DevDecks = import.meta.env.DEV
  ? lazy(() =>
      import('@/features/decks/dev/DevDecksRoute').then((module) => ({
        default: module.DevDecksRoute,
      })),
    )
  : null

export const Route = createFileRoute('/dev/decks')({
  validateSearch: parseDevDecksSearch,
  beforeLoad: () => {
    if (DevDecks === null) throw notFound()
  },
  component: () =>
    DevDecks === null ? null : (
      <Suspense fallback={null}>
        <DevDecks />
      </Suspense>
    ),
})
