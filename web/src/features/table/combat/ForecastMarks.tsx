import { Skull } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { CreatureOutlook } from './forecast'
import './combat.css'

interface ForecastMarksProps {
  outlook: CreatureOutlook
  /** The card lies on its side, so the card as seen is a band, five sevenths of its width high, across the middle of its button. */
  tapped: boolean
  /** A compact card has room for the skull only. */
  compact: boolean
}

// Cracks over a doomed card, drawn on a 60 by 84 sheet that is stretched to the card; the strokes keep their width.
const CRACKS = [
  'M30 0 L26 14 L34 24 L24 38 L33 50 L27 62 L31 84',
  'M26 14 L12 20 L7 31',
  'M33 50 L47 55 L53 69',
]

/**
 * What the forecast says about a card, drawn over it: a grey, cracked look and a skull when it would die, and a "-N"
 * badge for the damage it would take. Decoration for the eye; the same words are in the card's spoken name. Sits in a
 * `relative` card button and follows the card as seen, turned or not.
 */
export function ForecastMarks({ outlook, tapped, compact }: ForecastMarksProps) {
  const { damage, dies } = outlook
  return (
    <span
      data-slot="forecast"
      aria-hidden="true"
      className={cn(
        'pointer-events-none absolute inset-x-0 overflow-hidden rounded-md',
        tapped ? 'top-[calc((var(--card-h)-var(--card-w)*5/7)/2)] h-[calc(var(--card-w)*5/7)]' : 'inset-y-0',
      )}
    >
      {dies ? (
        <span
          data-slot="forecast-cracks"
          className="absolute inset-0 bg-neutral-500/45 backdrop-contrast-90 backdrop-grayscale"
        >
          <svg
            viewBox="0 0 60 84"
            preserveAspectRatio="none"
            fill="none"
            className="size-full text-neutral-900/80 dark:text-neutral-100/80"
          >
            {CRACKS.map((path) => (
              <path
                key={path}
                d={path}
                stroke="currentColor"
                strokeWidth={1.6}
                strokeLinejoin="round"
                vectorEffect="non-scaling-stroke"
              />
            ))}
          </svg>
        </span>
      ) : null}
      <span className="absolute top-0.5 right-0.5 flex items-center gap-0.5">
        {dies ? (
          <span
            data-slot="forecast-doomed"
            className="flex items-center gap-0.5 rounded-full bg-neutral-900 px-1 py-px text-xs leading-none font-bold text-white ring-1 ring-white/70"
          >
            <Skull className="size-3" />
            {compact ? null : 'Dies'}
          </span>
        ) : null}
        {damage > 0 ? (
          <span
            data-slot="forecast-damage"
            className="rounded bg-(--combat-damage) px-1 py-px text-xs leading-none font-bold text-white tabular-nums ring-1 ring-white/70"
          >
            -{damage}
          </span>
        ) : null}
      </span>
    </span>
  )
}
