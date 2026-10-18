import { useContext, useMemo, useRef } from 'react'
import { cn } from '@/lib/utils'
import { anchorsOf } from './plan'
import type { LineSpec } from './plan'
import { RectProviderContext, shapesOf, useAnchorRects } from './rects'
import type { DrawnLine } from './rects'
import { useCombatPlan } from './useCombatPlan'
import './combat.css'

interface LayerProps {
  drawn: readonly DrawnLine[]
  layer: 'lines' | 'ghost'
  className: string
}

function Layer({ drawn, layer, className }: LayerProps) {
  return (
    <svg
      data-layer={layer}
      focusable="false"
      className={cn('absolute inset-0 size-full overflow-visible', className)}
    >
      {drawn.map(({ line, shape }) => (
        <g
          key={line.key}
          className="combat-group"
          data-line={line.key}
          data-kind={line.kind}
          data-pending={line.pending || undefined}
          data-outcome={line.outcome}
        >
          <path className="combat-halo" d={shape.d} pathLength={1} />
          <path className="combat-line" d={shape.d} pathLength={1} />
          <path className="combat-head" d={shape.head} />
        </g>
      ))}
    </svg>
  )
}

function Lines({ lines }: { lines: readonly LineSpec[] }) {
  const origin = useRef<HTMLDivElement>(null)
  const provider = useContext(RectProviderContext)
  const anchors = useMemo(() => anchorsOf(lines), [lines])
  const rects = useAnchorRects(origin, anchors, provider)
  const drawn = useMemo(() => shapesOf(lines, rects), [lines, rects])
  return (
    <div
      ref={origin}
      data-slot="combat-overlay"
      aria-hidden="true"
      className="pointer-events-none absolute inset-0"
    >
      <Layer drawn={drawn} layer="lines" className="combat-overlay z-10" />
      {/* The same lines again, faint, above the centre strip that hides the first set: a line dives under the prompt
          and is still there to follow on the other side, without a stroke across the words. */}
      <Layer drawn={drawn} layer="ghost" className="combat-ghost z-30 opacity-30" />
    </div>
  )
}

/**
 * Lines over the table: from each declared attacker to the seat or planeswalker it attacks (muted), from each blocker
 * to the attacker it blocks (strong, coloured by what the forecast says happens to the two), and the same for the
 * choices a prompt has not sent yet (dashed). Decoration only: hidden from assistive technology, never takes the
 * pointer, and mounted only while there is a line to draw. Its parent must be positioned (`relative`).
 */
export function CombatOverlay() {
  const lines = useCombatPlan()
  return lines.length === 0 ? null : <Lines lines={lines} />
}
