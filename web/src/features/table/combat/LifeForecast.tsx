import { Skull } from 'lucide-react'
import { useMemo } from 'react'
import { cn } from '@/lib/utils'
import type { PlayerForecast, PlayerId } from '@/protocol/generated'
import { usePlayerOutlook } from './drawn'
import { useBlockPreview } from './forecast'
import { forecastSentences, lifeWords } from './forecastText'
import { useForecastNames } from './useForecastNames'
import './combat.css'

/** The change of life the forecast says, as the chip writes it: "-3", "+2" or "0". */
export function lifeDelta(outlook: Pick<PlayerForecast, 'life' | 'life_after'>): string {
  const change = outlook.life_after - outlook.life
  return change > 0 ? `+${change}` : String(change)
}

/** Which way the life would go: down (damage), up (lifelink of a blocker) or unchanged. */
export function lifeTrend(outlook: Pick<PlayerForecast, 'life' | 'life_after'>): 'down' | 'up' | 'same' {
  return outlook.life_after < outlook.life ? 'down' : outlook.life_after > outlook.life ? 'up' : 'same'
}

/** The tooltip of the chip: its sentence and then what each attacker would do, one line each. */
function useTip(outlook: PlayerForecast): string {
  const forecast = useBlockPreview()
  const names = useForecastNames(forecast)
  return useMemo(
    () => [lifeWords(outlook), ...(forecast === null ? [] : forecastSentences(forecast, names))].join('\n'),
    [outlook, forecast, names],
  )
}

/**
 * What combat would do to a seat's life, as a small chip beside the life figure: the change in red ("-3"), in green
 * when the seat would gain, "0" when everything aimed at it is blocked, and a skull on a filled chip when the damage
 * would end the game for it. Nothing when nobody attacks the seat or no forecast holds. A screen reader hears the
 * sentence ("Life would go from 20 to 17"); the tooltip adds the forecast's sentences.
 */
export function LifeForecast({ seat }: { seat: PlayerId }) {
  const outlook = usePlayerOutlook(seat)
  if (outlook === null) return null
  return <Chip outlook={outlook} />
}

function Chip({ outlook }: { outlook: PlayerForecast }) {
  const tip = useTip(outlook)
  const trend = lifeTrend(outlook)
  return (
    <span
      data-slot="life-forecast"
      data-lethal={outlook.lethal || undefined}
      data-trend={trend}
      role="img"
      aria-label={lifeWords(outlook)}
      title={tip}
      className={cn(
        'inline-flex h-5 shrink-0 items-center gap-0.5 rounded-full px-1.5 text-xs leading-none font-semibold tabular-nums',
        outlook.lethal
          ? 'bg-(--combat-damage) text-white'
          : trend === 'down'
            ? 'bg-destructive/10 text-destructive'
            : trend === 'up'
              ? 'bg-success/10 text-success'
              : 'bg-muted text-muted-foreground',
      )}
    >
      {outlook.lethal ? <Skull aria-hidden="true" className="size-3" /> : null}
      <span aria-hidden="true">{lifeDelta(outlook)}</span>
    </span>
  )
}
