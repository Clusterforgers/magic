/**
 * The travel of a batch's moves (docs/ui-design.md, Playing is legible): where each starts and ends on screen, when it
 * flies and how it moves. `planFlights` and `keyframesOf` are pure; `createTravel` measures a move's source when the
 * batch arrives, before React draws it, and its destination on the next frame, and hands `FxTravel` the flights to draw.
 * Nothing here holds game state or waits for the table.
 */
import { createStore } from 'zustand/vanilla'
import type { StoreApi } from 'zustand/vanilla'
import type { ObjectId, PlayerId } from '@/protocol/generated'
import { MAX_MOVES, MOVE_MS } from './moves'
import type { Move } from './moves'
import type { MarkFx } from './model'
import { measureHandCard, measurePlace, measureStrip } from './places'
import type { Spot } from './places'
import { planSpotlight } from './spotlight'
import type { Spotlight } from './spotlight'
import type { FxMode } from './store'

/** A picture's size in px. */
export interface Size {
  readonly width: number
  readonly height: number
}

/** The size of a card picture when neither end of a move is a card on the table. */
export const REFERENCE_CARD: Size = { width: 100, height: 140 }

/** The smallest a picture shrinks to, as a share of its size, when it lands on a chip. */
export const MIN_SCALE = 0.15

/** The easing of a leg: `--ease-out` of `index.css`. */
export const EASE_OUT = 'cubic-bezier(0.2, 0, 0, 1)'

/** One leg of a move, measured and timed, ready to fly. */
export interface Flight {
  /** Unique to the batch and the move. */
  readonly key: string
  readonly move: Move
  readonly from: Spot
  readonly to: Spot
  /** The picture's size; null for an ability's chip, which the stylesheet sizes. */
  readonly size: Size | null
  /** It starts as a card back (a card the viewer may not identify). */
  readonly faceDown: boolean
  /** It turns face up on its way in (a card cast from another seat's hand). */
  readonly reveal: boolean
  /** The badge it carries in flight: a spell that resolves, is countered or fizzles says so on its way out. */
  readonly mark: MarkFx | null
  /** Milliseconds after the batch's first leg, stretched by the slow-down. */
  readonly delay: number
  /** Milliseconds, stretched by the slow-down. */
  readonly duration: number
}

/** What is flying now, and the spell that stands large over the strip. Replaced, never changed. */
export interface TravelState {
  readonly flights: readonly Flight[]
  readonly spotlight: Spotlight | null
}

/** Whether a move carries an ability's chip rather than a card's picture. */
export const isChip = (move: Move): boolean => move.kind === 'activated' || move.kind === 'triggered'

/** Whether a move starts in a hand or library the viewer cannot see into. */
function fromUnseen(move: Move, viewer: PlayerId): boolean {
  const { from } = move
  return !('stack' in from) && from.seat !== viewer && (from.zone === 'hand' || from.zone === 'library')
}

/** The size of the picture of a leg: that of the card it starts or ends on, else the reference. */
function sizeOf(from: Spot, to: Spot): Size {
  const card = from.card ? from.box : to.card ? to.box : REFERENCE_CARD
  return { width: card.width, height: card.height }
}

/** What `planFlights` reads. */
export interface PlanInput {
  readonly moves: readonly Move[]
  readonly viewer: PlayerId
  /** Where each move starts and ends, by position in `moves`; null for a place that is not on screen. */
  readonly starts: readonly (Spot | null)[]
  readonly ends: readonly (Spot | null)[]
  /** Names the batch, so the keys of two batches never meet. */
  readonly batch: number
  /** The dev slow-down. */
  readonly slow?: number
}

/**
 * The flights of a batch. At most `MAX_MOVES`, in order, `MOVE_MS.stagger` apart; a move whose source or destination is
 * not on screen is left out. A card that flies twice (cast and resolved in one batch) starts its second leg when the
 * first has landed, and when it stood in the spotlight between them, when that has gone.
 */
export function planFlights({ moves, viewer, starts, ends, batch, slow = 1 }: PlanInput): Flight[] {
  const landed = new Map<ObjectId, number>()
  const flights: Flight[] = []
  moves.slice(0, MAX_MOVES).forEach((move, index) => {
    const from = starts[index]
    const to = ends[index]
    if (from == null || to == null || (from.box.x === to.box.x && from.box.y === to.box.y)) return
    const delay = Math.max(
      move.order * MOVE_MS.stagger,
      move.object === null ? 0 : (landed.get(move.object) ?? 0),
    )
    const gap = move.kind === 'cast' && move.spotlight ? MOVE_MS.spotlight : 0
    if (move.object !== null) landed.set(move.object, delay + MOVE_MS.travel + gap)
    const faceDown = move.look === null || fromUnseen(move, viewer)
    flights.push({
      key: `${batch}:${move.order}`,
      move,
      from,
      to,
      size: isChip(move) ? null : sizeOf(from, to),
      faceDown,
      reveal: faceDown && move.look !== null,
      mark:
        move.kind === 'resolved' || move.kind === 'countered' || move.kind === 'fizzled' ? move.kind : null,
      delay: delay * slow,
      duration: MOVE_MS.travel * slow,
    })
  })
  return flights
}

const center = (spot: Spot): { x: number; y: number } => ({
  x: spot.box.x + spot.box.width / 2,
  y: spot.box.y + spot.box.height / 2,
})

/** Where a leg's picture is along its way: its centre and the share of its size it is drawn at. */
interface Pose {
  x: number
  y: number
  scale: number
}

const lerp = (a: number, b: number, t: number): number => a + (b - a) * t

/** The share of its size a picture takes at a place: exactly the card's, else what fits the chip it lands on. */
function scaleAt(spot: Spot, size: Size): number {
  if (spot.card) return spot.box.width / size.width
  const fit = Math.min(spot.box.width / size.width, spot.box.height / size.height)
  return Math.min(1, Math.max(MIN_SCALE, fit))
}

/** The point of the way after which the picture fades out, settled on what the table already shows. */
const FADE_AT = 0.9

/** `at` kept inside 0 to `limit` for a length of `length`, as far as the length fits. */
const within = (at: number, length: number, limit: number | undefined): number =>
  limit === undefined ? at : Math.max(0, Math.min(at, limit - length))

/** The share of its size a picture dips to as it leaves: the wind-up before the flight. */
export const WINDUP_SCALE = 0.94

/** The share past its landing size a picture overshoots before it settles: the follow-through. */
export const LAND_OVERSHOOT = 1.04

/** How far the arc bows off the straight line, as a share of the way, and its least and greatest bow in px. */
export const ARC_SHARE = 0.12
export const ARC_MIN_PX = 24
export const ARC_MAX_PX = 96

/** Slow in and slow out: the leg eases out of its wind-up, speeds up to the top of the arc and slows to land. */
const EASE_IN = 'cubic-bezier(0.4, 0, 1, 1)'
const EASE_LAND = 'cubic-bezier(0, 0, 0.2, 1)'
const EASE_SETTLE = 'cubic-bezier(0.45, 0, 0.55, 1)'

/**
 * The poses of a leg, pose to pose: it dips as it winds up, is whole at the top of its arc, lands a little large and
 * settles to the size of where it lands, then fades. Each pose eases into the next by its own curve, so the leg's
 * timing is linear.
 */
const POSES = [
  { offset: 0, easing: EASE_SETTLE },
  { offset: 0.15, easing: EASE_IN },
  { offset: 0.5, easing: EASE_LAND },
  { offset: 0.8, easing: EASE_SETTLE },
  { offset: FADE_AT, easing: 'linear' },
  { offset: 1, easing: 'linear' },
] as const

/** The easing of a whole leg: linear, as the poses carry their own. */
export const LEG_EASING = 'linear'

/** The bow of the arc between two points: square to the way, upward (or rightward on a level way), 0 when they meet. */
export function arcBow(a: { x: number; y: number }, b: { x: number; y: number }): { x: number; y: number } {
  const dx = b.x - a.x
  const dy = b.y - a.y
  const distance = Math.hypot(dx, dy)
  if (distance < 1) return { x: 0, y: 0 }
  const bow = Math.min(ARC_MAX_PX, Math.max(ARC_MIN_PX, distance * ARC_SHARE))
  let nx = -dy / distance
  let ny = dx / distance
  if (ny > 0 || (ny === 0 && nx < 0)) {
    nx = -nx
    ny = -ny
  }
  return { x: nx * bow, y: ny * bow }
}

/**
 * The keyframes of a leg for a picture of `size` (an ability's chip is measured by the caller): an arc from the middle
 * of the source to the middle of the destination, the picture taking the size of each end and whole at the top of the
 * arc, where it can be read. It winds up, lands with a small overshoot, settles and fades on what the table already
 * shows. The picture stays inside `bounds` (the window) when they are given, so a source at the edge of the screen does
 * not draw it half off.
 */
export function keyframesOf(flight: Flight, size: Size, bounds?: Size): Keyframe[] {
  const { from, to } = flight
  const chip = flight.size === null
  const a = center(from)
  const b = center(to)
  const bow = arcBow(a, b)
  const startScale = chip ? 1 : scaleAt(from, size)
  const endScale = chip ? 1 : scaleAt(to, size)
  const poses: Pose[] = [
    { ...a, scale: startScale },
    { ...a, scale: startScale * WINDUP_SCALE },
    { x: lerp(a.x, b.x, 0.5) + bow.x, y: lerp(a.y, b.y, 0.5) + bow.y, scale: 1 },
    { ...b, scale: endScale * LAND_OVERSHOOT },
    { ...b, scale: endScale },
    { ...b, scale: endScale },
  ]
  return POSES.map(({ offset, easing }, at) => {
    const { x, y, scale } = poses[at] ?? { ...b, scale: endScale }
    // The origin is the top left corner, so the corner goes where the scaled picture's centre minus half of it is.
    const left = within(x - (size.width * scale) / 2, size.width * scale, bounds?.width)
    const top = within(y - (size.height * scale) / 2, size.height * scale, bounds?.height)
    return {
      offset,
      easing,
      transform: `translate(${left}px, ${top}px) scale(${scale})`,
      opacity: offset === 1 ? 0 : 1,
    }
  })
}

/** The keyframes of the face turning up over the back: hidden past the top of the arc and up by the landing. */
export const REVEAL_KEYFRAMES: Keyframe[] = [
  { offset: 0, opacity: 0 },
  { offset: 0.6, opacity: 0 },
  { offset: 0.8, opacity: 1 },
  { offset: 1, opacity: 1 },
]

/** Calls `callback` on the next frame; returns the function that cancels it. */
export type FrameScheduler = (callback: () => void) => () => void

/** The browser's next animation frame. */
export const browserFrame: FrameScheduler = (callback) => {
  const id = window.requestAnimationFrame(callback)
  return () => window.cancelAnimationFrame(id)
}

/** What `createTravel` can be given; tests replace the page, the frame and the mode. */
export interface TravelOptions {
  /** What to look for places in; the document by default. */
  root?: ParentNode
  /** Read at every batch: nothing flies unless this says `full`. */
  mode: () => FxMode
  /** The next frame; the browser's by default. */
  frame?: FrameScheduler
  /** Multiplies every duration; 1 by default. */
  slow?: number
}

/** The travel layer of one table. */
export interface Travel {
  /** The flights playing; read through `useFlights`. */
  readonly store: StoreApi<TravelState>
  /**
   * Starts the moves of a batch that has just arrived: measures where each starts now, while the table still shows the
   * game before it, and where it ends on the next frame. A batch with moves ends the flights of the one before.
   */
  launch(moves: readonly Move[], viewer: PlayerId): void
  /** A flight has landed: take it off the layer. */
  done(key: string): void
  /** Ends every flight now and drops what is waiting for its frame. */
  clear(): void
}

/** A travel layer with nothing flying. */
export function createTravel(options: TravelOptions): Travel {
  const frame = options.frame ?? browserFrame
  const slow = options.slow ?? 1
  const store = createStore<TravelState>(() => ({ flights: [], spotlight: null }))
  let cancelFrame: (() => void) | null = null
  let batches = 0

  function clear(): void {
    cancelFrame?.()
    cancelFrame = null
    const { flights, spotlight } = store.getState()
    if (flights.length > 0 || spotlight !== null) store.setState({ flights: [], spotlight: null })
  }

  function launch(moves: readonly Move[], viewer: PlayerId): void {
    const mode = options.mode()
    if (moves.length === 0 || mode === 'off') return
    clear()
    const root = options.root ?? document
    const picked = moves.slice(0, MAX_MOVES)
    batches += 1
    const batch = batches
    // Under reduced motion nothing flies, but the spell still stands for as long as a still card would.
    const spotlight = planSpotlight({
      moves: picked,
      strip: measureStrip(root),
      hand: measureHandCard(root, viewer),
      window: { width: window.innerWidth, height: window.innerHeight },
      viewer,
      batch,
      still: mode !== 'full',
      slow,
    })
    if (mode !== 'full') {
      if (spotlight !== null) store.setState({ flights: [], spotlight })
      return
    }
    const starts = picked.map((move) => measurePlace(root, move.from))
    if (spotlight === null && starts.every((start) => start === null)) return
    cancelFrame = frame(() => {
      cancelFrame = null
      const ends = picked.map((move) => measurePlace(root, move.to))
      const flights = planFlights({ moves: picked, viewer, starts, ends, batch, slow })
      if (flights.length > 0 || spotlight !== null) store.setState({ flights, spotlight })
    })
  }

  function done(key: string): void {
    const { flights, spotlight } = store.getState()
    const kept = flights.filter((flight) => flight.key !== key)
    if (kept.length !== flights.length) store.setState({ flights: kept })
    else if (spotlight?.key === key) store.setState({ spotlight: null })
  }

  return { store, launch, done, clear }
}
