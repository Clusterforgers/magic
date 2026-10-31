import { Crosshair } from 'lucide-react'
import { memo, useRef } from 'react'
import type { MouseEvent } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { describeLoss } from '@/features/game/outcome'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { PlayerId } from '@/protocol/generated'
import { DamageChips, TaxChips, useCommanderChips } from './CommanderChips'
import { cornerHeight } from './cornerFit'
import { useEnchantChips } from './enchantChips'
import { EnchantChips } from './EnchantChips'
import { useDefender } from './combat/defender'
import type { DefenderState } from './combat/defender'
import { LifeForecast } from './combat/LifeForecast'
import { LifeFx } from './fx/LifeFx'
import { LiveManaPool } from './LiveManaPool'
import { usePodRoom } from './podRoom'
import { useSeatNamer } from './names'
import { ACTIVE_EDGE, CountChip, PriorityDot, SeatDot, YOUR_TURN_EDGE } from './PodParts'
import { podCounts, podState, podStateWords, priorityName, showsPriorityDot } from './podFigures'
import { exileOf, holdsPriority, isActiveSeat, isYourTurn, NO_IDS, playerOf } from './selectors'
import { describeMulligan } from './turnText'
import { useBoxHeight } from './useBoxHeight'
import { SeatCorner } from './SeatCorner'
import { RevealedChip } from './RevealedChip'
import { ZoneButton } from './ZoneButton'

interface SeatPanelProps {
  seat: PlayerId
  /** `panel` is the pod for a column 168 px wide; `line` is one row, for a column of boards or a narrow window. */
  variant?: 'panel' | 'line'
}

/** How a pod reads while an attack is aimed: a seat that may be attacked has an outline, the one chosen a filled one. */
function defenderClass(state: DefenderState | undefined): string | undefined {
  if (state === undefined) return undefined
  return state === 'chosen'
    ? 'cursor-pointer bg-primary/10 outline-2 outline-primary'
    : 'cursor-pointer outline-1 outline-primary/40 hover:outline-2 hover:outline-primary'
}

/** A click on a pod that no control inside it took picks the seat as the defender. */
function pickOnClick(pick: () => void): (event: MouseEvent<HTMLElement>) => void {
  return (event) => {
    if (event.target instanceof Element && event.target.closest('button, a, [role="button"]') !== null) return
    pick()
  }
}

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
 * a list of `active` and `priority`. The active seat has a 3 px edge inside the pod, yours the your-turn glow with it,
 * and a seat that holds priority without the turn has a dot beside its name. As a `panel` it fits a column 168 px wide, with the
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
      active: isActiveSeat(state, seat),
      // The viewer's own turn: the pod joins the banner in the "your turn" glow.
      cue: state.seat === seat && isYourTurn(state),
      priority: holdsPriority(state, seat),
      mulligan: state.view?.stage === 'Mulligan',
    })),
  )
  const chips = useCommanderChips(seat)
  const enchants = useEnchantChips(seat)
  const defender = useDefender(seat)
  const commanderGame = usePlay((state) => state.view?.format.toLowerCase() === 'commander')
  // The blocks above the corner: its room is the column less these (the column is measured by `PodColumn`).
  const above = useRef<HTMLDivElement>(null)
  const aboveHeight = useBoxHeight(above)
  const room = usePodRoom()
  if (player === undefined) return null
  const graveyard = player.graveyard.cards ?? NO_IDS
  const commanders = player.command.cards ?? NO_IDS
  // Your own commanders wait at the end of the hand dock, where they are cast; the pod keeps their chips.
  const drawn = flags.you ? NO_IDS : commanders
  const taxOnCards = chips.tax.filter((chip) => drawn.includes(chip.key))
  const taxLoose = chips.tax.filter((chip) => !drawn.includes(chip.key))
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
    'data-defender': defender.state,
    onClick: defender.state === undefined ? undefined : pickOnClick(defender.pick),
  }
  const words = [
    podStateWords(flags.active, flags.priority),
    defender.state === 'chosen' ? 'Your attackers go here.' : '',
  ]
    .filter((part) => part !== '')
    .join(' ')
  // The glow carries its own ring and edge, so it stands in for the ring classes instead of adding to them.
  const ring = flags.cue ? YOUR_TURN_EDGE : cn('ring-1 ring-foreground/10', flags.active && ACTIVE_EDGE)
  const line = variant === 'line'
  const mana = <LiveManaPool pool={player.mana} />
  const notes = (
    <>
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
      {defender.state === 'chosen' ? (
        <Crosshair aria-hidden="true" className="size-3.5 shrink-0 text-primary" />
      ) : null}
      {flags.you ? (
        <span className={cn('shrink-0 text-xs text-muted-foreground', line && '@max-sm:hidden')}>you</span>
      ) : null}
      {showsPriorityDot(flags.active, flags.priority) ? (
        <PriorityDot label={priorityName(owner, flags.you)} />
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
          defenderClass(defender.state),
        )}
      >
        <div className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
          <div className="flex min-w-24 flex-1 items-center justify-between gap-2">
            {name}
            <div className="flex shrink-0 items-center gap-1.5">
              <LifeForecast seat={seat} />
              <Life
                seat={seat}
                life={player.life}
                as="span"
                className="text-2xl font-semibold tabular-nums"
              />
            </div>
          </div>
          <div role="group" aria-label={`${owner}'s zones`} className="flex shrink-0 items-center gap-1">
            <CountChip zone="hand" count={counts.hand} density="line" />
            <CountChip zone="library" count={counts.library} density="line" />
            <RevealedChip owner={owner} player={player} density="line" />
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
          <TaxChips chips={chips.tax} density="line" />
          <DamageChips chips={chips.damage} density="line" />
          <EnchantChips chips={enchants} density="line" />
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
        // One track that may shrink (`minmax(0, 1fr)`): a chip that is longer than the pod is cut inside it instead of
        // widening the pod and pushing the life figure out of the column.
        '@container relative grid min-w-0 grid-cols-[minmax(0,1fr)] content-start gap-2 rounded-xl bg-card p-2 text-card-foreground',
        ring,
        eliminated && 'border-l-4 border-destructive',
        defenderClass(defender.state),
      )}
    >
      {/* Everything above the corner: its height does not depend on how the corner is drawn, so the corner can be sized by it. */}
      <div ref={above} data-slot="pod-top" className="grid grid-cols-[minmax(0,1fr)] content-start gap-2">
        <header className="flex items-center justify-between gap-2">
          <div className="grid min-w-0 gap-0.5">
            {name}
            {/* Room for the pool is kept under the name, so nothing moves when mana floats. */}
            <div data-slot="mana-slot" className="min-h-5">
              {mana}
            </div>
          </div>
          <div className="flex shrink-0 items-center gap-1.5">
            <LifeForecast seat={seat} />
            <Life seat={seat} life={player.life} as="p" className="text-life" />
          </div>
        </header>
        {words === '' ? null : <p className="sr-only">{words}</p>}
        <div data-slot="pod-status" className="grid content-start gap-1 empty:hidden">
          {notes}
        </div>
        <div data-slot="pod-counts" className="flex flex-wrap gap-1">
          <CountChip zone="hand" count={counts.hand} density="line" />
          <CountChip zone="library" count={counts.library} density="line" />
          <RevealedChip owner={owner} player={player} density="line" />
          {/* One damage chip is the figure alone; with several each keeps the name of its commander. */}
          <DamageChips
            chips={chips.damage}
            density="line"
            compact={chips.damage.length === 1}
            className="max-w-full"
          />
          <TaxChips chips={taxLoose} density="line" className="max-w-full" />
          <EnchantChips chips={enchants} density="line" />
        </div>
      </div>
      <SeatCorner
        owner={owner}
        graveyard={graveyard}
        exile={exile}
        commanders={commanders}
        inDock={flags.you}
        commandSlot={commanderGame && (player.commanders?.length ?? 0) > 0}
        tax={taxOnCards}
        height={cornerHeight(room.height, aboveHeight)}
        card={room.card}
      />
    </section>
  )
})
