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
  if (anchor.kind === 'spot') return []
  // An item folded into a group or past the strip's room is drawn by the chip that holds it.
  if (anchor.kind === 'stack')
    return [`[data-stack-chip="${anchor.id}"]`, `[data-stack-members~="${anchor.id}"]`]
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
      if (anchor.kind === 'spot') {
        const { x, y, width, height } = anchor.box
        found.set(anchorKey(anchor), { x: x - base.left, y: y - base.top, width, height })
        continue
      }
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
  /** For a bundle of attacks: how many attackers it stands for. */
  count?: number
}

/** A seat that attacks with more creatures than this has its attacks bundled by target; fewer read one by one. */
export const BUNDLE_OVER = 5

/** The smallest box around `boxes`. */
function around(boxes: readonly Rect[]): Rect | undefined {
  if (boxes.length === 0) return undefined
  const left = Math.min(...boxes.map((box) => box.x))
  const top = Math.min(...boxes.map((box) => box.y))
  const right = Math.max(...boxes.map((box) => box.x + box.width))
  const bottom = Math.max(...boxes.map((box) => box.y + box.height))
  return { x: left, y: top, width: right - left, height: bottom - top }
}

/** The attacking seat of an attack, sent or still chosen apart, or null for any other line. */
const attackerOf = (line: LineSpec): string | null =>
  line.kind === 'attack' && line.from.kind === 'card' && line.from.seat !== undefined
    ? `${line.pending ? 'pending-' : ''}bundle:${line.from.seat}`
    : null

/** What folds attacks into one bundle: the same seat, the same target, both sent or both still chosen. */
const bundleOf = (line: LineSpec): string | null => {
  const seat = attackerOf(line)
  return seat === null ? null : `${seat}>${anchorKey(line.to)}`
}

/**
 * The lines with the attacks of a seat that attacks with more than `BUNDLE_OVER` creatures folded by target: two or
 * more at one target make one line, from the box around the attackers on screen to the target, with their count, in
 * the place of the first of them.
 */
export function bundled(
  lines: readonly LineSpec[],
  rects: RectMap,
): { line: LineSpec; from: Rect | undefined; count?: number }[] {
  const members = new Map<string, LineSpec[]>()
  const seats = new Map<string, number>()
  for (const line of lines) {
    const key = bundleOf(line)
    const seat = attackerOf(line)
    if (key !== null) members.set(key, [...(members.get(key) ?? []), line])
    if (seat !== null) seats.set(seat, (seats.get(seat) ?? 0) + 1)
  }
  const out: { line: LineSpec; from: Rect | undefined; count?: number }[] = []
  for (const line of lines) {
    const key = bundleOf(line)
    const group = key === null ? undefined : members.get(key)
    const crowd = (seats.get(attackerOf(line) ?? '') ?? 0) > BUNDLE_OVER
    if (key === null || group === undefined || group.length < 2 || !crowd) {
      out.push({ line, from: rects.get(anchorKey(line.from)) })
      continue
    }
    if (group[0] !== line) continue
    const boxes = group.flatMap((one) => {
      const box = rects.get(anchorKey(one.from))
      return box === undefined ? [] : [box]
    })
    out.push({ line: { ...line, key }, from: around(boxes), count: group.length })
  }
  return out
}

/**
 * The lines whose two ends are on screen and have room between them, with the attacks of a crowd bundled
 * (`bundled`). Lines that would be drawn on top of each other (several attackers of a seat that is not drawn, all
 * standing in as its tile) are drawn once.
 */
export function shapesOf(lines: readonly LineSpec[], rects: RectMap): DrawnLine[] {
  const seen = new Set<string>()
  const drawn: DrawnLine[] = []
  for (const { line, from, count } of bundled(lines, rects)) {
    const to = rects.get(anchorKey(line.to))
    if (from === undefined || to === undefined) continue
    const curve =
      line.kind === 'attack'
        ? { bow: 0.12, head: 8 }
        : line.kind === 'attach'
          ? ATTACH_CURVE
          : // A block runs from the attacker to the blocker, the way an attack runs the other way round, so it bows to
            // the other side of its travel: both bow the same way on the screen, clear of the pod column at the left.
            { bow: -0.1, head: 10 }
    const shape = connect(from, to, curve)
    if (shape === null) continue
    const same = `${shape.d}|${line.kind}|${line.pending}|${line.outcome ?? ''}`
    if (seen.has(same)) continue
    seen.add(same)
    drawn.push(count === undefined ? { line, shape } : { line, shape, count })
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
