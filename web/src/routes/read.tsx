import { createFileRoute, redirect } from '@tanstack/react-router'
import { parseReadSearch } from '@/features/reads/model'

/** The old address of the review screen: forwards to `/review` with the same deck, precon or `mine`. */
export const Route = createFileRoute('/read')({
  validateSearch: parseReadSearch,
  beforeLoad: ({ search }) => {
    throw redirect({ to: '/review', search, replace: true })
  },
})
