/** A fixed layout for the combat overlay, since jsdom has none: a rect provider that gives every anchor a box. */
import { vi } from 'vitest'
import type { Rect } from '@/features/table/combat/geometry'
import { anchorKey } from '@/features/table/combat/plan'
import type { Anchor } from '@/features/table/combat/plan'
import type { RectMap, RectProvider } from '@/features/table/combat/rects'
import { fakeFrames } from './frames'
import type { FakeFrames } from './frames'

/**
 * Where an anchor is: cards sit on a grid by id, 20 to a row, with room between them; seats are wide panels far below
 * every card, one under the other, so no two boxes overlap and every line has space to be drawn in.
 */
export function boxOf(anchor: Anchor): Rect {
  if (anchor.kind === 'seat') return { x: 0, y: 10_000 + 300 * anchor.seat, width: 1000, height: 200 }
  return { x: 20 + (anchor.id % 20) * 60, y: 100 + Math.floor(anchor.id / 20) * 150, width: 50, height: 70 }
}

/** The provider, a spy on how often it measured, and the frames the overlay asks for. */
export interface FakeRects extends FakeFrames {
  provider: RectProvider
  measure: ReturnType<typeof vi.fn<RectProvider['measure']>>
  /** The anchors the most recent measurement was asked about. */
  lastAnchors(): readonly Anchor[]
  /** Anchors left out of every measurement, as cards that are not on screen are. */
  hide(...anchors: Anchor[]): void
}

/** A fresh provider with its own frames; pass `layout` to put anchors somewhere else than `boxOf` does. */
export function fakeRects(layout: (anchor: Anchor) => Rect = boxOf): FakeRects {
  const frames = fakeFrames()
  const hidden = new Set<string>()
  const measure = vi.fn<RectProvider['measure']>(
    (anchors): RectMap =>
      new Map(
        anchors
          .filter((anchor) => !hidden.has(anchorKey(anchor)))
          .map((anchor) => [anchorKey(anchor), layout(anchor)]),
      ),
  )
  return {
    ...frames,
    provider: { measure, frames: frames.frames },
    measure,
    lastAnchors: () => measure.mock.calls.at(-1)?.[0] ?? [],
    hide: (...anchors) => anchors.forEach((anchor) => hidden.add(anchorKey(anchor))),
  }
}
