import { memo } from 'react'
import { usePlay } from '@/features/game/gameContext'
import type { PlayerId } from '@/protocol/generated'
import type { CardSize } from './cardSize'
import { CardRow } from './CardRow'
import { useSeatNamer } from './names'
import { playerOf } from './selectors'

/** The most card backs drawn for a hand; the count says the rest. */
const MAX_BACKS = 7

/** A hand the viewer may not see: a fan of card backs. Decoration only; the seat panel prints the count. */
export function HandBacks({ count }: { count: number }) {
  return (
    <span aria-hidden="true" data-slot="hand-backs" className="flex h-10 items-center pl-5">
      {Array.from({ length: Math.min(count, MAX_BACKS) }, (_, index) => (
        <span
          key={index}
          className="-ml-4 h-10 w-7 rounded-sm border border-primary/40 bg-linear-to-br from-primary/80 to-primary/40 ring-1 ring-foreground/20"
        />
      ))}
    </span>
  )
}

/** A seat's hand: the cards in a strip for its owner, card backs for everyone else. */
export const Hand = memo(function Hand({ seat, size }: { seat: PlayerId; size: CardSize }) {
  const owner = useSeatNamer()(seat)
  const cards = usePlay((state) => playerOf(state, seat)?.hand.cards ?? null)
  const count = usePlay((state) => playerOf(state, seat)?.hand.count ?? 0)
  if (cards === null) return <HandBacks count={count} />
  if (cards.length === 0) return <p className="text-sm text-muted-foreground">Your hand is empty.</p>
  return <CardRow ids={cards} label={`${owner}'s hand`} size={size} single />
})
