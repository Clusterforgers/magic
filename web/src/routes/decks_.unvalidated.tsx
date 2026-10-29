import { createFileRoute } from '@tanstack/react-router'
import { UnvalidatedPage } from '@/features/decks/unvalidated/UnvalidatedPage'

// The trailing underscore keeps this route out of `decks.tsx`, which has no outlet.
export const Route = createFileRoute('/decks_/unvalidated')({ component: () => <UnvalidatedPage /> })
