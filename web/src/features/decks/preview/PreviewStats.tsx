import { DISC } from '@/features/builder/discs'
import { IDENTITY_NAME } from '@/features/builder/search'
import { SupportMark } from '@/features/decks/SupportMark'
import { cn } from '@/lib/utils'
import type { Counts, Support } from '@/protocol/generated'
import { CURVE_LAST } from './model'
import type { CardKind } from './model'

/** The label of a curve bucket: its mana value, "7+" for the last. */
export const curveLabel = (bucket: number): string => (bucket === CURVE_LAST ? `${CURVE_LAST}+` : `${bucket}`)

const SUPPORT_SHOWN: readonly Support[] = ['verified', 'partial', 'manual', 'stale', 'unplayable']
// Whole class names, so Tailwind can see them.
const SEGMENT: Readonly<Record<Support, string>> = {
  verified: 'bg-support-verified',
  partial: 'bg-support-partial',
  manual: 'bg-support-manual',
  stale: 'bg-support-stale',
  unplayable: 'bg-support-unplayable',
}

/**
 * The copies of a deck by support state: a bar with a segment per state and, under it, the state's mark with its count
 * (Verified, Partial and Manual always, Stale and Unplayable when the deck has some).
 */
export function SupportBar({ counts }: { counts: Counts }) {
  const { total } = counts
  return (
    <>
      <div
        aria-hidden="true"
        className="flex h-2 overflow-hidden rounded-full bg-muted"
        data-slot="support-bar"
      >
        {SUPPORT_SHOWN.map((support) =>
          counts[support] === 0 ? null : (
            <span
              key={support}
              className={SEGMENT[support]}
              style={{ width: `${(counts[support] / total) * 100}%` }}
            />
          ),
        )}
      </div>
      <ul aria-label="Cards by support" className="flex flex-wrap gap-x-3 gap-y-2">
        {SUPPORT_SHOWN.map((support) => {
          const count = counts[support]
          const always = support === 'verified' || support === 'partial' || support === 'manual'
          return count === 0 && !always ? null : (
            <li key={support} data-support-count={support} className="flex items-center gap-2 text-sm">
              <SupportMark support={support} />
              <span className="font-medium tabular-nums">{count}</span>
            </li>
          )
        })}
      </ul>
    </>
  )
}

/** The spells of a deck by mana value as small bars with their counts; lands are not part of it. */
export function ManaCurve({ curve }: { curve: readonly number[] }) {
  const peak = Math.max(1, ...curve)
  return (
    <figure data-slot="mana-curve" className="grid min-w-0 gap-1">
      <figcaption className="text-xs font-medium">Mana curve</figcaption>
      <ol aria-label="Spells by mana value" className="grid h-24 grid-cols-8 items-end gap-1">
        {curve.map((count, bucket) => (
          <li
            key={curveLabel(bucket)}
            data-bucket={bucket}
            aria-label={`Mana value ${curveLabel(bucket)}: ${count} ${count === 1 ? 'card' : 'cards'}`}
            className="flex h-full min-w-0 flex-col items-center justify-end gap-1"
          >
            <span aria-hidden="true" className="text-label text-muted-foreground tabular-nums">
              {count}
            </span>
            <span
              aria-hidden="true"
              className="w-full rounded-t-sm bg-primary"
              style={{ height: `${Math.round((count / peak) * 62)}%`, minHeight: count > 0 ? 2 : 0 }}
            />
            <span aria-hidden="true" className="w-full border-t pt-1 text-center text-label tabular-nums">
              {curveLabel(bucket)}
            </span>
          </li>
        ))}
      </ol>
    </figure>
  )
}

/** The copies of a deck by type as horizontal bars with their counts. */
export function TypeBars({ types }: { types: readonly { kind: CardKind; label: string; count: number }[] }) {
  const peak = Math.max(1, ...types.map((type) => type.count))
  return (
    <figure data-slot="type-bars" className="grid min-w-0 gap-1">
      <figcaption className="text-xs font-medium">Card types</figcaption>
      <ul aria-label="Cards by type" className="grid gap-1">
        {types.map((type) => (
          <li
            key={type.kind}
            data-kind={type.kind}
            className="grid grid-cols-[5.5rem_minmax(0,1fr)_1.75rem] items-center gap-2 text-xs"
          >
            <span className="truncate">{type.label}</span>
            <span aria-hidden="true" className="h-2 rounded-full bg-muted">
              <span
                className="block h-full rounded-full bg-primary"
                style={{ width: `${Math.round((type.count / peak) * 100)}%` }}
              />
            </span>
            <span className="text-right tabular-nums">{type.count}</span>
          </li>
        ))}
      </ul>
    </figure>
  )
}

/** The spells of a deck by colour as a disc with the letter and the count; colour is never the only sign. */
export function ColourCounts({ colours }: { colours: readonly { letter: string; count: number }[] }) {
  return (
    <figure data-slot="colour-counts" className="grid min-w-0 gap-1">
      <figcaption className="text-xs font-medium">Spells by colour</figcaption>
      <ul aria-label="Spells by colour" className="flex flex-wrap gap-x-3 gap-y-1">
        {colours.map(({ letter, count }) => (
          <li
            key={letter}
            data-colour={letter}
            aria-label={`${letter === 'C' ? 'Colorless' : (IDENTITY_NAME[letter] ?? letter)}: ${count} ${count === 1 ? 'card' : 'cards'}`}
            className="flex items-center gap-1 text-sm"
          >
            <span
              aria-hidden="true"
              className={cn(
                'flex size-5 items-center justify-center rounded-full text-xs leading-none font-semibold ring-1 ring-foreground/25',
                DISC[letter],
              )}
            >
              {letter}
            </span>
            <span aria-hidden="true" className="tabular-nums">
              {count}
            </span>
          </li>
        ))}
      </ul>
    </figure>
  )
}
