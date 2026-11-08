import { createFileRoute, notFound } from '@tanstack/react-router'
import { lazy, Suspense } from 'react'
import { parseDevAccountsSearch } from '@/features/admin/dev/accountsSearch'

// `import.meta.env.DEV` is fixed at build time, so in a production build this is `null` and the dynamic import
// (with the fixture behind it) is dead code that never reaches the bundle.
const DevAccounts = import.meta.env.DEV
  ? lazy(() =>
      import('@/features/admin/dev/DevAccountsRoute').then((module) => ({
        default: module.DevAccountsRoute,
      })),
    )
  : null

export const Route = createFileRoute('/dev/accounts')({
  validateSearch: parseDevAccountsSearch,
  beforeLoad: () => {
    if (DevAccounts === null) throw notFound()
  },
  component: () =>
    DevAccounts === null ? null : (
      <Suspense fallback={null}>
        <DevAccounts />
      </Suspense>
    ),
})
