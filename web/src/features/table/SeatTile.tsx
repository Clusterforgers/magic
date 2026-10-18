import { memo, useRef } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { PlayerId } from '@/protocol/generated'
import { useSeatNamer } from './names'
import { battlefieldCounts, playerOf } from './selectors'
import { useKeepInView } from './useKeepInView'

interface SeatTileProps {
  seat: PlayerId
  /** This is the opponent shown large. */
  focused: boolean
  /** Called with the seat when the tile is clicked, or activated with Enter or Space. */
  onFocusSeat: (seat: PlayerId) => void
}

/** A compact stand-in for an opponent in focus mode: name, life, counts and a summary of their board. */
export const SeatTile = memo(function SeatTile({ seat, focused, onFocusSeat }: SeatTileProps) {
  const name = useSeatNamer()(seat)
  const button = useRef<HTMLButtonElement>(null)
  useKeepInView(button, focused)
  const player = usePlay((state) => playerOf(state, seat))
  const flags = usePlay(
    useShallow((state) => ({
      active: state.view?.turn.active === seat,
      priority: state.view?.priority?.holder === seat,
    })),
  )
  const counts = usePlay(useShallow((state) => battlefieldCounts(state, seat)))
  if (player === undefined) return null
  const eliminated = player.lost !== null
  const notes = [
    flags.active ? 'active turn' : null,
    flags.priority ? 'has priority' : null,
    eliminated ? 'eliminated' : null,
  ]
  const spoken = notes.filter((note) => note !== null).join(', ')
  return (
    <button
      ref={button}
      type="button"
      data-tile-seat={seat}
      aria-current={focused || undefined}
      aria-label={`Show ${name}'s board. Life ${player.life}, ${player.hand.count} cards in hand${spoken === '' ? '' : `, ${spoken}`}${focused ? ', shown now' : ''}`}
      onClick={() => onFocusSeat(seat)}
      className={cn(
        'grid min-w-36 content-start gap-0.5 rounded-lg bg-card p-2 text-left text-xs ring-1 ring-foreground/10 outline-none hover:bg-muted focus-visible:ring-3 focus-visible:ring-primary',
        focused && 'ring-2 ring-primary',
        eliminated && 'border-l-4 border-destructive',
        flags.active && 'bg-primary/5',
      )}
    >
      <span className="flex items-baseline justify-between gap-2">
        <span className="truncate text-sm font-semibold">{name}</span>
        <span className="text-xl leading-none font-bold tabular-nums">{player.life}</span>
      </span>
      {/* Each figure keeps together, so a narrow tile wraps between them and never in the middle of one. */}
      <span className="flex flex-wrap gap-x-2 text-muted-foreground">
        <span className="whitespace-nowrap">Hand {player.hand.count}</span>
        <span className="whitespace-nowrap">Library {player.library.count}</span>
      </span>
      <span className="flex flex-wrap gap-x-2 text-muted-foreground">
        <span className="whitespace-nowrap">{counts.creatures} creatures</span>
        <span className="whitespace-nowrap">{counts.lands + counts.creatures + counts.other} permanents</span>
      </span>
      <span className="flex flex-wrap gap-1 empty:hidden">
        {flags.active ? <b className="rounded bg-primary px-1 text-primary-foreground">Active turn</b> : null}
        {flags.priority ? <b className="rounded border px-1">Priority</b> : null}
        {eliminated ? <b className="rounded bg-destructive/10 px-1 text-destructive">Eliminated</b> : null}
      </span>
    </button>
  )
})
