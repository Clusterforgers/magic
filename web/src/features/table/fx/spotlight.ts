/**
 * The spotlight of a batch (docs/ui-design.md, Playing is legible): a spell cast by another seat, or one that passes
 * through the stack inside the batch, stands large over the shared strip between its two legs; the viewer's own cast
 * stands there too, for a shorter time. Pure: what stands, where and when. `Spotlight.tsx` draws it and the travel
 * layer holds it beside the flights.
 */
import type { ObjectId, ObjectView, PlayerId, Target } from '@/protocol/generated'
import { MOVE_MS } from './moves'
import type { Move } from './moves'
import type { Box } from './places'

/** The spotlight's card is this many times as high as a card of the viewer's hand. */
export const SPOTLIGHT_SCALE = 1.6

/** The card is never lower than this, nor higher than this share of the window, in px and as a share. */
export const SPOTLIGHT_MIN_PX = 168
export const SPOTLIGHT_MAX_SHARE = 0.4

/** A window too short for that shrinks the card to this height and no further, in px. */
export const SPOTLIGHT_FLOOR_PX = 96

/** The card's shape: width over height. */
const ASPECT = 5 / 7

/** Room under the card for the caster, the name and the targets, and its width, in px. */
export const CAPTION_PX = 64
export const CAPTION_WIDTH_PX = 224

/** The gap between the spotlight and the strip, and the margin to the window's edge, in px. */
export const SPOTLIGHT_GAP_PX = 8
const MARGIN_PX = 8

/** The viewer's own cast stands this share of the time another seat's does. */
export const OWN_SHARE = 0.6

/** The time a spotlight stands before the slow-down, in ms: the viewer's own is shorter. */
export const spotlightMs = (own: boolean): number => MOVE_MS.spotlight * (own ? OWN_SHARE : 1)

/** The spotlight that stands now. Replaced, never changed. */
export interface Spotlight {
  /** Unique to the batch. */
  readonly key: string
  readonly object: ObjectId | null
  /** How to draw the card; null (or `faceDown`) draws a card back, which is all a viewer may see of a card unknown to them. */
  readonly look: ObjectView | null
  readonly faceDown: boolean
  readonly caster: PlayerId
  /** The viewer's own cast: it stands shorter, in a frame of lower contrast. */
  readonly own: boolean
  readonly targets: readonly Target[]
  /** Milliseconds after the batch's first leg, stretched by the slow-down. */
  readonly delay: number
  /** Milliseconds, stretched by the slow-down. */
  readonly duration: number
  /** It appears and goes without motion (reduced motion). */
  readonly still: boolean
  /** The card's box, the caption under it, in viewport coordinates. */
  readonly card: Box
  readonly box: Box
}

/** The time a spotlight takes to come in, and to go out, in ms: the same for every spotlight. */
const FADE_MS = 150

/**
 * The keyframes of the spotlight over its whole time: in with a little growth (`ease-out`), whole, then out (`ease-in`),
 * each fade `FADE_MS` long. A still one is on at once and off at once, as a card standing for the same time.
 */
export function spotlightKeyframes({ still, own = false }: { still: boolean; own?: boolean }): Keyframe[] {
  if (still) {
    return [
      { offset: 0, opacity: 0 },
      { offset: 0.001, opacity: 1 },
      { offset: 0.999, opacity: 1 },
      { offset: 1, opacity: 0 },
    ]
  }
  const fade = FADE_MS / spotlightMs(own)
  return [
    { offset: 0, opacity: 0, transform: 'scale(0.92)', easing: 'ease-out' },
    { offset: fade, opacity: 1, transform: 'scale(1)' },
    { offset: 1 - fade, opacity: 1, transform: 'scale(1)', easing: 'ease-in' },
    { offset: 1, opacity: 0, transform: 'scale(0.96)' },
  ]
}

/** What `planSpotlight` reads. */
export interface SpotlightInput {
  readonly moves: readonly Move[]
  /** The shared strip's stack on screen; null when none is, and nothing stands. */
  readonly strip: Box | null
  /** A card of the viewer's hand, for the size; the reference card when none is on screen. */
  readonly hand: { width: number; height: number } | null
  readonly window: { width: number; height: number }
  /** The viewer's seat: a cast by it is the viewer's own. */
  readonly viewer: PlayerId
  readonly batch: number
  readonly still: boolean
  readonly slow?: number
}

/**
 * The card's size: 1.6 times the hand card, at least `SPOTLIGHT_MIN_PX` high, at most `SPOTLIGHT_MAX_SHARE` of the
 * window and no higher than the room above the strip leaves (`room`), down to `SPOTLIGHT_FLOOR_PX`.
 */
export function spotlightSize(
  hand: { width: number; height: number } | null,
  window: { width: number; height: number },
  room: number = Infinity,
): { width: number; height: number } {
  const wanted = (hand?.height ?? 140) * SPOTLIGHT_SCALE
  const fitting = Math.max(SPOTLIGHT_MIN_PX, Math.min(wanted, window.height * SPOTLIGHT_MAX_SHARE))
  const height = Math.min(fitting, Math.max(SPOTLIGHT_FLOOR_PX, room))
  return { width: height * ASPECT, height }
}

const within = (at: number, length: number, limit: number): number =>
  Math.max(MARGIN_PX, Math.min(at, limit - length - MARGIN_PX))

/** The boxes of the card and of the whole spotlight: centred over the strip, its foot `SPOTLIGHT_GAP_PX` above it. */
export function spotlightBoxes(
  strip: Box,
  size: { width: number; height: number },
  window: { width: number; height: number },
): { card: Box; box: Box } {
  const width = Math.max(size.width, CAPTION_WIDTH_PX)
  const height = size.height + CAPTION_PX
  const x = within(strip.x + strip.width / 2 - width / 2, width, window.width)
  const y = within(strip.y - SPOTLIGHT_GAP_PX - height, height, window.height)
  return {
    box: { x, y, width, height },
    card: { x: x + (width - size.width) / 2, y, width: size.width, height: size.height },
  }
}

/** Whether the table shows a face for the card of a cast; the viewer's own is never drawn as a back. */
const identified = (look: ObjectView | null): look is ObjectView =>
  look !== null && look.face_down !== true && look.name !== ''

/**
 * The spotlight of the batch: the first cast that stands in it, `MOVE_MS.travel` after the batch (its first leg has
 * landed on the stack by then) or at once when nothing flies. When no cast of another seat (or through the stack)
 * stands, the viewer's own first cast does, for `OWN_SHARE` of the time and never as a card back. Null when no cast
 * does, or no strip is on screen.
 */
export function planSpotlight({
  moves,
  strip,
  hand,
  window,
  viewer,
  batch,
  still,
  slow = 1,
}: SpotlightInput): Spotlight | null {
  if (strip === null) return null
  const casterOf = (candidate: Move): PlayerId | null =>
    candidate.look?.controller ?? ('stack' in candidate.from ? null : candidate.from.seat)
  const full = moves.find((candidate) => candidate.kind === 'cast' && candidate.spotlight)
  const move =
    full ??
    moves.find(
      (candidate) =>
        candidate.kind === 'cast' && casterOf(candidate) === viewer && identified(candidate.look),
    )
  if (move === undefined) return null
  const caster = casterOf(move)
  if (caster === null) return null
  const own = full === undefined
  return {
    key: `${batch}:spotlight`,
    object: move.object,
    look: move.look,
    faceDown: !identified(move.look),
    caster,
    own,
    targets: move.targets,
    delay: (move.order * MOVE_MS.stagger + (still ? 0 : MOVE_MS.travel)) * slow,
    duration: spotlightMs(own) * slow,
    still,
    ...spotlightBoxes(
      strip,
      spotlightSize(hand, window, strip.y - SPOTLIGHT_GAP_PX - MARGIN_PX - CAPTION_PX),
      window,
    ),
  }
}
