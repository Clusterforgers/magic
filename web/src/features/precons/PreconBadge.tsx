import { DeckStateBadge } from '@/features/decks/DeckStateBadge'
import type { DeckState, PreconListItem } from '@/protocol/generated'
import { statusOf, statusWords } from './format'
import type { PreconStatus } from './format'

const AS_STATE: Record<PreconStatus, DeckState> = {
  whole: 'verified',
  hand: 'manual_play',
  blocked: 'blocked',
}

/**
 * What the game does with a precon as a pill: an icon and the words ("Whole deck runs", "3 cards played by hand",
 * "Cannot be taken"), never colour alone.
 */
export function PreconBadge({
  precon,
  id,
  className,
}: {
  precon: PreconListItem
  id?: string
  className?: string
}) {
  const status = statusOf(precon)
  return (
    <DeckStateBadge
      state={AS_STATE[status]}
      id={id}
      label={statusWords(precon)}
      data-precon-status={status}
      className={className}
    />
  )
}
