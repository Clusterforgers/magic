import { TriangleAlertIcon } from 'lucide-react'
import type { ComponentProps } from 'react'
import { Pill } from '@/components/Pill'
import { cn } from '@/lib/utils'
import { STILL_PLAYS } from './legality'

interface LegalityPillProps extends Omit<ComponentProps<typeof Pill>, 'tone' | 'children'> {
  /** The words: "Not legal in Commander", "Not legal at this table (2)". */
  label: string
}

/**
 * A short legality warning beside a deck in a list: an icon and the words, with that the deck still plays as its hover.
 * Never colour alone, and never a refusal.
 */
export function LegalityPill({ label, className, ...props }: LegalityPillProps) {
  return (
    <Pill
      tone="warning"
      data-slot="legality-pill"
      title={`${label}. ${STILL_PLAYS}`}
      className={cn('h-auto min-h-6 whitespace-normal', className)}
      {...props}
    >
      <TriangleAlertIcon aria-hidden="true" />
      {label}
    </Pill>
  )
}
