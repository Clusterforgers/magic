import { createFileRoute, notFound } from '@tanstack/react-router'
import { lazy, Suspense } from 'react'

// In a production build this is `null`, so the fixture behind the dynamic import never reaches the bundle.
const DevBulkRead = import.meta.env.DEV
  ? lazy(() =>
      import('@/features/bulk-reads/dev/DevBulkReadRoute').then((module) => ({
        default: module.DevBulkReadRoute,
      })),
    )
  : null

export const Route = createFileRoute('/dev/read-bulk')({
  beforeLoad: () => {
    if (DevBulkRead === null) throw notFound()
  },
  component: () =>
    DevBulkRead === null ? null : (
      <Suspense fallback={null}>
        <DevBulkRead />
      </Suspense>
    ),
})
