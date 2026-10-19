import { memo, useRef } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { PlayerId } from '@/protocol/generated'
import { LifeFx } from './fx/LifeFx'
import { ManaPool } from './ManaPool'
import { useSeatNamer } from './names'
import { CountChip, SeatDot } from './PodParts'
import { chipZones, podCounts, podState, tileName } from './podFigures'
import { exileOf, playerOf } from './selectors'
import { useKeepInView } from './useKeepInView'

interface SeatTileProps {
  seat: PlayerId
  /** This is the opponent shown large. */
  focused: boolean
  /** Called with the seat when the tile is clicked, or activated with Enter or Space. */
  onFocusSeat: (seat: PlayerId) => void
}

/**
 * A compact stand-in for an opponent in focus mode, two lines high: the seat's dot, name and life, then the chips of its
 * zones and the mana floating. A press shows that seat's board large. The marks of the turn (`data-state`) are carried
 * like a pod's and styled by the turn cues; the chips are not buttons, as a tile has no button of its own inside.
 */
export const SeatTile = memo(function SeatTile({ seat, focused, onFocusSeat }: SeatTileProps) {
  const name = useSeatNamer()(seat)
  const button = useRef<HTMLButtonElement>(null)
  useKeepInView(button, focused)
  const player = usePlay((state) => playerOf(state, seat))
  const exile = usePlay((state) => exileOf(state, seat).length)
  const flags = usePlay(
    useShallow((state) => ({
      active: state.view?.turn.active === seat,
      priority: state.view?.priority?.holder === seat,
    })),
  )
  if (player === undefined) return null
  const eliminated = player.lost !== null
  const counts = podCounts(player, exile)
  return (
    <button
      ref={button}
      type="button"
      data-variant="tile"
      data-tile-seat={seat}
      data-state={podState(flags.active, flags.priority)}
      aria-current={focused || undefined}
      aria-label={tileName({
        name,
        life: player.life,
        counts,
        active: flags.active,
        priority: flags.priority,
        eliminated,
        focused,
      })}
      onClick={() => onFocusSeat(seat)}
      className={cn(
        'relative grid h-full min-w-44 flex-1 grid-cols-[minmax(0,1fr)] content-center gap-0.5 overflow-hidden rounded-lg bg-card px-2 py-0.5 text-left text-xs ring-1 ring-foreground/10 outline-none hover:bg-muted focus-visible:ring-3 focus-visible:ring-primary',
        focused && 'ring-2 ring-primary',
        eliminated && 'border-l-4 border-destructive',
        flags.active && 'bg-primary/5',
      )}
    >
      <span className="flex h-6 items-center justify-between gap-2">
        <span className="flex min-w-0 items-center gap-1.5">
          <SeatDot seat={seat} />
          <span className="truncate text-sm font-semibold">{name}</span>
        </span>
        <LifeFx
          seat={seat}
          className={cn(
            'text-xl leading-6 font-semibold tabular-nums',
            player.life <= 5 && 'text-destructive',
          )}
        >
          {player.life}
        </LifeFx>
      </span>
      {/* What does not fit the tile is cut, the last figure first; the spoken name has it all. */}
      <span className="flex h-5 items-center gap-x-1.5 overflow-hidden whitespace-nowrap text-muted-foreground">
        {chipZones(counts).map((zone) => (
          <CountChip key={zone} zone={zone} count={counts[zone]} density="tile" />
        ))}
        <ManaPool pool={player.mana} className="shrink-0 flex-nowrap text-xs" />
        {eliminated ? (
          <b className="shrink-0 rounded bg-destructive/10 px-1 text-destructive">Eliminated</b>
        ) : null}
      </span>
    </button>
  )
})
