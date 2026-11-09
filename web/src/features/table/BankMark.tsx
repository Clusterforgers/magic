import { Hourglass } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { PlayerId } from '@/protocol/generated'
import { bankText } from './bankText'
import { playerOf } from './selectors'

/** How long a pod keeps saying that a seat spent an extension, in milliseconds. */
export const SPENT_MS = 5000

/**
 * The timer bank of a seat beside its name: an hourglass and the count of extensions it holds, quiet at zero. When the
 * count drops the mark turns to "used" for a few seconds, so a spent extension is seen by every seat.
 */
export function BankMark({ seat, className }: { seat: PlayerId; className?: string }) {
  const bank = usePlay((state) => playerOf(state, seat)?.bank ?? 0)
  const last = useRef(bank)
  const [spent, setSpent] = useState(false)
  useEffect(() => {
    const before = last.current
    last.current = bank
    if (bank >= before) return undefined
    setSpent(true)
    const id = setTimeout(() => setSpent(false), SPENT_MS)
    return () => clearTimeout(id)
  }, [bank])
  if (bank === 0 && !spent) return null
  const text = bankText(bank, spent)
  return (
    <span
      role="img"
      aria-label={text}
      title={text}
      data-slot="bank-mark"
      data-bank={bank}
      data-spent={spent || undefined}
      className={cn(
        'inline-flex shrink-0 items-center gap-0.5 rounded px-1 text-xs leading-4 font-medium tabular-nums',
        spent ? 'border border-warning bg-warning/15 text-foreground' : 'text-muted-foreground',
        className,
      )}
    >
      <Hourglass aria-hidden="true" className="size-3" />
      <span aria-hidden="true">{spent ? `${bank} used` : bank}</span>
    </span>
  )
}
