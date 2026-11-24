import { Skull, Sword } from 'lucide-react'
import type { ReactNode } from 'react'
import { cn } from '@/lib/utils'
import type { CreatureOutlook } from './forecast'
import { CHOICE_WORDS, DIES_WORDS, dealtWords, takenWords } from './forecastText'
import './combat.css'

interface ForecastMarksProps {
  outlook: CreatureOutlook
  /** The card lies on its side, so the card as seen is a band, five sevenths of its width high, across the middle of its button. */
  tapped: boolean
  /** A compact card has room for the skull only. */
  compact: boolean
  /** The card's top right corner holds another mark (the chosen tick, a prompt's note, a cost now): the badges go under it. */
  cornerTaken?: boolean
}

// Cracks over a doomed card, drawn on a 60 by 84 sheet that is stretched to the card; the strokes keep their width.
const CRACKS = [
  'M30 0 L26 14 L34 24 L24 38 L33 50 L27 62 L31 84',
  'M26 14 L12 20 L7 31',
  'M33 50 L47 55 L53 69',
]

/** One badge of the forecast: its meaning in its icon or sign, its words in its label and tooltip. */
function Badge({
  slot,
  label,
  className,
  children,
}: {
  slot: string
  label: string
  className: string
  children: ReactNode
}) {
  return (
    <span
      data-slot={slot}
      role="img"
      aria-label={label}
      title={label}
      className={cn(
        'flex items-center gap-px px-1 py-px text-xs leading-none font-bold text-white tabular-nums ring-1 ring-white/70',
        className,
      )}
    >
      {children}
    </span>
  )
}

/**
 * What the forecast says about a card, drawn over it: a sword and the damage it would deal, a "-N" badge for the damage
 * it would take, and a grey, cracked look with a skull when it would die. Each badge carries its meaning in a sign or
 * icon and its words in its label, never in its colour alone; the card's spoken name says the same. Sits in a
 * `relative` card button, takes no pointer and follows the card as seen, turned or not.
 */
export function ForecastMarks({ outlook, tapped, compact, cornerTaken = false }: ForecastMarksProps) {
  const { damage, dies, choice } = outlook
  const dealt = outlook.dealt ?? 0
  const about = choice ? '≈' : ''
  const words = (text: string): string => (choice ? `${text}, ${CHOICE_WORDS}` : text)
  // A compact card on its side is too low for three rows of badges: the skull joins the damage badge ("☠-6").
  const merged = compact && tapped && damage > 0 && dies
  const dealtBadge =
    dealt > 0 ? (
      <Badge slot="forecast-dealt" label={words(dealtWords(dealt))} className="rounded bg-neutral-900 px-0.5">
        <Sword aria-hidden="true" className="size-3" />
        <span aria-hidden="true">
          {about}
          {dealt}
        </span>
      </Badge>
    ) : null
  return (
    <span
      data-slot="forecast"
      className={cn(
        'pointer-events-none absolute inset-x-0 overflow-hidden rounded-md',
        tapped ? 'top-[calc((var(--card-h)-var(--card-w)*5/7)/2)] h-[calc(var(--card-w)*5/7)]' : 'inset-y-0',
      )}
    >
      {dies ? (
        <span
          data-slot="forecast-cracks"
          aria-hidden="true"
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
      {/* What it deals stands bottom left on a card that stands, over the foot's line where the power is read and clear
          of the stat box; a card on its side is too low for that line, so there it leads the top row. */}
      {dealtBadge === null || tapped ? null : (
        <span data-slot="forecast-dealt-place" className="absolute bottom-5 left-0.5 flex">
          {dealtBadge}
        </span>
      )}
      {/* What happens to it stands top right; on a card too narrow for every badge they wrap rather than leave it. */}
      <span
        data-slot="forecast-badges"
        data-corner-taken={cornerTaken || undefined}
        className={cn(
          'absolute inset-x-0.5 flex flex-wrap items-center justify-end gap-0.5',
          cornerTaken ? 'top-5.5' : 'top-0.5',
        )}
      >
        {tapped ? dealtBadge : null}
        {damage > 0 ? (
          <Badge
            slot="forecast-damage"
            label={words(merged ? `${takenWords(damage)} and dies` : takenWords(damage))}
            className="rounded bg-(--combat-damage)"
          >
            {merged ? <Skull aria-hidden="true" className="size-3" /> : null}
            <span aria-hidden="true">
              {about}-{damage}
            </span>
          </Badge>
        ) : null}
        {dies && !merged ? (
          <Badge slot="forecast-doomed" label={words(DIES_WORDS)} className="rounded-full bg-neutral-900">
            <Skull aria-hidden="true" className="size-3" />
            {compact ? null : <span aria-hidden="true">{DIES_WORDS}</span>}
          </Badge>
        ) : null}
      </span>
    </span>
  )
}
