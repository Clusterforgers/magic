/**
 * The moments of a batch that every seat is meant to see: cards a player revealed and dice a player rolled. Pure: what
 * stands, for how long and how large. `store.ts` holds them, `TableMoments.tsx` draws them.
 */
import type { LogEntry } from '@/play/store'
import type { Event, ObjectView, PlayerId } from '@/protocol/generated'
import { eventContexts } from '../eventContext'

/** Cards a player revealed to every seat. */
export interface RevealMoment {
  kind: 'reveal'
  key: string
  player: PlayerId
  /** The cards as they looked when shown, in the order revealed. */
  cards: readonly ObjectView[]
  /** The spell or ability that made the reveal, by name; null when none was resolving. */
  why: string | null
}

/** A die a player rolled, here or by hand. */
export interface RollMoment {
  kind: 'roll'
  key: string
  player: PlayerId
  sides: number
  result: number
  why: string | null
}

/** One thing that stands over the table for a while. */
export type Moment = RevealMoment | RollMoment

/** How long a reveal stands before it goes by itself, in ms; a click, Escape or Dismiss ends it sooner. */
export const REVEAL_MS = 6000

/** How long a die stands: the roll takes `ROLL_TICKS` ticks, the result stays the rest of the time, in ms. */
export const ROLL_MS = 3600

/** The most moments on screen together; the oldest goes when a newer one comes. */
export const MOST_MOMENTS = 3

/** How long a moment stands, in ms. */
export const momentMs = (moment: Moment): number => (moment.kind === 'reveal' ? REVEAL_MS : ROLL_MS)

/** What `momentsOf` needs besides the events. */
export interface MomentInput {
  /** The log entries of one batch, with the cards its reveals showed. */
  entries: readonly LogEntry[]
  /** The name of an object the table holds or held, or undefined. */
  nameOf(id: number): string | undefined
}

/** An event with a payload: the plain ones (`"DamageCleared"`) carry no card and no roll. */
type Payload = Exclude<Event, string>

const publicOf = (entry: LogEntry): Payload | null =>
  'Public' in entry.event && typeof entry.event.Public !== 'string' ? entry.event.Public : null

/** The sides of a die a public event rolled, with who rolled it and the face; null for any other event. */
function rollOf(event: Payload): { player: PlayerId; sides: number; result: number } | null {
  if ('DieRolled' in event) return event.DieRolled
  if ('ManualRolled' in event) {
    const { player, kind, result } = event.ManualRolled
    return 'Die' in kind ? { player, sides: kind.Die.sides, result } : null
  }
  return null
}

/**
 * The moments of a batch, in the order of its events. A reveal is public (`Event::Revealed`), so every seat has one;
 * a look at a library (`LibraryLooked`) is the looker's alone and a hidden event names no card, so neither makes a
 * moment. A reveal whose cards the viewer's view never held makes none. The spell or ability that was resolving at the
 * event is the reason a line gives.
 */
export function momentsOf({ entries, nameOf }: MomentInput): Moment[] {
  const contexts = eventContexts(entries.map((entry) => entry.event))
  const moments: Moment[] = []
  entries.forEach((entry, index) => {
    const event = publicOf(entry)
    if (event === null) return
    const resolving = contexts[index]?.resolving
    const why = resolving === undefined ? null : (nameOf(resolving) ?? null)
    const key = `${entry.seq}`
    if ('Revealed' in event) {
      const cards = entry.seen ?? []
      if (cards.length > 0) moments.push({ kind: 'reveal', key, player: event.Revealed.player, cards, why })
      return
    }
    const roll = rollOf(event)
    if (roll !== null && roll.sides >= 1 && Number.isInteger(roll.sides) && Number.isInteger(roll.result))
      moments.push({ kind: 'roll', key, ...roll, why })
  })
  return moments
}

/** The moments left after `more` came: the newest `MOST_MOMENTS`. */
export function withMoments(current: readonly Moment[], more: readonly Moment[]): Moment[] {
  return [...current, ...more].slice(-MOST_MOMENTS)
}

/** The least and the most a revealed card's picture is wide, in px. */
export const REVEAL_MIN_PX = 96
export const REVEAL_MAX_PX = 300

/** A card's height over its width. */
const ASPECT = 7 / 5

/** The share of the window's height a card may take, the gap between cards, and the room the panel and the page's margin keep at the sides, in px. */
const HEIGHT_SHARE = 0.5
const CARD_GAP_PX = 12
const SIDE_ROOM_PX = 56

/**
 * How wide the picture of each of `count` revealed cards is: as large as the window allows (at most half its height and
 * all the width the cards share), never over `REVEAL_MAX_PX` and never under `REVEAL_MIN_PX`, below which the row scrolls.
 */
export function revealWidth(count: number, window: { width: number; height: number }): number {
  const n = Math.max(1, count)
  const byRow = (window.width - SIDE_ROOM_PX - CARD_GAP_PX * (n - 1)) / n
  const byHeight = (window.height * HEIGHT_SHARE) / ASPECT
  return Math.floor(Math.max(REVEAL_MIN_PX, Math.min(REVEAL_MAX_PX, byRow, byHeight)))
}

/** The ticks a roll runs through before it lands, and how long each takes, in ms. */
export const ROLL_TICKS = 12
export const ROLL_TICK_MS = 75

/** A small deterministic generator, so a roll shows the same faces to every seat and in every test. */
function lcg(seed: number): () => number {
  let state = seed >>> 0 || 1
  return () => {
    state = (Math.imul(state, 1664525) + 1013904223) >>> 0
    return state
  }
}

const seedOf = (key: string): number =>
  [...key].reduce((hash, char) => (Math.imul(hash, 31) + char.charCodeAt(0)) >>> 0, 7)

/**
 * The faces a die shows while it tumbles, ending on `result`: `ticks` numbers from 1 to `sides`, no two in a row the
 * same while the die has another face, the last one the result.
 */
export function rollFaces(sides: number, result: number, key: string, ticks = ROLL_TICKS): number[] {
  const next = lcg(seedOf(key))
  const faces: number[] = []
  for (let tick = 0; tick < ticks - 1; tick += 1) {
    let face = (next() % sides) + 1
    if (sides > 1 && face === faces.at(-1)) face = (face % sides) + 1
    faces.push(face)
  }
  if (sides > 1 && faces.at(-1) === result) faces[faces.length - 1] = (result % sides) + 1
  return [...faces, result]
}

/** The outline of a die, by its number of sides: a polygon for the dice of a game, a circle for any other. */
export type DieShape = 'triangle' | 'square' | 'diamond' | 'kite' | 'pentagon' | 'hexagon' | 'circle'

/** The shape of a die of `sides` faces (d4 a triangle, d6 a square, d8 a diamond, d10 a kite, d12 a pentagon, d20 a hexagon). */
export function dieShape(sides: number): DieShape {
  const shapes: Record<number, DieShape> = {
    4: 'triangle',
    6: 'square',
    8: 'diamond',
    10: 'kite',
    12: 'pentagon',
    20: 'hexagon',
  }
  return shapes[sides] ?? 'circle'
}

/** The points of a regular polygon of `n` corners in a 100 by 100 box, the first at the top, rounded to a tenth. */
function polygon(n: number, turn = 0): string {
  return Array.from({ length: n }, (_, corner) => {
    const angle = ((corner / n) * 360 - 90 + turn) * (Math.PI / 180)
    return `${(50 + 46 * Math.cos(angle)).toFixed(1)},${(50 + 46 * Math.sin(angle)).toFixed(1)}`
  }).join(' ')
}

/** The `points` of a die's outline in a 100 by 100 box; null for a circle. */
export function diePoints(shape: DieShape): string | null {
  switch (shape) {
    case 'triangle':
      return '50,6 95,90 5,90'
    case 'square':
      return '8,8 92,8 92,92 8,92'
    case 'diamond':
      return polygon(4)
    case 'kite':
      return '50,4 92,40 50,96 8,40'
    case 'pentagon':
      return polygon(5)
    case 'hexagon':
      return polygon(6, 30)
    case 'circle':
      return null
  }
}
