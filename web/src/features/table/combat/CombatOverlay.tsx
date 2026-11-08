import { useContext, useLayoutEffect, useMemo, useRef } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import { useSeatNamer } from '../names'
import { useAttachLines } from './attachLines'
import { clearOf, labelPoint, shiftInto } from './geometry'
import type { Rect } from './geometry'
import { useEffectLines } from './effectLines'
import { anchorKey, anchorsOf } from './plan'
import type { LineSpec } from './plan'
import { RectProviderContext, shapesOf, useAnchorRects } from './rects'
import type { DrawnLine } from './rects'
import { useCombatPlan } from './useCombatPlan'
import { useStackArrows } from './useStackArrows'
import './combat.css'
import './attach.css'

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
      {drawn.map(({ line, shape, count }) => (
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
          data-count={count}
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

/**
 * The count of each bundle of attacks ("4 attackers → Cy"), on a pill near its arrowhead, above the strip so that no
 * prompt hides it. Bundles come only from a seat that attacks with a crowd (`BUNDLE_OVER`), one per target.
 */
function BundleLabels({ drawn }: { drawn: readonly DrawnLine[] }) {
  const seat = useSeatNamer()
  const viewer = usePlay((state) => state.seat)
  const objects = usePlay((state) => state.objectsById)
  const bundles = drawn.filter((entry) => entry.count !== undefined)
  const layer = useRef<HTMLDivElement>(null)
  // A label steps off the shared strip, toward its tip, so it covers no word of the prompt, and in from the edge of the
  // table and the window until it is whole.
  useLayoutEffect(() => {
    const element = layer.current
    if (element === null) return
    const outer = element.getBoundingClientRect()
    const left = Math.max(outer.left, 0)
    const top = Math.max(outer.top, 0)
    const bounds = {
      x: left,
      y: top,
      width: Math.min(outer.right, window.innerWidth) - left,
      height: Math.min(outer.bottom, window.innerHeight) - top,
    }
    const strip = element.closest('[data-slot="combat-overlay"]')?.parentElement
    const sheet = strip?.querySelector('[data-slot="shared-sheet"]')?.getBoundingClientRect()
    const placed: Rect[] =
      sheet === undefined ? [] : [{ x: sheet.left, y: sheet.top, width: sheet.width, height: sheet.height }]
    for (const label of element.children) {
      if (!(label instanceof HTMLElement)) continue
      label.style.margin = ''
      const seen = label.getBoundingClientRect()
      const tip = outer.top + Number(label.dataset.tipY ?? 0)
      // In from the sides first, then off the strip and the labels already placed, in the order they were placed.
      const raw: Rect = { x: seen.left, y: seen.top, width: seen.width, height: seen.height }
      let box: Rect = { ...raw, x: raw.x + shiftInto(raw, bounds).x }
      for (const other of placed) {
        const toward = { ...box, y: box.y + clearOf(box, other, tip) }
        const inside = toward.y >= bounds.y && toward.y + toward.height <= bounds.y + bounds.height
        // Where the way to the tip runs out of the table, the label steps off the other way.
        box = inside ? toward : { ...box, y: box.y + clearOf(box, other, tip < box.y ? Infinity : -Infinity) }
      }
      const shift = shiftInto(box, bounds)
      box = { ...box, x: box.x + shift.x, y: box.y + shift.y }
      placed.push(box)
      const x = box.x - seen.left
      const y = box.y - seen.top
      if (x !== 0 || y !== 0) label.style.margin = `${y}px 0 0 ${x}px`
    }
  })
  if (bundles.length === 0) return null
  return (
    <div ref={layer} data-layer="bundle-labels" className="absolute inset-0 z-30">
      {bundles.map(({ line, shape, count }) => {
        const to =
          line.to.kind === 'seat'
            ? line.to.seat === viewer
              ? 'you'
              : seat(line.to.seat)
            : line.to.kind === 'card'
              ? (objects[line.to.id]?.name ?? 'a card')
              : ''
        const at = labelPoint(shape)
        return (
          <span
            key={line.key}
            data-bundle-label={line.key}
            data-tip-y={Math.round(shape.end.y)}
            style={{ left: at.x, top: at.y }}
            className="absolute -translate-x-1/2 -translate-y-1/2 rounded-full border border-border-strong bg-card px-2 py-0.5 text-xs font-semibold whitespace-nowrap text-foreground shadow-sm"
          >
            {count} attackers → {to}
          </span>
        )
      })}
    </div>
  )
}

/** The lines that run above the strip: they start at a chip in it. */
const above = (kind: LineSpec['kind']): boolean => kind === 'target' || kind === 'effect'

function Lines({ lines }: { lines: readonly LineSpec[] }) {
  const origin = useRef<HTMLDivElement>(null)
  const provider = useContext(RectProviderContext)
  const anchors = useMemo(() => anchorsOf(lines), [lines])
  const rects = useAnchorRects(origin, anchors, provider)
  const drawn = useMemo(() => shapesOf(lines, rects), [lines, rects])
  const combat = useMemo(() => drawn.filter(({ line }) => !above(line.kind)), [drawn])
  const targets = useMemo(() => drawn.filter(({ line }) => above(line.kind)), [drawn])
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
      <BundleLabels drawn={combat} />
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
  const moved = useAttachLines()
  const effects = useEffectLines()
  const lines = useMemo(
    () =>
      stack.length === 0 && moved.length === 0 && effects.length === 0
        ? combat
        : [...combat, ...stack, ...moved, ...effects],
    [combat, stack, moved, effects],
  )
  return lines.length === 0 ? null : <Lines lines={lines} />
}
