import { cn } from '@/lib/utils'
import type { ObjectId, PlayerId } from '@/protocol/generated'
import { useCardForecast, useSeatForecast } from './forecast'

interface ForecastBadgeProps {
  /** The words ("+3/+3 → 7/7", "3 damage · dies", "−3 → 17"). */
  words: string
  /** On a card it sits at the card's top edge; on a pod it is a chip in the row of the life figure. */
  place: 'card' | 'pod'
}

/**
 * What the top stack item will do to one card or seat, in the fewest words: a dark badge at the card's top edge or a
 * chip beside a seat's life (docs/ui-design.md, Forecast). Read by a screen reader as "Forecast: …".
 */
export function ForecastBadge({ words, place }: ForecastBadgeProps) {
  return (
    <span
      data-slot="stack-forecast"
      data-place={place}
      role="note"
      aria-label={`Forecast: ${words}`}
      title={words}
      className={cn(
        'pointer-events-none rounded-lg bg-neutral-900 px-1.5 text-xs leading-4 font-bold text-white tabular-nums shadow-sm ring-1 ring-white/70',
        place === 'card'
          ? 'absolute inset-x-0.5 top-0.5 z-10 mx-auto w-fit text-center text-balance'
          : 'inline-block max-w-full shrink-0 truncate',
      )}
    >
      {words}
    </span>
  )
}

/** The forecast badge of a card on the table; nothing when the top stack item leaves it as it is. */
export function CardForecast({ id }: { id: ObjectId }) {
  const words = useCardForecast(id)
  return words === null ? null : <ForecastBadge words={words} place="card" />
}

/** The forecast chip of a seat's pod; nothing when the top stack item leaves the seat as it is. */
export function SeatForecast({ seat }: { seat: PlayerId }) {
  const words = useSeatForecast(seat)
  return words === null ? null : <ForecastBadge words={words} place="pod" />
}
