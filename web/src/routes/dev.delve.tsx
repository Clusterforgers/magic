import { createFileRoute, notFound } from '@tanstack/react-router'
import { lazy, Suspense } from 'react'

// Dead code in a production build, as `/dev/lobby` is: the fixture never reaches the bundle.
const DevDelve = import.meta.env.DEV
  ? lazy(() =>
      import('@/features/delves/dev/DevDelveRoute').then((module) => ({ default: module.DevDelveRoute })),
    )
  : null

export const Route = createFileRoute('/dev/delve')({
  validateSearch: (search: Record<string, unknown>): { state?: string } =>
    typeof search.state === 'string' ? { state: search.state } : {},
  beforeLoad: () => {
    if (DevDelve === null) throw notFound()
  },
  component: () =>
    DevDelve === null ? null : (
      <Suspense fallback={null}>
        <DevDelve />
      </Suspense>
    ),
})
