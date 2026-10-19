import { memo } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { describeLoss } from '@/features/game/outcome'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { PlayerId } from '@/protocol/generated'
import { CardRow } from './CardRow'
import type { CardBox } from './CardRow'
import { LifeForecast } from './combat/LifeForecast'
import { LifeFx } from './fx/LifeFx'
import { ManaPool } from './ManaPool'
import { useSeatNamer } from './names'
import { CountChip, SeatDot } from './PodParts'
import { podCounts, podState, podStateWords } from './podFigures'
import { exileOf, isYourTurn, NO_IDS, playerOf } from './selectors'
import { describeMulligan } from './turnText'
import { ZoneButton } from './ZoneButton'

interface SeatPanelProps {
  seat: PlayerId
  /** `panel` is the pod for a column 168 px wide; `line` is one row, for a column of boards or a narrow window. */
  variant?: 'panel' | 'line'
}

/** The box of a commander in the pod: the smallest card the fit engine draws. */
const COMMAND_BOX: CardBox = { width: 44, height: 62 }

interface LifeProps {
  seat: PlayerId
  life: number
  as: 'p' | 'span'
  className: string
}

/** The life total, the largest figure of a pod; red at 5 or less. */
function Life({ seat, life, as, className }: LifeProps) {
  return (
    <LifeFx as={as} seat={seat} className={cn('shrink-0 leading-none', className)}>
      <span className="sr-only">Life </span>
      <span className={cn(life <= 5 && 'text-destructive')}>{life}</span>
    </LifeFx>
  )
}

/**
 * The pod of a seat: its name with the seat's dot, life as the largest figure, the mana floating in the pool, chips
 * with the number of cards in the hand, library, graveyard and exile, and the commander once the command zone holds
 * one. A slot with nothing to say draws nothing. The root carries `data-seat` for the decision layer and `data-state`,
 * a list of `active` and `priority`, for the turn cues. As a `panel` it fits a column 168 px wide, with the
 * commander beside the chips; as a `line` it keeps to one row of the essentials and wraps the chips only when the
 * row is too narrow.
 */
export const SeatPanel = memo(function SeatPanel({ seat, variant = 'panel' }: SeatPanelProps) {
  const owner = useSeatNamer()(seat)
  const player = usePlay((state) => playerOf(state, seat))
  const exile = usePlay(useShallow((state) => exileOf(state, seat)))
  const flags = usePlay(
    useShallow((state) => ({
      you: state.seat === seat,
      active: state.view?.turn.active === seat,
      // The viewer's own turn: the pod joins the banner in the "your turn" glow.
      cue: state.seat === seat && isYourTurn(state),
      priority: state.view?.priority?.holder === seat,
      mulligan: state.view?.stage === 'Mulligan',
    })),
  )
  if (player === undefined) return null
  const graveyard = player.graveyard.cards ?? NO_IDS
  const commanders = player.command.cards ?? NO_IDS
  const counts = podCounts(player, exile.length)
  const eliminated = player.lost !== null
  const marks = {
    'aria-label': flags.you ? `${owner} (you)` : owner,
    'data-seat': seat,
    'data-you': flags.you || undefined,
    'data-active': flags.active || undefined,
    'data-your-turn': flags.cue || undefined,
    'data-eliminated': eliminated || undefined,
    'data-state': podState(flags.active, flags.priority),
  }
  const words = podStateWords(flags.active, flags.priority)
  // The glow carries its own ring, so it stands in for the ring classes instead of adding to them.
  const ring = flags.cue
    ? 'your-turn-cue'
    : flags.active
      ? 'ring-2 ring-primary'
      : 'ring-1 ring-foreground/10'
  const line = variant === 'line'
  const mana = <ManaPool pool={player.mana} />
  const notes = (
    <>
      <LifeForecast seat={seat} />
      {player.lost === null ? null : (
        <p className="rounded-md bg-destructive/10 px-2 py-1 text-xs font-medium text-destructive">
          Eliminated: {describeLoss(player.lost)}
        </p>
      )}
      {flags.mulligan ? (
        <p data-slot="mulligan" className="rounded-md bg-muted px-2 py-1 text-xs">
          Mulligan: {describeMulligan(player.mulligan, player.mulligans)}
        </p>
      ) : null}
    </>
  )
  const name = (
    <div className="flex min-w-0 items-center gap-1.5">
      <SeatDot seat={seat} />
      <h3 className="truncate text-sm leading-5 font-semibold">{owner}</h3>
      {flags.you ? (
        <span className={cn('shrink-0 text-xs text-muted-foreground', line && '@max-sm:hidden')}>you</span>
      ) : null}
    </div>
  )

  if (line) {
    return (
      <section
        {...marks}
        data-variant="line"
        className={cn(
          '@container flex min-w-0 flex-col gap-1 rounded-xl bg-card px-2 py-1 text-card-foreground',
          ring,
          eliminated && 'border-l-4 border-destructive',
        )}
      >
        <div className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
          <div className="flex min-w-24 flex-1 items-center justify-between gap-2">
            {name}
            <Life seat={seat} life={player.life} as="span" className="text-2xl font-semibold tabular-nums" />
          </div>
          <div role="group" aria-label={`${owner}'s zones`} className="flex shrink-0 items-center gap-1">
            <CountChip zone="hand" count={counts.hand} density="line" />
            <CountChip zone="library" count={counts.library} density="line" />
            <ZoneButton zone="graveyard" owner={owner} ids={graveyard} density="line" />
            <ZoneButton zone="exile" owner={owner} ids={exile} density="line" />
            {counts.command > 0 ? (
              <ZoneButton zone="command" owner={owner} ids={commanders} density="line" />
            ) : null}
          </div>
        </div>
        {words === '' ? null : <p className="sr-only">{words}</p>}
        <div data-slot="pod-status" className="flex flex-wrap items-center gap-x-2 gap-y-1 empty:hidden">
          {mana}
          {notes}
        </div>
      </section>
    )
  }

  return (
    <section
      {...marks}
      data-variant="panel"
      className={cn(
        '@container relative grid content-start gap-2 rounded-xl bg-card p-2 text-card-foreground',
        ring,
        eliminated && 'border-l-4 border-destructive',
      )}
    >
      <header className="flex items-center justify-between gap-2">
        <div className="grid min-w-0 gap-0.5">
          {name}
          {/* Room for the pool is kept under the name, so nothing moves when mana floats. */}
          <div data-slot="mana-slot" className="min-h-5">
            {mana}
          </div>
        </div>
        <Life seat={seat} life={player.life} as="p" className="text-life" />
      </header>
      {words === '' ? null : <p className="sr-only">{words}</p>}
      <div data-slot="pod-status" className="grid content-start gap-1 empty:hidden">
        {notes}
      </div>
      <div className="flex gap-2">
        <div role="group" aria-label={`${owner}'s zones`} className="grid min-w-0 flex-1 grid-cols-2 gap-1">
          <CountChip zone="hand" count={counts.hand} />
          <CountChip zone="library" count={counts.library} />
          <ZoneButton zone="graveyard" owner={owner} ids={graveyard} />
          <ZoneButton zone="exile" owner={owner} ids={exile} />
        </div>
        {commanders.length === 0 ? null : (
          <CardRow
            ids={commanders}
            label={`${owner}'s command zone`}
            size="compact"
            box={COMMAND_BOX}
            className="shrink-0 content-start"
          />
        )}
      </div>
    </section>
  )
})
