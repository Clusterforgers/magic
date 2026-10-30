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
const mix = (a: Pose, b: Pose, t: number): Pose => ({
  x: lerp(a.x, b.x, t),
  y: lerp(a.y, b.y, t),
  scale: lerp(a.scale, b.scale, t),
})

/** The share of its size a picture takes at a place: exactly the card's, else what fits the chip it lands on. */
function scaleAt(spot: Spot, size: Size): number {
  if (spot.card) return spot.box.width / size.width
  const fit = Math.min(spot.box.width / size.width, spot.box.height / size.height)
  return Math.min(1, Math.max(MIN_SCALE, fit))
}

/** The point of the way after which the picture fades out. */
const FADE_AT = 0.85

/** `at` kept inside 0 to `limit` for a length of `length`, as far as the length fits. */
const within = (at: number, length: number, limit: number | undefined): number =>
  limit === undefined ? at : Math.max(0, Math.min(at, limit - length))

/** The steps of a leg, which the easing runs over: it leaves, is at its fullest halfway and fades as it lands. */
const STEPS = [0, 0.5, FADE_AT, 1] as const

/**
 * The keyframes of a leg for a picture of `size` (an ability's chip is measured by the caller): a straight path from
 * the middle of the source to the middle of the destination, the picture taking the size of each end and whole halfway,
 * where it can be read. It fades in the last part, once it has landed on what the table already shows. The picture
 * stays inside `bounds` (the window) when they are given, so a source at the edge of the screen does not draw it half off.
 */
export function keyframesOf(flight: Flight, size: Size, bounds?: Size): Keyframe[] {
  const { from, to } = flight
  const chip = flight.size === null
  const a = center(from)
  const b = center(to)
  const start: Pose = { ...a, scale: chip ? 1 : scaleAt(from, size) }
  const end: Pose = { ...b, scale: chip ? 1 : scaleAt(to, size) }
  const middle: Pose = { x: lerp(a.x, b.x, 0.5), y: lerp(a.y, b.y, 0.5), scale: 1 }
  const pose = (t: number): Pose =>
    t <= 0.5 ? mix(start, middle, t / 0.5) : mix(middle, end, (t - 0.5) / 0.5)
  return STEPS.map((offset) => {
    const { x, y, scale } = pose(offset)
    // The origin is the top left corner, so the corner goes where the scaled picture's centre minus half of it is.
    const left = within(x - (size.width * scale) / 2, size.width * scale, bounds?.width)
    const top = within(y - (size.height * scale) / 2, size.height * scale, bounds?.height)
    return {
      offset,
      transform: `translate(${left}px, ${top}px) scale(${scale})`,
      opacity: offset === 1 ? 0 : 1,
    }
  })
}

/** The keyframes of the face turning up over the back: hidden until the leg is nearly through. */
export const REVEAL_KEYFRAMES: Keyframe[] = [
  { offset: 0, opacity: 0 },
  { offset: 0.7, opacity: 0 },
  { offset: FADE_AT, opacity: 1 },
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
