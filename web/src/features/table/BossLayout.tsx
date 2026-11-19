/**
 * Boss mode of the table, for a co-op fight against a boss: the boss's side across the top (the boss's board wide
 * with its pod, its minions beside it), and the party together at the bottom: the other heroes' boards in a band over
 * your own. The rows share the height by what each draws (`bossSplit.ts`): the heroes' cards come first, the boss's
 * are smaller, an empty ally or idle own board is a line, and a hand dock with nothing in it is a slim bar. On a phone both
 * sides are tile strips over one board.
 */
import { useMemo, useState } from 'react'
import type { CSSProperties } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import type { PlayerId } from '@/protocol/generated'
import type { CardSize } from './cardSize'
import { GridCardContext, useGridCard } from './gridCard'
import type { GridCard } from './gridCard'
import { handIdle } from './handIdle'
import { ALLY_STRIP_PX, ALLY_WIDE_SPAN, BOSS_SPAN } from './layout'
import type { PartyRows, PodPlace, SeatPlan } from './layout'
import { LeftStrip, SeatArea, TileStrip } from './Seats'
import { SeatTile } from './SeatTile'
import { BoardZones } from './BoardZones'
import { useSeatNamer } from './names'
import { BossAbilitiesBar } from './campaign/BossAbilities'
import { BossBarContext } from './campaign/useBossAbilities'
import { handSeat, isBattlefieldEmpty, playerOf } from './selectors'

interface BossSideProps {
  plan: SeatPlan
  size: CardSize
  /** The boss's pod: a column beside its board, a line over it, or none under a strip of tiles. */
  pod: PodPlace
  /** The widest card the boss's side draws in px (`PartyRows.bossCard`); null leaves each board its own fit. */
  card?: number | null
  onFocusSeat: (seat: PlayerId) => void
}

/** The seats of `seats` that have left the game, and those still playing. */
function useGone(seats: readonly PlayerId[]): { gone: PlayerId[]; playing: PlayerId[] } {
  const gone = usePlay(useShallow((state) => seats.filter((seat) => playerOf(state, seat)?.lost != null)))
  return { gone, playing: seats.filter((seat) => !gone.includes(seat)) }
}

const noReport = (): void => undefined

/** `grid` with its widest card cut to `cap` px; a board alone gets the cap as its widest. Unchanged without a cap. */
function useCapped(grid: GridCard | null, cap: number | null | undefined): GridCard | null {
  return useMemo(() => {
    if (cap === null || cap === undefined || cap <= 0) return grid
    return { most: Math.min(grid?.most ?? cap, cap), report: grid?.report ?? noReport }
  }, [grid, cap])
}

/** The minions' boards, framed, at one card size. */
function Minions({
  seats,
  size,
  card,
}: {
  seats: readonly PlayerId[]
  size: CardSize
  card: number | null | undefined
}) {
  const grid = useCapped(useGridCard(seats), card)
  return (
    <GridCardContext value={grid}>
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

/**
 * The boss's side: its abilities in a row on top, then the boss's board wide with its pod and each minion's board beside
 * it; tiles and one board on a phone. The bar shows the boss's phase, scheme, Omen and rules, so its pod leaves them out.
 * A side with no permanent keeps its room (`splitBoss`): the boss's pod and its empty board, so its first permanents
 * move no row.
 */
export function BossSide(props: BossSideProps) {
  return (
    <BossBarContext value={true}>
      <BossSideBody {...props} />
    </BossBarContext>
  )
}

function BossSideBody({ plan, size, pod, card, onFocusSeat }: BossSideProps) {
  const [boss, ...minions] = plan.boards
  const { gone, playing } = useGone(minions)
  const bossGrid = useCapped(null, card)
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
            <GridCardContext value={bossGrid}>
              <SeatArea seat={boss} size={size} facing="far" pod={pod} />
            </GridCardContext>
          </div>
        )}
        <Minions seats={playing} size={size} card={card} />
      </div>
    </div>
  )
}

interface PartySideProps {
  plan: SeatPlan
  size: CardSize
  /** Your pod: a column beside your board, or a line under it on a phone. */
  pod: PodPlace
  /** The height of the other heroes' band in px (`PartyRows.band`). */
  band: number
  /** The allies' column shares (`PartyRows.shares`); one each when left out. */
  shares?: PartyRows['shares']
  phone: boolean
}

/** One ally in its frame: its tile with its zone chips beside it over its board, which is left out while it holds no permanent. */
function AllyFrame({
  seat,
  size,
  wide,
  onPress,
}: {
  seat: PlayerId
  size: CardSize
  wide: boolean
  onPress: (seat: PlayerId) => void
}) {
  const empty = usePlay((state) => isBattlefieldEmpty(state, seat))
  const owner = useSeatNamer()(seat)
  return (
    <div
      data-slot="ally-frame"
      data-wide={wide || undefined}
      data-collapsed={empty || undefined}
      className="flex min-h-0 min-w-0 flex-col gap-1 rounded-xl border-2 border-foreground/15 p-0.5"
    >
      {/* The zone chips stand beside the tile, so the board under it keeps the whole rest of the frame. */}
      <div className="flex h-14 shrink-0 items-center gap-1">
        <SeatTile seat={seat} focused={wide} onFocusSeat={onPress} />
        <BoardZones seat={seat} owner={owner} />
      </div>
      <SeatArea seat={seat} size={size} facing="near" pod="none" collapsed={empty} zones={false} />
    </div>
  )
}

/**
 * The other heroes in a row of frames: each a tile (name, life, zones) over its board, at one card size, the columns
 * sharing the row by what the boards hold. A press on a tile widens that hero's board to `ALLY_WIDE_SPAN` shares of
 * the row, a second press evens the row again.
 */
function AllyRow({
  allies,
  size,
  band,
  shares,
}: {
  allies: readonly PlayerId[]
  size: CardSize
  band: number
  shares: readonly number[] | undefined
}) {
  const [wide, setWide] = useState<PlayerId | null>(null)
  const widened = wide !== null && allies.includes(wide) && allies.length > 1 ? wide : null
  const card = useGridCard(widened === null ? allies : [])
  const press = (pressed: PlayerId): void => setWide((was) => (was === pressed ? null : pressed))
  const style: CSSProperties = {
    height: band,
    gridTemplateColumns: allies
      .map((seat, at) => {
        const share = widened === null ? (shares?.[at] ?? 1) : seat === widened ? ALLY_WIDE_SPAN : 1
        return `minmax(0, ${share}fr)`
      })
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
          <AllyFrame key={seat} seat={seat} size={size} wide={seat === widened} onPress={press} />
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

/** Your board: with no permanent and nothing in your hand to play, your pod as one line. */
function OwnBoard({ plan, size, pod }: { plan: SeatPlan; size: CardSize; pod: PodPlace }) {
  const idle = usePlay(
    (state) => isBattlefieldEmpty(state, plan.viewer) && handIdle(state, handSeat(state, plan.viewer)),
  )
  const line = idle && pod === 'aside'
  return <SeatArea seat={plan.viewer} size={size} facing="near" pod={line ? 'line' : pod} collapsed={line} />
}

/** The party: the other heroes over your board, which takes the rest of the row. */
export function PartySide({ plan, size, pod, band, shares, phone }: PartySideProps) {
  const own = <OwnBoard plan={plan} size={size} pod={pod} />
  if (plan.allies.length === 0) return own
  return (
    <div data-slot="party" className="flex min-h-0 min-w-0 flex-1 flex-col gap-2">
      {/* A row too short for the other heroes' boards (`splitBoss`) keeps their tiles, as a phone does. */}
      {phone || band === ALLY_STRIP_PX ? (
        <AllyStrip allies={plan.allies} size={size} />
      ) : (
        <AllyRow allies={plan.allies} size={size} band={band} shares={shares} />
      )}
      {own}
    </div>
  )
}
