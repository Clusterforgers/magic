import { createFileRoute, notFound } from '@tanstack/react-router'
import { lazy, Suspense } from 'react'
import { parseDevSupportSearch } from '@/features/cardSupport/dev/supportSearch'

// `import.meta.env.DEV` is fixed at build time, so in a production build this is `null` and the dynamic import
// (with the fixture behind it) is dead code that never reaches the bundle.
const DevSupport = import.meta.env.DEV
  ? lazy(() =>
      import('@/features/cardSupport/dev/DevSupportRoute').then((module) => ({
        default: module.DevSupportRoute,
      })),
    )
  : null

export const Route = createFileRoute('/dev/support')({
  validateSearch: parseDevSupportSearch,
  beforeLoad: () => {
    if (DevSupport === null) throw notFound()
  },
  component: () =>
    DevSupport === null ? null : (
      <Suspense fallback={null}>
        <DevSupport />
      </Suspense>
    ),
})
