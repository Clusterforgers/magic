/**
 * Boss mode of the table, for a co-op fight against a boss: the boss's side across the top (the boss's board wide
 * with its pod, its minions beside it), and the party together at the bottom: the other heroes' boards in a band over
 * your own, which stays the largest. On a phone both sides are tile strips over one board.
 */
import { useState } from 'react'
import type { CSSProperties } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import type { PlayerId } from '@/protocol/generated'
import type { CardSize } from './cardSize'
import { GridCardContext, useGridCard } from './gridCard'
import { ALLY_WIDE_SPAN, BOSS_SPAN } from './layout'
import type { PodPlace, SeatPlan } from './layout'
import { LeftStrip, SeatArea, TileStrip } from './Seats'
import { SeatTile } from './SeatTile'
import { BossAbilitiesBar } from './campaign/BossAbilities'
import { playerOf } from './selectors'

interface BossSideProps {
  plan: SeatPlan
  size: CardSize
  /** The boss's pod: a column beside its board, a line over it, or none under a strip of tiles. */
  pod: PodPlace
  onFocusSeat: (seat: PlayerId) => void
}

/** The seats of `seats` that have left the game, and those still playing. */
function useGone(seats: readonly PlayerId[]): { gone: PlayerId[]; playing: PlayerId[] } {
  const gone = usePlay(useShallow((state) => seats.filter((seat) => playerOf(state, seat)?.lost != null)))
  return { gone, playing: seats.filter((seat) => !gone.includes(seat)) }
}

/** The minions' boards, framed, at one card size. */
function Minions({ seats, size }: { seats: readonly PlayerId[]; size: CardSize }) {
  const card = useGridCard(seats)
  return (
    <GridCardContext value={card}>
      {seats.map((seat) => (
        <div
          key={seat}
          data-slot="minion-frame"
          className="flex min-h-0 min-w-0 rounded-xl border-2 border-boss/40 p-1"
        >
          <SeatArea seat={seat} size={size} facing="far" pod="line" />
        </div>
      ))}
    </GridCardContext>
  )
}

/** The boss's side: the boss's board wide with its pod, each minion's board beside it; tiles and one board on a phone. */
export function BossSide({ plan, size, pod, onFocusSeat }: BossSideProps) {
  const [boss, ...minions] = plan.boards
  const { gone, playing } = useGone(minions)
  const abilities = boss === undefined ? null : <BossAbilitiesBar seat={boss} />
  if (plan.tiles.length > 0) {
    return (
      <div data-layout="boss" className="flex min-h-0 min-w-0 flex-col gap-2">
        {abilities}
        <TileStrip
          tiles={plan.tiles}
          focused={plan.focused}
          onFocusSeat={onFocusSeat}
          label="The boss's side"
        />
        {plan.focused === null ? null : <SeatArea seat={plan.focused} size={size} facing="far" pod={pod} />}
      </div>
    )
  }
  const columns: CSSProperties = {
    gridTemplateColumns:
      playing.length === 0
        ? 'minmax(0, 1fr)'
        : `minmax(0, ${BOSS_SPAN}fr) repeat(${playing.length}, minmax(0, 1fr))`,
  }
  return (
    <div data-layout="boss" data-minions={playing.length} className="flex min-h-0 min-w-0 flex-col gap-2">
      {abilities}
      {gone.map((seat) => (
        <LeftStrip key={seat} seat={seat} />
      ))}
      <div className="grid min-h-0 min-w-0 flex-1 gap-2" style={columns}>
        {boss === undefined ? null : (
          <div data-slot="boss-frame" className="flex min-h-0 min-w-0 rounded-xl border-2 border-boss/70 p-1">
            <SeatArea seat={boss} size={size} facing="far" pod={pod} />
          </div>
        )}
        <Minions seats={playing} size={size} />
      </div>
    </div>
  )
}

interface PartySideProps {
  plan: SeatPlan
  size: CardSize
  /** Your pod: a column beside your board, or a line under it on a phone. */
  pod: PodPlace
  /** The height of the other heroes' band in px (`alliesBandPx`). */
  band: number
  phone: boolean
}

/**
 * The other heroes in a row of frames: each a tile (name, life, zones) over its board, at one card size. A press on a
 * tile widens that hero's board to `ALLY_WIDE_SPAN` shares of the row, a second press evens the row again.
 */
function AllyRow({ allies, size, band }: { allies: readonly PlayerId[]; size: CardSize; band: number }) {
  const [wide, setWide] = useState<PlayerId | null>(null)
  const widened = wide !== null && allies.includes(wide) && allies.length > 1 ? wide : null
  const card = useGridCard(widened === null ? allies : [])
  const style: CSSProperties = {
    height: band,
    gridTemplateColumns: allies
      .map((seat) => `minmax(0, ${seat === widened ? ALLY_WIDE_SPAN : 1}fr)`)
      .join(' '),
  }
  return (
    <GridCardContext value={widened === null ? card : null}>
      <section
        aria-label="Your party"
        data-slot="allies"
        className="grid min-h-0 shrink-0 gap-2"
        style={style}
      >
        {allies.map((seat) => (
          <div
            key={seat}
            data-slot="ally-frame"
            data-wide={seat === widened || undefined}
            className="flex min-h-0 min-w-0 flex-col gap-1 rounded-xl border-2 border-foreground/15 p-1"
          >
            <div className="flex h-14 shrink-0">
              <SeatTile
                seat={seat}
                focused={seat === widened}
                onFocusSeat={(pressed) => setWide((was) => (was === pressed ? null : pressed))}
              />
            </div>
            <SeatArea seat={seat} size={size} facing="near" pod="none" />
          </div>
        ))}
      </section>
    </GridCardContext>
  )
}

/** The party on a phone: a strip of the other heroes' tiles; a tap opens that hero's board over yours, a second closes it. */
function AllyStrip({ allies, size }: { allies: readonly PlayerId[]; size: CardSize }) {
  const [peek, setPeek] = useState<PlayerId | null>(null)
  const shown = peek !== null && allies.includes(peek) ? peek : null
  return (
    <>
      <TileStrip
        tiles={allies}
        focused={shown}
        onFocusSeat={(seat) => setPeek((was) => (was === seat ? null : seat))}
        label="Your party"
      />
      {shown === null ? null : (
        <div
          data-slot="ally-peek"
          className="flex min-h-0 min-w-0 shrink-0 basis-2/5 rounded-xl border-2 border-foreground/15 p-1"
        >
          <SeatArea seat={shown} size={size} facing="near" pod="none" />
        </div>
      )}
    </>
  )
}

/** The party: the other heroes over your board, which takes the rest of the row. */
export function PartySide({ plan, size, pod, band, phone }: PartySideProps) {
  const own = <SeatArea seat={plan.viewer} size={size} facing="near" pod={pod} />
  if (plan.allies.length === 0) return own
  return (
    <div data-slot="party" className="flex min-h-0 min-w-0 flex-1 flex-col gap-2">
      {phone ? (
        <AllyStrip allies={plan.allies} size={size} />
      ) : (
        <AllyRow allies={plan.allies} size={size} band={band} />
      )}
      {own}
    </div>
  )
}
