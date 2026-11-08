import { createFileRoute } from '@tanstack/react-router'
import { parseReadSearch, targetOf } from '@/features/reads/model'
import { ReadPage } from '@/features/reads/ReadPage'

function ReadRoute() {
  const search = Route.useSearch()
  return <ReadPage target={targetOf(search)} />
}

export const Route = createFileRoute('/read')({ validateSearch: parseReadSearch, component: ReadRoute })
