import { cn } from '@/lib/utils'
import type { ManaPip } from './cardModel'
import { PIP, PIP_FROM, PIP_INK, PIP_TO } from './cardSize'

interface ManaPipsProps {
  pips: readonly ManaPip[]
  /** Size classes for one symbol, such as `size-3.5 text-[0.56rem]`. */
  sizeClass: string
  /** Names the group for a screen reader, such as "Mana cost". */
  label: string
  className?: string
}

/**
 * One symbol: a coloured disc with its letter or number. A hybrid symbol is a disc split on the diagonal with a
 * letter in each half, `{2/W}` has its 2 over the generic half, and a Phyrexian symbol carries the phi mark.
 */
function Pip({ pip, sizeClass }: { pip: ManaPip; sizeClass: string }) {
  const split = pip.split
  return (
    <span
      aria-hidden="true"
      data-pip={pip.symbol}
      className={cn(
        'relative inline-flex shrink-0 items-center justify-center overflow-hidden rounded-full font-bold ring-1 ring-foreground/25',
        sizeClass,
        split === undefined
          ? PIP[pip.tone]
          : cn('bg-linear-to-br from-50% to-50%', PIP_FROM[pip.tone], PIP_TO[split]),
      )}
    >
      {split !== undefined && pip.text2 !== undefined ? (
        <>
          <span className={cn('absolute top-[5%] left-[13%] text-[0.62em] leading-none', PIP_INK[pip.tone])}>
            {pip.text}
          </span>
          <span className={cn('absolute right-[13%] bottom-[5%] text-[0.62em] leading-none', PIP_INK[split])}>
            {pip.text2}
          </span>
        </>
      ) : (
        <span
          className={cn(
            split !== undefined &&
              'inline-flex size-[68%] items-center justify-center rounded-full bg-background/85 text-[0.85em] leading-none text-foreground',
          )}
        >
          {pip.text}
        </span>
      )}
    </span>
  )
}

/** A row of mana symbols; the group has one spoken name and the symbols themselves are decoration. */
export function ManaPips({ pips, sizeClass, label, className }: ManaPipsProps) {
  const spoken = pips.map((pip) => pip.name).join(', ')
  return (
    <span role="img" aria-label={`${label}: ${spoken}`} className={cn('flex flex-wrap gap-0.5', className)}>
      {pips.map((pip) => (
        <Pip key={pip.key} pip={pip} sizeClass={sizeClass} />
      ))}
    </span>
  )
}
