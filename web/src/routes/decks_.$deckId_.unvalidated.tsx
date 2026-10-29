import { createFileRoute } from '@tanstack/react-router'
import { UnvalidatedDeckRoute } from '@/features/decks/unvalidated/UnvalidatedDeckRoute'

// Trailing underscores keep this route out of `decks.tsx` and `decks_.$deckId.tsx`, which have no outlet.
export const Route = createFileRoute('/decks_/$deckId_/unvalidated')({ component: UnvalidatedDeckRoute })
