import { BanIcon, CircleDashedIcon, HandIcon, ShieldCheckIcon } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import type { ComponentProps } from 'react'
import { Pill } from '@/components/Pill'
import type { PillTone } from '@/components/Pill'
import { cn } from '@/lib/utils'
import type { DeckState } from '@/protocol/generated'
import { STATE_LABEL } from './format'

// Whole class names, so Tailwind can see them. A pill has no destructive tone, so Blocked tints the neutral one.
const LOOK: Record<DeckState, { Icon: LucideIcon; tone: PillTone; className?: string }> = {
  verified: { Icon: ShieldCheckIcon, tone: 'success' },
  manual_play: { Icon: HandIcon, tone: 'warning' },
  blocked: { Icon: BanIcon, tone: 'neutral', className: 'bg-destructive/12 text-destructive' },
  unchecked: { Icon: CircleDashedIcon, tone: 'neutral' },
}

interface DeckStateBadgeProps extends Omit<ComponentProps<typeof Pill>, 'tone' | 'children'> {
  state: DeckState
}

/** The check state of a deck as a pill: an icon and a word (Verified, Manual play, Blocked, Unchecked), never colour alone. */
export function DeckStateBadge({ state, className, ...props }: DeckStateBadgeProps) {
  const { Icon, tone, className: tint } = LOOK[state]
  return (
    <Pill tone={tone} data-deck-state={state} className={cn(tint, className)} {...props}>
      <Icon aria-hidden="true" />
      {STATE_LABEL[state]}
    </Pill>
  )
}
