import { useMemo } from 'react'
import { mergeDecks } from '@/features/decks/unvalidated/model'
import type { FlaggedCard } from '@/features/decks/unvalidated/model'
import { useUnvalidated } from '@/features/decks/unvalidated/useUnvalidated'
import type { Scope } from '@/features/decks/unvalidated/useUnvalidated'
import { flaggedCards } from '@/features/precons/format'
import { usePreconDetail } from '@/features/precons/usePreconDetail'
import type { ReadTarget } from '@/lib/toRead'
import type { DecksApi } from '@/net/decks'
import type { PreconsApi } from '@/net/precons'

/** The cards to check of a review, as the page sees them; a failure leaves the cards to read on their own. */
export type CheckCards =
  | { status: 'loading' }
  | { status: 'ready'; cards: readonly FlaggedCard[] }
  | { status: 'failed'; message: string }

/** The endpoints the cards to check come from. */
export interface CheckApis {
  decks: Pick<DecksApi, 'list' | 'get' | 'check'>
  precons: Pick<PreconsApi, 'get'>
}

function scopeOf(target: ReadTarget | null): Scope {
  if (target === null || 'precon' in target) return { kind: 'none' }
  return 'mine' in target ? { kind: 'all' } : { kind: 'deck', id: target.deck }
}

/**
 * The cards of the review's target that the game plays partly or wholly by hand (docs/web.md, Reviewing cards): a
 * deck's report, every deck's reports merged by card, or a precon's cards; none for every card.
 */
export function useCheckCards(apis: CheckApis, target: ReadTarget | null): CheckCards {
  const { state } = useUnvalidated(apis.decks, scopeOf(target))
  const precon = target !== null && 'precon' in target ? target.precon : null
  const { state: detail } = usePreconDetail(apis.precons, precon)
  return useMemo((): CheckCards => {
    if (precon !== null) {
      if (detail.status === 'ready') return { status: 'ready', cards: flaggedCards(detail.view) }
      return detail.status === 'failed'
        ? { status: 'failed', message: detail.message }
        : { status: 'loading' }
    }
    switch (state.status) {
      case 'ready':
        return { status: 'ready', cards: mergeDecks(state.decks) }
      case 'loading':
        return { status: 'loading' }
      case 'unavailable':
        return { status: 'failed', message: 'This server keeps no decks.' }
      default:
        return { status: 'failed', message: state.message }
    }
  }, [precon, detail, state])
}
