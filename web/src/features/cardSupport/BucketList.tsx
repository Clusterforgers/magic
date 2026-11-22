import type { SupportBucket } from '@/protocol/generated'
import { count, percent } from './model'

/** "top 100" as "Top 100", "all ranked" as "All ranked". */
const label = (text: string): string => text.charAt(0).toUpperCase() + text.slice(1)

/**
 * The share that plays automatically among the most played cards (EDHREC rank), one row per bucket: the label, a bar
 * of the automatic share with the verified part darker, and the numbers.
 */
export function BucketList({ buckets }: { buckets: readonly SupportBucket[] }) {
  if (buckets.length === 0) return null
  return (
    <section aria-labelledby="buckets-title" className="surface grid gap-3 p-4 sm:p-6">
      <div className="grid gap-1">
        <h2 id="buckets-title" className="text-lg font-semibold tracking-tight">
          By popularity
        </h2>
        <p className="text-sm text-muted-foreground">
          The most played cards first, by EDHREC rank. The darker part is verified.
        </p>
      </div>
      <ul className="grid gap-3">
        {buckets.map((bucket) => (
          <li
            key={bucket.label}
            data-bucket={bucket.label}
            className="grid grid-cols-[6rem_minmax(0,1fr)_auto] items-center gap-3 text-sm"
          >
            <span className="truncate">{label(bucket.label)}</span>
            <span aria-hidden="true" className="relative h-2 overflow-hidden rounded-full bg-muted">
              <span
                className="absolute inset-y-0 left-0 rounded-full bg-support-verified/40"
                style={{ width: `${(bucket.automatic / Math.max(1, bucket.cards)) * 100}%` }}
              />
              <span
                className="absolute inset-y-0 left-0 rounded-full bg-support-verified"
                style={{ width: `${(bucket.verified / Math.max(1, bucket.cards)) * 100}%` }}
              />
            </span>
            <span className="text-right tabular-nums">
              <span className="font-medium">{percent(bucket.automatic, bucket.cards)}</span>
              <span className="text-muted-foreground">{` of ${count(bucket.cards)}`}</span>
            </span>
          </li>
        ))}
      </ul>
    </section>
  )
}
