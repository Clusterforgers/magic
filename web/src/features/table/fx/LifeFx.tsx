import type { ReactNode } from 'react'
import { cn } from '@/lib/utils'
import type { PlayerId } from '@/protocol/generated'
import { FxFloat } from './FxFloat'
import { useSeatFx } from './hooks'
import { fxAttr } from './model'

interface LifeFxProps {
  seat: PlayerId
  /** The element to draw: a paragraph, or inside a line of text a span. */
  as?: 'p' | 'span'
  className?: string
  children: ReactNode
}

/**
 * Wraps the life total of a seat. While the seat takes or gains life it carries `data-fx` (a colour flash in the
 * stylesheet) and floats the amount; the number inside is always the true life, and the float is `aria-hidden`.
 */
export function LifeFx({ seat, as: Tag = 'span', className, children }: LifeFxProps) {
  const entries = useSeatFx(seat)
  return (
    <Tag data-slot="life" data-fx={fxAttr(entries)} className={cn('relative', className)}>
      {children}
      {entries.map((entry) =>
        entry.kind === 'damaged' || entry.kind === 'healed' ? (
          <FxFloat key={entry.token} kind={entry.kind} amount={entry.amount} />
        ) : null,
      )}
    </Tag>
  )
}
