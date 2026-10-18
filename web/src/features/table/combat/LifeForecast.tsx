import { Skull, Swords } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { PlayerId } from '@/protocol/generated'
import { usePlayerOutlook } from './drawn'
import { lifeWords } from './forecastText'
import './combat.css'

/**
 * What combat would do to a seat's life, on its panel: "20 → 16", with a "Lethal" mark when it would end the game for
 * that seat, or "No combat damage" when every attacker aimed at it is blocked. Nothing when nobody attacks the seat or
 * no forecast holds. The words for a screen reader are the full sentence; the figures and the mark are for the eye.
 */
export function LifeForecast({ seat }: { seat: PlayerId }) {
  const outlook = usePlayerOutlook(seat)
  if (outlook === null) return null
  const { life, life_after: after, damage, lethal } = outlook
  return (
    <p
      data-slot="life-forecast"
      data-lethal={lethal || undefined}
      className={cn(
        'flex flex-wrap items-center gap-x-2 gap-y-1 rounded-md px-2 py-1 text-sm font-medium',
        lethal ? 'bg-destructive/10 text-destructive ring-1 ring-destructive/50' : 'bg-muted',
      )}
    >
      <Swords aria-hidden="true" className="size-4 shrink-0" />
      <span className="sr-only">{lifeWords(outlook)}</span>
      <span aria-hidden="true" className="flex items-baseline gap-1.5 tabular-nums">
        {damage === 0 ? (
          'No combat damage'
        ) : (
          <>
            <span>{life}</span>
            <span>→</span>
            <b className="text-base">{after}</b>
          </>
        )}
      </span>
      {lethal ? (
        <b
          aria-hidden="true"
          data-slot="lethal"
          className="inline-flex items-center gap-1 rounded bg-(--combat-damage) px-1.5 py-0.5 text-xs text-white uppercase"
        >
          <Skull className="size-3.5" />
          Lethal
        </b>
      ) : null}
    </p>
  )
}
