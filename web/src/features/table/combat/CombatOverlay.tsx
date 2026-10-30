import { useContext, useMemo, useRef } from 'react'
import { cn } from '@/lib/utils'
import { anchorKey, anchorsOf } from './plan'
import type { LineSpec } from './plan'
import { RectProviderContext, shapesOf, useAnchorRects } from './rects'
import type { DrawnLine } from './rects'
import { useCombatPlan } from './useCombatPlan'
import { useStackArrows } from './useStackArrows'
import './combat.css'

interface LayerProps {
  drawn: readonly DrawnLine[]
  layer: 'lines' | 'ghost' | 'stack'
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
          data-from={anchorKey(line.from)}
          data-to={anchorKey(line.to)}
          data-kind={line.kind}
          data-pending={line.pending || undefined}
          data-outcome={line.outcome}
          data-mine={line.mine || undefined}
        >
          {/* One unit is the whole line, which is what lets the draw-in work on any length; a dashed line keeps its
              pixels, so it is left alone. */}
          <path className="combat-halo" d={shape.d} pathLength={line.pending ? undefined : 1} />
          <path className="combat-line" d={shape.d} pathLength={line.pending ? undefined : 1} />
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
  const combat = useMemo(() => drawn.filter(({ line }) => line.kind !== 'target'), [drawn])
  const targets = useMemo(() => drawn.filter(({ line }) => line.kind === 'target'), [drawn])
  return (
    <div
      ref={origin}
      data-slot="combat-overlay"
      aria-hidden="true"
      className="pointer-events-none absolute inset-0"
    >
      <Layer drawn={combat} layer="lines" className="combat-overlay z-10" />
      {/* The same lines again, faint, above the centre strip that hides the first set: a line dives under the prompt
          and is still there to follow on the other side, without a stroke across the words. */}
      <Layer drawn={combat} layer="ghost" className="combat-ghost z-30 opacity-30" />
      {/* The arrows of the stack start at a chip in the strip, so they run above it (and under the hand and the controls). */}
      {targets.length === 0 ? null : <Layer drawn={targets} layer="stack" className="combat-stack z-[25]" />}
    </div>
  )
}

/**
 * Lines over the table: from each declared attacker to the seat or planeswalker it attacks (muted), from each blocker
 * to the attacker it blocks (strong, coloured by what the forecast says happens to the two), and the same for the
 * choices a prompt has not sent yet (dashed), and from the stack item on show to its targets (`stackArrows.ts`).
 * Decoration only: hidden from assistive technology, never takes the
 * pointer, and mounted only while there is a line to draw. Its parent must be positioned (`relative`).
 */
export function CombatOverlay() {
  const combat = useCombatPlan()
  const stack = useStackArrows()
  const lines = useMemo(() => (stack.length === 0 ? combat : [...combat, ...stack]), [combat, stack])
  return lines.length === 0 ? null : <Lines lines={lines} />
}
