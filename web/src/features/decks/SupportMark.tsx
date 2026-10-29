import { BanIcon, ClockAlertIcon, HandIcon, ShieldCheckIcon, ShieldHalfIcon } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import { Pill } from '@/components/Pill'
import { cn } from '@/lib/utils'
import type { Support } from '@/protocol/generated'
import { SUPPORT_LABEL } from './format'

// Whole class names, so Tailwind can see them. `mark` is the pill: the state's text, a 12 % tint and a hairline edge in
// its colour (tokens.test.ts holds the contrast). `edge` is the left edge of the line that carries the mark and
// `decoration` the colour of the underline under the clauses a card preview leaves to the table.
const LOOK: Record<
  Support,
  { Icon: LucideIcon; mark: string; edge: string; decoration: string; ring: string }
> = {
  verified: {
    Icon: ShieldCheckIcon,
    mark: 'border-support-verified bg-support-verified/12 text-support-verified',
    edge: 'border-support-verified',
    ring: 'ring-support-verified',
    decoration: 'decoration-support-verified',
  },
  partial: {
    Icon: ShieldHalfIcon,
    mark: 'border-support-partial bg-support-partial/12 text-support-partial',
    edge: 'border-support-partial',
    ring: 'ring-support-partial',
    decoration: 'decoration-support-partial',
  },
  manual: {
    Icon: HandIcon,
    mark: 'border-support-manual bg-support-manual/12 text-support-manual',
    edge: 'border-support-manual',
    ring: 'ring-support-manual',
    decoration: 'decoration-support-manual',
  },
  stale: {
    Icon: ClockAlertIcon,
    mark: 'border-support-stale bg-support-stale/12 text-support-stale',
    edge: 'border-support-stale',
    ring: 'ring-support-stale',
    decoration: 'decoration-support-stale',
  },
  unplayable: {
    Icon: BanIcon,
    mark: 'border-support-unplayable bg-support-unplayable/12 text-support-unplayable',
    edge: 'border-support-unplayable',
    ring: 'ring-support-unplayable',
    decoration: 'decoration-support-unplayable',
  },
}

/** The class that draws a line's left edge in the colour of `support`; the edge is 4 px wide and set by the line. */
export const supportEdge = (support: Support): string => LOOK[support].edge

/** The class that draws a ring in the colour of `support` round a card's picture. */
export const supportRing = (support: Support): string => LOOK[support].ring

/** The class that colours the underline of a clause in the state's colour; the line style is the caller's. */
export const supportDecoration = (support: Support): string => LOOK[support].decoration

/**
 * How far the game plays a card as a pill: an icon, the word (Verified, Partial, Manual, Stale, Unplayable) and an
 * edge in the state's colour, so colour never carries the meaning alone.
 */
export function SupportMark({ support, className }: { support: Support; className?: string }) {
  const { Icon, mark } = LOOK[support]
  return (
    <Pill tone="neutral" data-support={support} className={cn(mark, className)}>
      <Icon aria-hidden="true" />
      {SUPPORT_LABEL[support]}
    </Pill>
  )
}

/**
 * The state's icon alone, in its colour, for a row too tight for a pill: named by the state's word for a screen reader
 * and by a tooltip for a pointer.
 */
export function SupportGlyph({ support, className }: { support: Support; className?: string }) {
  const { Icon, mark } = LOOK[support]
  const word = SUPPORT_LABEL[support]
  return (
    <span
      role="img"
      aria-label={word}
      title={word}
      data-support={support}
      className={cn('inline-grid size-6 shrink-0 place-items-center rounded-full border', mark, className)}
    >
      <Icon aria-hidden="true" className="size-3.5" />
    </span>
  )
}
