import { cn } from '@/lib/utils'
import type { ManaPip } from './cardModel'
import { PIP } from './cardSize'

interface ManaPipsProps {
  pips: readonly ManaPip[]
  /** Size classes for one symbol, such as `size-3.5 text-[0.56rem]`. */
  sizeClass: string
  /** Names the group for a screen reader, such as "Mana cost". */
  label: string
  className?: string
}

/** A row of mana symbols; the group has one spoken name and the symbols themselves are decoration. */
export function ManaPips({ pips, sizeClass, label, className }: ManaPipsProps) {
  const spoken = pips.map((pip) => pip.name).join(', ')
  return (
    <span role="img" aria-label={`${label}: ${spoken}`} className={cn('flex flex-wrap gap-0.5', className)}>
      {pips.map((pip) => (
        <span
          key={pip.key}
          aria-hidden="true"
          className={cn(
            'inline-flex shrink-0 items-center justify-center rounded-full font-bold ring-1 ring-foreground/25',
            sizeClass,
            PIP[pip.tone],
          )}
        >
          {pip.text}
        </span>
      ))}
    </span>
  )
}
