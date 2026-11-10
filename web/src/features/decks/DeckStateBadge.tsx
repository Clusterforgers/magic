import { CircleCheckIcon, CircleDashedIcon, HandIcon, ShieldCheckIcon } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import type { ComponentProps } from 'react'
import { Pill } from '@/components/Pill'
import type { PillTone } from '@/components/Pill'
import type { DeckState } from '@/protocol/generated'
import { STATE_LABEL } from './format'

const LOOK: Record<DeckState, { Icon: LucideIcon; tone: PillTone }> = {
  verified: { Icon: ShieldCheckIcon, tone: 'success' },
  // The whole deck runs, some of it not Verified: the success tone with its own icon, apart from Verified's shield.
  read: { Icon: CircleCheckIcon, tone: 'success' },
  manual_play: { Icon: HandIcon, tone: 'warning' },
  // Only a check stored before legality became a warning says `blocked`; such a deck is played by hand.
  blocked: { Icon: HandIcon, tone: 'warning' },
  unchecked: { Icon: CircleDashedIcon, tone: 'neutral' },
}

interface DeckStateBadgeProps extends Omit<ComponentProps<typeof Pill>, 'tone' | 'children'> {
  state: DeckState
  /** Other words than the state's own, for a page that says more ('12 cards played by hand'). */
  label?: string
}

/**
 * The check state of a deck as a pill: an icon and a word (Verified, Whole deck runs, Played by hand, Unchecked), never
 * colour alone. Legality is apart (`LegalityPill`).
 */
export function DeckStateBadge({ state, label, ...props }: DeckStateBadgeProps) {
  const { Icon, tone } = LOOK[state]
  return (
    <Pill tone={tone} data-deck-state={state} {...props}>
      <Icon aria-hidden="true" />
      {label ?? STATE_LABEL[state]}
    </Pill>
  )
}
