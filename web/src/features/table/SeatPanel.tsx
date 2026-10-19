import { memo } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { describeLoss } from '@/features/game/outcome'
import { usePlay } from '@/features/game/gameContext'
import { Badge } from '@/components/ui/badge'
import { cn } from '@/lib/utils'
import type { PlayerId } from '@/protocol/generated'
import { Card } from './Card'
import type { CardSize } from './cardSize'
import { CardRow } from './CardRow'
import { LifeForecast } from './combat/LifeForecast'
import { LifeFx } from './fx/LifeFx'
import { Hand } from './Hand'
import { ManaPool } from './ManaPool'
import { useSeatNamer } from './names'
import { exileOf, isYourTurn, NO_IDS, playerOf } from './selectors'
import { describeMulligan } from './turnText'
import { ZoneButton } from './ZoneButton'

interface SeatPanelProps {
  seat: PlayerId
  /** The size of the cards in this panel's hand strip. */
  size: CardSize
  /** `panel` is the whole pod; `line` puts name, life and counts on one line, for a column of boards or a narrow window. */
  variant?: 'panel' | 'line'
  /** Draw the viewer's own hand in the panel. The table has a hand dock for it and turns this off. */
  hand?: boolean
}

/**
 * Everything about a seat except its battlefield: name, life, mana pool, library and hand, graveyard, exile and the
 * command zone. `data-seat` marks it for the decision layer and for tests. Given room (a container query) it lays
 * out in two columns, the seat's numbers beside its zones; as a `line` it keeps to one row of the essentials.
 */
export const SeatPanel = memo(function SeatPanel({
  seat,
  size,
  variant = 'panel',
  hand: withHand = true,
}: SeatPanelProps) {
  const owner = useSeatNamer()(seat)
  const player = usePlay((state) => playerOf(state, seat))
  const exile = usePlay(useShallow((state) => exileOf(state, seat)))
  const flags = usePlay(
    useShallow((state) => ({
      you: state.seat === seat,
      active: state.view?.turn.active === seat,
      // The viewer's own turn: the panel joins the banner in the "your turn" glow.
      cue: state.seat === seat && isYourTurn(state),
      priority: state.view?.priority?.holder === seat,
      mulligan: state.view?.stage === 'Mulligan',
      commander: state.view?.format.toLowerCase().includes('commander') ?? false,
    })),
  )
  if (player === undefined) return null
  const graveyard = player.graveyard.cards ?? NO_IDS
  const top = graveyard.at(-1)
  const commanders = player.command.cards ?? NO_IDS
  const eliminated = player.lost !== null
  // Only the owner is told which cards are in a hand; for everyone else it is a fan of card backs.
  const seesHand = player.hand.cards !== null
  const marks = {
    'aria-label': flags.you ? `${owner} (you)` : owner,
    'data-seat': seat,
    'data-you': flags.you || undefined,
    'data-active': flags.active || undefined,
    'data-your-turn': flags.cue || undefined,
    'data-eliminated': eliminated || undefined,
  }
  // The glow carries its own ring, so it stands in for the ring classes instead of adding to them.
  const ring = flags.cue
    ? 'your-turn-cue'
    : flags.active
      ? 'ring-2 ring-primary'
      : 'ring-1 ring-foreground/10'

  if (variant === 'line') {
    const floating = player.mana.counts.some((count) => count > 0)
    return (
      <section
        {...marks}
        data-variant="line"
        className={cn(
          '@container flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1 rounded-xl bg-card px-3 py-1 text-card-foreground',
          ring,
          eliminated && 'border-l-4 border-destructive',
        )}
      >
        <h3 className="min-w-0 truncate text-sm font-semibold">{owner}</h3>
        <LifeFx as="span" seat={seat} className="shrink-0">
          <span className="sr-only">Life </span>
          <span className={cn('text-lg font-bold tabular-nums', player.life <= 5 && 'text-destructive')}>
            {player.life}
          </span>
        </LifeFx>
        {flags.you ? <Badge variant="secondary">you</Badge> : null}
        {flags.active ? <Badge>Active turn</Badge> : null}
        {flags.priority ? <Badge variant="outline">Has priority</Badge> : null}
        <span className="text-xs text-muted-foreground">
          Hand <b className="text-foreground tabular-nums">{player.hand.count}</b>
        </span>
        <span className="text-xs text-muted-foreground">
          Library <b className="text-foreground tabular-nums">{player.library.count}</b>
        </span>
        <LifeForecast seat={seat} />
        {player.lost === null ? null : (
          <span className="text-xs font-medium text-destructive">
            Eliminated: {describeLoss(player.lost)}
          </span>
        )}
        {flags.mulligan ? (
          <span data-slot="mulligan" className="text-xs text-muted-foreground">
            Mulligan: {describeMulligan(player.mulligan, player.mulligans)}
          </span>
        ) : null}
        {floating ? <ManaPool pool={player.mana} /> : null}
        <span className="ml-auto flex items-center gap-1">
          <ZoneButton zone="Graveyard" owner={owner} ids={graveyard} compact />
          <ZoneButton zone="Exile" owner={owner} ids={exile} compact />
        </span>
      </section>
    )
  }

  return (
    <section
      {...marks}
      className={cn(
        '@container grid content-start gap-2 rounded-xl bg-card p-3 text-card-foreground',
        ring,
        eliminated && 'border-l-4 border-destructive',
      )}
    >
      <div className="grid gap-x-6 gap-y-2 @2xl:grid-cols-[minmax(11rem,16rem)_minmax(0,1fr)]">
        <div className="grid content-start gap-2">
          <header className="flex items-start justify-between gap-3">
            <div className="min-w-0">
              <h3 className="truncate text-lg leading-tight font-semibold">{owner}</h3>
              <p className="mt-1 flex flex-wrap gap-1 empty:hidden">
                {flags.you ? <Badge variant="secondary">you</Badge> : null}
                {flags.active ? <Badge>Active turn</Badge> : null}
                {flags.priority ? <Badge variant="outline">Has priority</Badge> : null}
              </p>
            </div>
            <LifeFx as="p" seat={seat} className="shrink-0 text-right">
              <span className="block text-xs text-muted-foreground">Life</span>
              <span
                className={cn(
                  'text-4xl leading-none font-bold tabular-nums',
                  player.life <= 5 && 'text-destructive',
                )}
              >
                {player.life}
              </span>
            </LifeFx>
          </header>
          <LifeForecast seat={seat} />
          {player.lost === null ? null : (
            <p className="rounded-md bg-destructive/10 px-2 py-1 text-sm font-medium text-destructive">
              Eliminated: {describeLoss(player.lost)}
            </p>
          )}
          {flags.mulligan ? (
            <p data-slot="mulligan" className="rounded-md bg-muted px-2 py-1 text-sm">
              Mulligan: {describeMulligan(player.mulligan, player.mulligans)}
            </p>
          ) : null}
          <ManaPool pool={player.mana} />
          <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-sm text-muted-foreground">
            {seesHand ? null : <Hand seat={seat} size={size} />}
            <span>
              Library <b className="text-foreground tabular-nums">{player.library.count}</b>
            </span>
            <span>
              Hand <b className="text-foreground tabular-nums">{player.hand.count}</b>
            </span>
          </div>
        </div>
        <div className="grid content-start gap-2">
          <div className="flex flex-wrap items-center gap-2">
            {top === undefined ? null : <Card id={top} size="compact" />}
            <ZoneButton zone="Graveyard" owner={owner} ids={graveyard} />
            <ZoneButton zone="Exile" owner={owner} ids={exile} />
            <CardRow ids={commanders} label={`${owner}'s command zone`} size="compact" className="p-0" />
          </div>
          {flags.commander || commanders.length > 0 ? (
            <p
              data-slot="commander-stats"
              className="rounded-md border border-dashed px-2 py-1 text-xs text-muted-foreground"
            >
              Commander damage and tax: not tracked yet
            </p>
          ) : null}
        </div>
      </div>
      {seesHand && withHand ? <Hand seat={seat} size={size} /> : null}
    </section>
  )
})
