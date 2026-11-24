import { HeartCrack, Skull, Sword } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import { useMemo, useState } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { CombatForecast, ObjectId, PlayerId } from '@/protocol/generated'
import { useAllies } from '../prompt/useAllies'
import { forecastTotal, totalChips } from './forecastTotal'
import type { TotalChip } from './forecastTotal'
import './combat.css'

const ICONS: Record<TotalChip['kind'], LucideIcon | null> = {
  deals: Sword,
  takes: HeartCrack,
  lost: Skull,
  killed: Skull,
  none: null,
}

/** The ids of every creature and planeswalker a forecast names, for looking up their controllers. */
const namedIds = (forecast: CombatForecast | null): ObjectId[] =>
  forecast === null
    ? []
    : [
        ...forecast.creatures.map((entry) => entry.object),
        ...(forecast.attacked ?? []).map((entry) => entry.object),
      ]

/**
 * The combat forecast's total in the dock or a phone's bar, under a combat declaration: what the viewer's side would
 * deal and take, and the creatures each side would lose, as chips with an icon, a word and a number (on a line of its
 * own, or beside the last answers where the home has no line to spare, `PromptLayout`). The total is
 * always drawn, empty without a forecast, so the dock does not move, and while the engine is asked (`checking`) the
 * last total stays, dimmed. Each chip has its sentence as its label; the line is a group a screen reader can find.
 */
export function ForecastTotal({
  forecast,
  checking,
}: {
  forecast: CombatForecast | null
  checking: boolean
}) {
  const seat = usePlay((state) => state.seat)
  const allies = useAllies()
  const ids = useMemo(() => namedIds(forecast), [forecast])
  const controllers = usePlay(useShallow((state) => ids.map((id) => state.objectsById[id]?.controller)))
  const chips = useMemo(() => {
    if (forecast === null || seat === null) return null
    const byId = new Map(ids.map((id, at) => [id, controllers[at]]))
    const side = new Set<PlayerId>([seat, ...allies])
    return totalChips(
      forecastTotal(forecast, side, (id) => byId.get(id)),
      allies.length > 0,
    )
  }, [forecast, seat, allies, controllers, ids])
  // The last chips drawn, kept (set while rendering, the way React documents it) while a new answer is checked.
  const [kept, setKept] = useState<TotalChip[] | null>(chips)
  if (chips !== null && chips !== kept) setKept(chips)
  const stale = chips === null && checking ? kept : null
  const shown = chips ?? stale
  return (
    <div
      data-slot="forecast-total"
      data-state={chips !== null ? 'ready' : stale !== null ? 'stale' : 'empty'}
      role="group"
      aria-label="Combat outcome"
      className={cn(
        'flex h-5 min-w-0 grow items-center gap-1 overflow-hidden transition-opacity',
        stale !== null && 'opacity-50',
      )}
    >
      {(shown ?? []).map((chip) => {
        const Icon = chip.lethal ? Skull : ICONS[chip.kind]
        return (
          <span
            key={chip.kind}
            data-slot="total-chip"
            data-kind={chip.kind}
            data-lethal={chip.lethal || undefined}
            role="img"
            aria-label={chip.label}
            title={chip.label}
            className={cn(
              'inline-flex h-5 shrink-0 items-center gap-1 rounded-full px-1.5 text-xs leading-none font-semibold whitespace-nowrap tabular-nums',
              chip.lethal
                ? 'bg-(--combat-damage) text-white'
                : chip.kind === 'takes' || chip.kind === 'lost'
                  ? 'bg-destructive/10 text-destructive'
                  : chip.kind === 'none'
                    ? 'bg-muted text-muted-foreground'
                    : 'bg-foreground/10 text-foreground',
            )}
          >
            {Icon === null ? null : <Icon aria-hidden="true" className="size-3 shrink-0" />}
            <span aria-hidden="true">
              {chip.text}
              {chip.lethal ? ' · lethal' : ''}
            </span>
          </span>
        )
      })}
    </div>
  )
}
