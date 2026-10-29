import { cn } from '@/lib/utils'
import type { ManaPip } from './cardModel'
import { PIP, PIP_FROM, PIP_INK, PIP_TO } from './cardSize'

interface ManaPipsProps {
  pips: readonly ManaPip[]
  /** Size classes for one symbol, such as `size-4 text-[0.6875rem]`. */
  sizeClass: string
  /** Names the group for a screen reader, such as "Mana cost". */
  label: string
  className?: string
}

/**
 * One symbol: a coloured disc with its letter or number. A hybrid symbol is a disc split on the diagonal with a
 * letter in each half, `{2/W}` has its 2 over the generic half, and a Phyrexian symbol carries the phi mark.
 */
export function Pip({ pip, sizeClass }: { pip: ManaPip; sizeClass: string }) {
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

/** The five colours as one disc, for "one mana of any colour"; decoration, so name it in words beside it. */
export function AnyPip({ sizeClass }: { sizeClass: string }) {
  return (
    <span
      aria-hidden="true"
      data-pip="any"
      style={{
        background:
          'conic-gradient(var(--color-mana-white) 0 20%, var(--color-mana-blue) 0 40%, var(--color-mana-black) 0 60%, var(--color-mana-red) 0 80%, var(--color-mana-green) 0 100%)',
      }}
      className={cn('inline-block shrink-0 rounded-full ring-1 ring-foreground/25', sizeClass)}
    />
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
