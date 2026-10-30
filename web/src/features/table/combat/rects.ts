/**
 * Where the anchors of the overlay are on screen. The DOM is the only source (cards carry `data-object-id`, seat panels
 * `data-seat`, tiles `data-tile-seat`), measured with `getBoundingClientRect` and kept in the overlay's own
 * coordinates. A table test supplies its own `RectProvider`, since jsdom has no layout.
 */
import { createContext, useLayoutEffect, useRef, useState } from 'react'
import type { RefObject } from 'react'
import { connect } from './geometry'
import type { Rect, Shape } from './geometry'
import { anchorKey } from './plan'
import type { Anchor, LineSpec } from './plan'

/** Boxes by `anchorKey`; an anchor that is not on screen has none. */
export type RectMap = ReadonlyMap<string, Rect>

/** Measures anchors: the box of each one that is on screen, relative to the top left corner of `origin`. */
export interface RectProvider {
  measure(anchors: readonly Anchor[], origin: Element): RectMap
  /** Where to ask for animation frames when the table moves; the window's when left out. A test supplies its own. */
  frames?: Frames
}

/** Where to look for an anchor, best first. A card that is not drawn is stood in for by its controller's panel or tile. */
export function selectorsOf(anchor: Anchor): string[] {
  const seat = (n: number): string[] => [`[data-seat="${n}"]`, `[data-tile-seat="${n}"]`]
  if (anchor.kind === 'seat') return seat(anchor.seat)
  const card = `button[data-object-id="${anchor.id}"]`
  return anchor.seat === undefined ? [card] : [card, ...seat(anchor.seat)]
}

// A tapped card lies on its side inside its box, so the face (whose bounding box is the card as seen) is measured,
// not the button. Only a card button has a face of its own: a seat panel holds cards, and the first of those is not
// the panel.
const FACE = '[data-slot="card-face"]'
const faceOf = (element: Element): Element =>
  (element.hasAttribute('data-object-id') ? element.querySelector(FACE) : null) ?? element

const clips = (overflow: string): boolean => overflow !== '' && overflow !== 'visible'

/**
 * Whether some of `box`, the box of an element of the table, is drawn: no ancestor below `scope` that scrolls or
 * clips (a board's zone, a section squeezed to no room) cuts all of it off. A card in such a place is in the page but
 * not on the table, so a line must not run to where it would be.
 */
function isDrawn(element: Element, box: DOMRect, scope: ParentNode): boolean {
  let { left, top, right, bottom } = box
  for (
    let parent = element.parentElement;
    parent !== null && parent !== scope;
    parent = parent.parentElement
  ) {
    const { overflowX, overflowY } = getComputedStyle(parent)
    if (!clips(overflowX) && !clips(overflowY)) continue
    const clip = parent.getBoundingClientRect()
    left = Math.max(left, clip.left)
    top = Math.max(top, clip.top)
    right = Math.min(right, clip.right)
    bottom = Math.min(bottom, clip.bottom)
    if (right <= left || bottom <= top) return false
  }
  return true
}

/** The provider that reads the page. */
export const domRects: RectProvider = {
  measure(anchors, origin) {
    const scope: ParentNode = origin.parentElement ?? document
    const base = origin.getBoundingClientRect()
    const found = new Map<string, Rect>()
    for (const anchor of anchors) {
      for (const selector of selectorsOf(anchor)) {
        const element = scope.querySelector(selector)
        if (element === null) continue
        const box = faceOf(element).getBoundingClientRect()
        if (box.width <= 0 || box.height <= 0 || !isDrawn(element, box, scope)) continue
        found.set(anchorKey(anchor), {
          x: box.left - base.left,
          y: box.top - base.top,
          width: box.width,
          height: box.height,
        })
        break
      }
    }
    return found
  },
}

/** The provider `CombatOverlay` uses; a test provides its own around the table. */
export const RectProviderContext = createContext<RectProvider>(domRects)

/** Whether two maps hold the same boxes. */
export function sameRects(a: RectMap, b: RectMap): boolean {
  if (a.size !== b.size) return false
  for (const [key, rect] of a) {
    const other = b.get(key)
    if (
      other === undefined ||
      other.x !== rect.x ||
      other.y !== rect.y ||
      other.width !== rect.width ||
      other.height !== rect.height
    ) {
      return false
    }
  }
  return true
}

/**
 * The curve of an attach arrow. Hosts of a lane stand a few pixels apart, so the arrow starts and ends inside the
 * boxes (a negative gap): from edge to edge there would be no room for it.
 */
const ATTACH_CURVE = { bow: 0.15, head: 10, gap: -20 }

/** A line with the curve that joins its two anchors. */
export interface DrawnLine {
  line: LineSpec
  shape: Shape
}

/**
 * The lines whose two ends are on screen and have room between them. Lines that would be drawn on top of each other
 * (several attackers of a seat that is not drawn, all standing in as its tile) are drawn once.
 */
export function shapesOf(lines: readonly LineSpec[], rects: RectMap): DrawnLine[] {
  const seen = new Set<string>()
  const drawn: DrawnLine[] = []
  for (const line of lines) {
    const from = rects.get(anchorKey(line.from))
    const to = rects.get(anchorKey(line.to))
    if (from === undefined || to === undefined) continue
    const curve =
      line.kind === 'block'
        ? { bow: 0.1, head: 10 }
        : line.kind === 'attach'
          ? ATTACH_CURVE
          : { bow: 0.12, head: 8 }
    const shape = connect(from, to, curve)
    if (shape === null) continue
    const same = `${shape.d}|${line.kind}|${line.pending}|${line.outcome ?? ''}`
    if (seen.has(same)) continue
    seen.add(same)
    drawn.push({ line, shape })
  }
  return drawn
}

/** Where to ask for a frame; the page's by default, a fake in a test. */
export interface Frames {
  request(run: () => void): number
  cancel(handle: number): void
}

/** The animation frames of the window. */
export const windowFrames: Frames = {
  request: (run) => window.requestAnimationFrame(run),
  cancel: (handle) => window.cancelAnimationFrame(handle),
}

/** Runs `work` at most once per frame, however often it is asked. */
export interface Scheduler {
  /** Asks for `work` at the next frame; a request while one is waiting changes nothing. */
  schedule(): void
  /** Forgets a waiting request. */
  stop(): void
}

/** A scheduler that coalesces every request made within a frame into one run of `work`. */
export function createScheduler(work: () => void, frames: Frames): Scheduler {
  let handle: number | null = null
  return {
    schedule() {
      if (handle !== null) return
      handle = frames.request(() => {
        handle = null
        work()
      })
    },
    stop() {
      if (handle === null) return
      frames.cancel(handle)
      handle = null
    },
  }
}

// What can move an anchor without a React render: a card turning, the page reflowing, a scroll inside a row.
const TURNS = ['data-tapped', 'data-size']
const NO_RECTS: RectMap = new Map()

/**
 * The boxes of `anchors`, measured when they first appear and again, once per frame, whenever the table could have
 * moved: a resize of the table or the window, a scroll inside it, cards entering or leaving, a card turning (its
 * transition ends), fonts arriving. Never measured per render.
 */
export function useAnchorRects(
  origin: RefObject<Element | null>,
  anchors: readonly Anchor[],
  provider: RectProvider,
  frames: Frames = provider.frames ?? windowFrames,
): RectMap {
  const [rects, setRects] = useState<RectMap>(NO_RECTS)
  const shown = useRef<RectMap>(NO_RECTS)
  // The effect depends on which anchors there are, not on the array that happens to hold them this render.
  const wanted = anchors.map(anchorKey).join(',')
  const latest = useRef(anchors)
  useLayoutEffect(() => {
    latest.current = anchors
  })
  useLayoutEffect(() => {
    const element = origin.current
    const root = element?.parentElement
    if (element == null || root == null) return
    let live = true
    const measure = (): void => {
      const next = provider.measure(latest.current, element)
      // Compared here, not in a state updater, which can still cost a render when nothing changed.
      if (sameRects(shown.current, next)) return
      shown.current = next
      setRects(next)
    }
    measure()
    const scheduler = createScheduler(measure, frames)
    const again = (): void => {
      if (live) scheduler.schedule()
    }
    const resizes = typeof ResizeObserver === 'function' ? new ResizeObserver(again) : null
    resizes?.observe(root)
    // The lines are drawn inside `element`; only a change elsewhere can move an anchor.
    const changes = new MutationObserver((records) => {
      if (records.some((record) => !element.contains(record.target))) again()
    })
    changes.observe(root, { childList: true, subtree: true, attributes: true, attributeFilter: TURNS })
    // Scroll does not bubble, so a scroll inside any row of the table is caught on the way down.
    root.addEventListener('scroll', again, { capture: true, passive: true })
    root.addEventListener('transitionend', again)
    window.addEventListener('resize', again)
    void document.fonts?.ready.then(again)
    return () => {
      live = false
      scheduler.stop()
      resizes?.disconnect()
      changes.disconnect()
      root.removeEventListener('scroll', again, { capture: true })
      root.removeEventListener('transitionend', again)
      window.removeEventListener('resize', again)
    }
  }, [origin, wanted, provider, frames])
  return rects
}
