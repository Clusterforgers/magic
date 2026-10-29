import { CURVE_LAST } from './model'
import type { CardKind } from './model'

/** The label of a curve bucket: its mana value, "7+" for the last. */
export const curveLabel = (bucket: number): string => (bucket === CURVE_LAST ? `${CURVE_LAST}+` : `${bucket}`)

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
            className="flex h-full min-w-0 flex-col items-center justify-end gap-0.5"
          >
            <span aria-hidden="true" className="text-label text-muted-foreground tabular-nums">
              {count}
            </span>
            <span
              aria-hidden="true"
              className="w-full rounded-t-sm bg-primary"
              style={{ height: `${Math.round((count / peak) * 62)}%`, minHeight: count > 0 ? 2 : 0 }}
            />
            <span aria-hidden="true" className="w-full border-t pt-0.5 text-center text-label tabular-nums">
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
