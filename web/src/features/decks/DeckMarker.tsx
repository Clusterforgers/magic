import { HistoryIcon } from 'lucide-react'
import { Pill } from '@/components/Pill'
import { cn } from '@/lib/utils'
import type { DeckState } from '@/protocol/generated'
import { DeckStateBadge } from './DeckStateBadge'
import { STATE_LABEL } from './format'

/** What the marker of a deck can say: a check state, or that the check no longer applies. */
export type MarkerKind = DeckState | 'changed'

/** The words of the marker: the badge's for each state, and the note that replaces it while the draft differs from the saved deck. */
export const MARKER_LABEL: Readonly<Record<MarkerKind, string>> = {
  ...STATE_LABEL,
  changed: 'Unsaved draft',
}

/** What changed about a deck since its check. */
export interface MarkerChange {
  /** The draft in the browser differs from the saved deck. */
  dirty?: boolean
}

/**
 * Which marker a deck shows. A draft that differs from the saved deck shows
 * `changed` whatever the saved state was, so a marker never outlives the deck it was earned by; every other state is
 * the state of the check of the moment.
 */
export function markerKind(
  state: DeckState,
  { dirty = false }: MarkerChange = {},
): MarkerKind {
  if (dirty) return 'changed'
  return state
}

interface DeckMarkerProps extends MarkerChange {
  state: DeckState
  className?: string
}

/**
 * The marker beside a deck's name (docs/deck-check.md, "The marker"): the badge for a clean saved check (Verified), for
 * a deck played by hand, for a blocked one and for one never checked, and "Unsaved draft" when the draft
 * differs from the saved deck.
 */
export function DeckMarker({ state, dirty, className }: DeckMarkerProps) {
  const kind = markerKind(state, { dirty })
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
