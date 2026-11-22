import { useState } from 'react'
import type { KeyboardEvent, PointerEvent } from 'react'
import type { SupportPoint } from '@/protocol/generated'
import { count, percent, shortDate, trendPoints } from './model'

/**
 * The share that plays automatically over the stored snapshots: one 2 px line from the lowest share (rounded down) to
 * 100%, a dot on the newest, and a crosshair with the snapshot's numbers under a pointer or the arrow keys. The same
 * numbers are a table under "Show as table".
 */
export function TrendChart({ history }: { history: readonly SupportPoint[] }) {
  const { points, floor } = trendPoints(history, 100, 100)
  const [active, setActive] = useState<number | null>(null)
  if (points.length === 0) return null
  const shown = active === null ? null : points[active]
  const last = points[points.length - 1]
  const line = points.map((p) => `${p.x.toFixed(2)},${p.y.toFixed(2)}`).join(' ')

  const nearest = (event: PointerEvent<HTMLDivElement>) => {
    const box = event.currentTarget.getBoundingClientRect()
    const x = ((event.clientX - box.left) / Math.max(1, box.width)) * 100
    let best = 0
    points.forEach((p, at) => {
      if (Math.abs(p.x - x) < Math.abs((points[best]?.x ?? 0) - x)) best = at
    })
    setActive(best)
  }
  const onKey = (event: KeyboardEvent<HTMLDivElement>) => {
    const at = active ?? points.length - 1
    if (event.key === 'ArrowLeft') setActive(Math.max(0, at - 1))
    else if (event.key === 'ArrowRight') setActive(Math.min(points.length - 1, at + 1))
    else if (event.key === 'Escape') setActive(null)
    else return
    event.preventDefault()
  }
  const first = points[0]

  return (
    <figure className="grid content-start gap-2" aria-labelledby="trend-title">
      <figcaption id="trend-title" className="text-sm font-medium">
        Share that plays automatically
        <span className="font-normal text-muted-foreground">{` · ${points.length} ${points.length === 1 ? 'snapshot' : 'snapshots'}`}</span>
      </figcaption>
      <div className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-2">
        <div
          className="flex h-36 flex-col justify-between text-xs text-muted-foreground tabular-nums"
          aria-hidden="true"
        >
          <span>100%</span>
          <span>{`${Math.round(floor * 100)}%`}</span>
        </div>
        <div
          role="img"
          tabIndex={0}
          aria-label={`From ${percent(first?.point.automatic ?? 0, first?.point.cards ?? 0)} on ${shortDate(first?.point.taken_at ?? '')} to ${percent(last?.point.automatic ?? 0, last?.point.cards ?? 0)} on ${shortDate(last?.point.taken_at ?? '')}. Arrow keys step through the snapshots.`}
          data-slot="trend"
          className="relative h-36 touch-none rounded-lg border border-border bg-table/40"
          onPointerMove={nearest}
          onPointerLeave={() => setActive(null)}
          onKeyDown={onKey}
          onBlur={() => setActive(null)}
        >
          <svg
            viewBox="0 0 100 100"
            preserveAspectRatio="none"
            className="absolute inset-0 size-full overflow-visible"
            aria-hidden="true"
          >
            <line
              x1="0"
              x2="100"
              y1="50"
              y2="50"
              className="stroke-border"
              strokeWidth="1"
              vectorEffect="non-scaling-stroke"
              strokeDasharray="2 4"
            />
            <polyline
              points={line}
              fill="none"
              className="stroke-support-verified"
              strokeWidth="2"
              strokeLinejoin="round"
              strokeLinecap="round"
              vectorEffect="non-scaling-stroke"
            />
            {shown ? (
              <line
                x1={shown.x}
                x2={shown.x}
                y1="0"
                y2="100"
                className="stroke-muted-foreground"
                strokeWidth="1"
                vectorEffect="non-scaling-stroke"
              />
            ) : null}
          </svg>
          {[shown ?? last].map((p) =>
            p ? (
              <span
                key="dot"
                aria-hidden="true"
                className="absolute size-2.5 -translate-x-1/2 -translate-y-1/2 rounded-full bg-support-verified ring-2 ring-surface"
                style={{ left: `${p.x}%`, top: `${p.y}%` }}
              />
            ) : null,
          )}
          {shown ? (
            <div
              role="status"
              data-slot="trend-tip"
              className="pointer-events-none absolute top-1 z-10 grid gap-0.5 rounded-lg border border-border bg-raised px-2 py-1 text-xs shadow-e2"
              style={shown.x > 60 ? { right: `${100 - shown.x}%` } : { left: `${shown.x}%` }}
            >
              <span className="font-semibold tabular-nums">
                {percent(shown.point.automatic, shown.point.cards)}
              </span>
              <span className="text-muted-foreground tabular-nums">
                {`${count(shown.point.automatic)} of ${count(shown.point.cards)}`}
              </span>
              <span className="text-muted-foreground">
                {`${shortDate(shown.point.taken_at)}${shown.point.release ? ` · ${shown.point.release}` : ''}`}
              </span>
            </div>
          ) : null}
        </div>
        <span />
        <div className="flex justify-between text-xs text-muted-foreground" aria-hidden="true">
          <span>{shortDate(first?.point.taken_at ?? '')}</span>
          <span>{shortDate(last?.point.taken_at ?? '')}</span>
        </div>
      </div>
      <details className="text-sm">
        <summary className="cursor-pointer text-muted-foreground hover:text-foreground">
          Show as table
        </summary>
        <div className="mt-2 max-h-48 overflow-auto">
          <table className="w-full text-left text-xs tabular-nums">
            <thead className="text-muted-foreground">
              <tr>
                <th className="py-1 font-medium">Date</th>
                <th className="py-1 font-medium">Release</th>
                <th className="py-1 text-right font-medium">Automatic</th>
                <th className="py-1 text-right font-medium">Share</th>
              </tr>
            </thead>
            <tbody>
              {[...points].reverse().map(({ point }) => (
                <tr key={point.taken_at} className="border-t border-border">
                  <td className="py-1">{shortDate(point.taken_at)}</td>
                  <td className="py-1">{point.release ?? '–'}</td>
                  <td className="py-1 text-right">{count(point.automatic)}</td>
                  <td className="py-1 text-right">{percent(point.automatic, point.cards)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </details>
    </figure>
  )
}
