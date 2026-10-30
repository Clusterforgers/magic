import { BanIcon, CircleDashedIcon, HandIcon, ShieldCheckIcon } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import type { ComponentProps } from 'react'
import { Pill } from '@/components/Pill'
import type { PillTone } from '@/components/Pill'
import { cn } from '@/lib/utils'
import type { DeckState } from '@/protocol/generated'
import { BLOCKED_MEANING, STATE_LABEL } from './format'

// Whole class names, so Tailwind can see them. A pill has no destructive tone, so Blocked tints the neutral one.
const LOOK: Record<DeckState, { Icon: LucideIcon; tone: PillTone; className?: string }> = {
  verified: { Icon: ShieldCheckIcon, tone: 'success' },
  manual_play: { Icon: HandIcon, tone: 'warning' },
  blocked: { Icon: BanIcon, tone: 'neutral', className: 'bg-destructive/12 text-destructive' },
  unchecked: { Icon: CircleDashedIcon, tone: 'neutral' },
}

interface DeckStateBadgeProps extends Omit<ComponentProps<typeof Pill>, 'tone' | 'children'> {
  state: DeckState
  /** Other words than the state's own, for a page that says more ('12 cards played by hand'). */
  label?: string
}

/**
 * The check state of a deck as a pill: an icon and a word (Verified, Played by hand, Blocked, Unchecked), never colour
 * alone. A blocked deck's pill carries what that means as its description.
 */
export function DeckStateBadge({ state, label, className, ...props }: DeckStateBadgeProps) {
  const { Icon, tone, className: tint } = LOOK[state]
  return (
    <Pill
      tone={tone}
      data-deck-state={state}
      title={state === 'blocked' ? BLOCKED_MEANING : undefined}
      className={cn(tint, className)}
      {...props}
    >
      <Icon aria-hidden="true" />
      {label ?? STATE_LABEL[state]}
    </Pill>
  )
}
