import { createFileRoute } from '@tanstack/react-router'
import { BuilderRoute } from '@/features/builder/BuilderRoute'

// The trailing underscore keeps this route out of `decks.tsx`, which is a page of its own and has no outlet.
export const Route = createFileRoute('/decks_/$deckId')({ component: () => <BuilderRoute /> })
