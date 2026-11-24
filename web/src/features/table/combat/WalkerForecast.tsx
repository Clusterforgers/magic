import { Skull } from 'lucide-react'
import { useMemo } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import type { ObjectView, PlayerId } from '@/protocol/generated'
import { useBlockPreview } from './forecast'
import { seatWalkers, walkerWords } from './forecastTotal'
import './combat.css'

/**
 * What combat would do to the planeswalkers of a seat, as chips beside its life: the walker's name, the loyalty it
 * would lose ("-3") and a skull when it would fall; `short` (a tile) keeps the name's first letter. Nothing when none
 * of its planeswalkers is attacked.
 */
export function WalkerForecast({ seat, short = false }: { seat: PlayerId; short?: boolean }) {
  const forecast = useBlockPreview()
  const objects = usePlay(
    useShallow((state) =>
      (forecast?.attacked ?? []).map((walker): ObjectView | undefined => state.objectsById[walker.object]),
    ),
  )
  const walkers = useMemo(() => {
    const byId = new Map(
      objects.flatMap((object) => (object === undefined ? [] : [[object.id, object] as const])),
    )
    return seatWalkers(forecast, seat, (id) => byId.get(id)?.controller).map((walker) => ({
      walker,
      name: byId.get(walker.object)?.name ?? 'Planeswalker',
    }))
  }, [forecast, objects, seat])
  if (walkers.length === 0) return null
  const choice = forecast?.depends_on_choice === true
  return walkers.map(({ walker, name }) => {
    const label = walkerWords(name, walker, choice)
    return (
      <span
        key={walker.object}
        data-slot="walker-forecast"
        data-object-id={walker.object}
        data-lethal={walker.dies || undefined}
        role="img"
        aria-label={label}
        title={label}
        className="inline-flex h-5 max-w-28 shrink-0 items-center gap-0.5 rounded-full bg-destructive/10 px-1.5 text-xs leading-none font-semibold text-destructive tabular-nums"
      >
        {/* A tile has no room for the name: its first letter tells two walkers apart, the label says it whole. */}
        <span aria-hidden="true" className="min-w-0 truncate font-medium">
          {short ? name.slice(0, 1) : name}
        </span>
        {walker.dies ? <Skull aria-hidden="true" className="size-3 shrink-0" /> : null}
        <span aria-hidden="true" className="shrink-0">
          {choice ? '≈' : ''}-{walker.damage}
        </span>
      </span>
    )
  })
}
