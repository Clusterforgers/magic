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
import { Hand } from './Hand'
import { ManaPool } from './ManaPool'
import { useSeatNamer } from './names'
import { exileOf, NO_IDS, playerOf } from './selectors'
import { describeMulligan } from './turnText'
import { ZoneButton } from './ZoneButton'

interface SeatPanelProps {
  seat: PlayerId
  /** The size of the cards in this panel's hand and command zone. */
  size: CardSize
}

/**
 * Everything about a seat except its battlefield: name, life, mana pool, library and hand, graveyard, exile and the
 * command zone. `data-seat` marks it for the decision layer and for tests.
 */
export const SeatPanel = memo(function SeatPanel({ seat, size }: SeatPanelProps) {
  const owner = useSeatNamer()(seat)
  const player = usePlay((state) => playerOf(state, seat))
  const exile = usePlay(useShallow((state) => exileOf(state, seat)))
  const flags = usePlay(
    useShallow((state) => ({
      you: state.seat === seat,
      active: state.view?.turn.active === seat,
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
  return (
    <section
      aria-label={flags.you ? `${owner} (you)` : owner}
      data-seat={seat}
      data-you={flags.you || undefined}
      data-active={flags.active || undefined}
      data-eliminated={eliminated || undefined}
      className={cn(
        'grid content-start gap-2 rounded-xl bg-card p-3 text-card-foreground ring-1 ring-foreground/10',
        flags.active && 'ring-2 ring-primary',
        eliminated && 'border-l-4 border-destructive',
      )}
    >
      <header className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <h3 className="truncate text-lg leading-tight font-semibold">{owner}</h3>
          <p className="mt-1 flex flex-wrap gap-1 empty:hidden">
            {flags.you ? <Badge variant="secondary">you</Badge> : null}
            {flags.active ? <Badge>Active turn</Badge> : null}
            {flags.priority ? <Badge variant="outline">Has priority</Badge> : null}
          </p>
        </div>
        <p className="shrink-0 text-right">
          <span className="block text-xs text-muted-foreground">Life</span>
          <span
            className={cn(
              'text-4xl leading-none font-bold tabular-nums',
              player.life <= 5 && 'text-destructive',
            )}
          >
            {player.life}
          </span>
        </p>
      </header>
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
        <span>
          Library <b className="text-foreground tabular-nums">{player.library.count}</b>
        </span>
        <span>
          Hand <b className="text-foreground tabular-nums">{player.hand.count}</b>
        </span>
      </div>
      <Hand seat={seat} size={size} />
      <div className="flex flex-wrap items-center gap-2">
        {top === undefined ? null : <Card id={top} size="compact" />}
        <ZoneButton zone="Graveyard" owner={owner} ids={graveyard} />
        <ZoneButton zone="Exile" owner={owner} ids={exile} />
      </div>
      <CardRow ids={commanders} label={`${owner}'s command zone`} size={size} />
      {flags.commander || commanders.length > 0 ? (
        <p
          data-slot="commander-stats"
          className="rounded-md border border-dashed px-2 py-1 text-xs text-muted-foreground"
        >
          Commander damage and tax: not tracked yet
        </p>
      ) : null}
    </section>
  )
})
