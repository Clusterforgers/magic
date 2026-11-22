import { cn } from '@/lib/utils'
import type { SupportSnapshot } from '@/protocol/generated'
import { LOOKS, STATE_ORDER } from './looks'
import { count, percent } from './model'
import { TrendChart } from './TrendChart'
import type { SupportPoint } from '@/protocol/generated'

/**
 * The headline: the share of Commander-legal cards that play automatically as one big number, the states as a bar with
 * a legend that names each with its count, and the trend beside it.
 */
export function Headline({
  snapshot,
  history,
}: {
  snapshot: SupportSnapshot
  history: readonly SupportPoint[]
}) {
  const { states, cards } = snapshot
  return (
    <section
      aria-labelledby="support-share"
      className="surface grid gap-6 p-4 sm:p-6 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]"
    >
      <div className="grid content-start gap-4">
        <div className="grid gap-1">
          <p id="support-share" className="font-display text-display leading-none font-bold tabular-nums">
            {percent(snapshot.automatic, cards)}
          </p>
          <p className="text-base text-muted-foreground">
            {`of ${count(cards)} Commander-legal cards play automatically (${count(snapshot.automatic)}); ${count(snapshot.missing)} still need a hand.`}
          </p>
        </div>
        <div
          aria-hidden="true"
          data-slot="state-bar"
          className="flex h-3 gap-0.5 overflow-hidden rounded-full bg-muted"
        >
          {STATE_ORDER.map((state) =>
            states[state] === 0 ? null : (
              <span
                key={state}
                className={cn('h-full first:rounded-l-full last:rounded-r-full', LOOKS[state].segment)}
                style={{ width: `${(states[state] / Math.max(1, cards)) * 100}%` }}
              />
            ),
          )}
        </div>
        <ul aria-label="Cards by state" className="grid grid-cols-2 gap-x-4 gap-y-2 sm:grid-cols-4">
          {STATE_ORDER.map((state) => {
            const look = LOOKS[state]
            return (
              <li
                key={state}
                data-state={state}
                title={look.meaning}
                className="flex items-center gap-2 text-sm"
              >
                <span
                  aria-hidden="true"
                  className={cn(
                    'inline-grid size-6 shrink-0 place-items-center rounded-full border',
                    look.mark,
                  )}
                >
                  <look.Icon className="size-3.5" />
                </span>
                <span className="min-w-0">
                  <span className="block truncate">{look.word}</span>
                  <span className="block text-xs text-muted-foreground tabular-nums">
                    {`${count(states[state])} · ${percent(states[state], cards)}`}
                  </span>
                </span>
              </li>
            )
          })}
        </ul>
      </div>
      <TrendChart history={history} />
    </section>
  )
}
