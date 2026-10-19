import { HistoryIcon } from 'lucide-react'
import { Pill } from '@/components/Pill'
import { cn } from '@/lib/utils'
import type { DeckState } from '@/protocol/generated'
import { DeckStateBadge } from './DeckStateBadge'
import { STATE_LABEL } from './format'

/** What the marker of a deck can say: a check state, or that the check no longer applies. */
export type MarkerKind = DeckState | 'changed'

/** The words of the marker: the badge's for each state, and the note that replaces it once the deck has changed. */
export const MARKER_LABEL: Readonly<Record<MarkerKind, string>> = {
  ...STATE_LABEL,
  changed: 'Changed since the last check',
}

/** What changed about a deck since its check. */
export interface MarkerChange {
  /** The server says a check is stored but the deck, the rules of its cards or the engine changed since it. */
  changed?: boolean
  /** The draft in the browser differs from the saved deck. */
  dirty?: boolean
}

/**
 * Which marker a deck shows. A draft that differs from the saved deck, or a stored check that is stale, shows
 * `changed` whatever the saved state was, so a marker never outlives the deck it was earned by; a deck that was never
 * checked shows `unchecked`; every other state shows itself.
 */
export function markerKind(
  state: DeckState,
  { changed = false, dirty = false }: MarkerChange = {},
): MarkerKind {
  if (dirty) return 'changed'
  if (state === 'unchecked' && changed) return 'changed'
  return state
}

interface DeckMarkerProps extends MarkerChange {
  state: DeckState
  className?: string
}

/**
 * The marker beside a deck's name (docs/deck-check.md, "The marker"): the badge for a clean saved check (Verified), for
 * a deck played by hand, for a blocked one and for one never checked, and "Changed since the last check" when the draft
 * differs from the saved deck or the stored check is stale.
 */
export function DeckMarker({ state, changed, dirty, className }: DeckMarkerProps) {
  const kind = markerKind(state, { changed, dirty })
  if (kind !== 'changed') return <DeckStateBadge state={kind} data-marker={kind} className={className} />
  return (
    <Pill
      tone="neutral"
      data-deck-state="unchecked"
      data-marker="changed"
      className={cn('h-auto min-h-6 whitespace-normal', className)}
    >
      <HistoryIcon aria-hidden="true" />
      {MARKER_LABEL.changed}
    </Pill>
  )
}
