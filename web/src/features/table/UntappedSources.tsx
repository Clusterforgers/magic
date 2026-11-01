import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import type { PlayerId } from '@/protocol/generated'
import { cn } from '@/lib/utils'
import { ManaDot } from './ManaPool'
import { SOURCE_KINDS, sourceCounts } from './untappedSources'
import { NO_IDS } from './selectors'

/**
 * The untapped mana sources of a seat by colour: a symbol and its number for every kind at least one source can make,
 * so a glance tells what the seat could still pay. Draws nothing while no source is untapped.
 */
export function UntappedSources({ seat, className }: { seat: PlayerId; className?: string }) {
  const counts = usePlay(
    useShallow((state) => {
      const ids = state.view?.battlefield.cards ?? NO_IDS
      return sourceCounts(
        ids.map((id) => state.objectsById[id]).filter((object) => object?.controller === seat),
        seat,
      )
    }),
  )
  const shown = SOURCE_KINDS.flatMap((type, index) => {
    const count = counts[index] ?? 0
    return count > 0 ? [{ type, count }] : []
  })
  if (shown.length === 0) return null
  return (
    <div
      role="group"
      aria-label="Untapped mana sources"
      data-slot="mana-sources"
      className={cn(
        'flex flex-wrap items-center gap-x-1.5 gap-y-0.5 text-xs text-muted-foreground',
        className,
      )}
    >
      {shown.map(({ type, count }) => (
        <span key={type} data-source={type} className="inline-flex items-center gap-0.5">
          <ManaDot type={type} />
          <span className="font-semibold tabular-nums" aria-hidden="true">
            {count}
          </span>
          <span className="sr-only">
            {count} untapped {type.toLowerCase()} {count === 1 ? 'source' : 'sources'}
          </span>
        </span>
      ))}
    </div>
  )
}
