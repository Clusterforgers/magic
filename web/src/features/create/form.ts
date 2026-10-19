/** The create-game form as plain data: defaults, edits, the server's own checks, and the request body. */
import { MAX_NAME_CHARS, MAX_SEATS, MIN_SEATS } from '@/lib/limits'
import type { CreateGameRequest, DeckInfo } from '@/net/api'
import type { Visibility } from '@/protocol/generated'

/** One seat in the form; `id` is a stable key that is never reused, so edits stay with their seat when another goes. */
export interface SeatDraft {
  id: number
  name: string
  deck: string
}

/** The whole form. */
export interface Draft {
  seats: SeatDraft[]
  nextId: number
  /** Who may find the game when its other seats are open; a game with a link per seat is always invite only. */
  visibility: Visibility
  /** Every seat is named here and gets its own link, as before open seats: the game is invite only and holds them all. */
  links: boolean
}

/** What is wrong with one seat, in words for the person filling in the form; null for fine. */
export interface SeatProblems {
  name: string | null
  deck: string | null
}

/** The DOM id of seat `id`'s name field; the form focuses it after the seat is added. */
export const nameFieldId = (id: number): string => `seat-${id}-name`

// The same set the server refuses (`is_unsafe_in_names` in crates/mtg-server/src/api.rs): control characters,
// zero-width and direction-changing marks, which could make a name pose as other text in the game log.
const UNSAFE_IN_NAMES = /[\p{Cc}​-‏‪-‮⁠-⁤⁦-⁩﻿]/u

const defaultName = (taken: ReadonlySet<string>): string => {
  for (let n = 1; ; n += 1) {
    if (!taken.has(`Player ${n}`)) return `Player ${n}`
  }
}

// The deck fewest seats use so far, the first one on a tie: two seats start on different decks while there are some.
function leastUsedDeck(seats: readonly SeatDraft[], decks: readonly DeckInfo[]): string {
  const uses = (id: string): number => seats.filter((seat) => seat.deck === id).length
  let best = decks[0]
  for (const deck of decks) {
    if (best === undefined || uses(deck.id) < uses(best.id)) best = deck
  }
  return best?.id ?? ''
}

function withSeat(draft: Draft, decks: readonly DeckInfo[]): Draft {
  const taken = new Set(draft.seats.map((seat) => seat.name))
  const seat: SeatDraft = {
    id: draft.nextId,
    name: defaultName(taken),
    deck: leastUsedDeck(draft.seats, decks),
  }
  return { ...draft, seats: [...draft.seats, seat], nextId: draft.nextId + 1 }
}

/**
 * The form as first shown: `MIN_SEATS` seats named Player 1, Player 2, ..., on different decks while the server has
 * them, listed in the game browser, the other seats open. `host`, the signed-in creator's username, names the first
 * seat, which is the creator's own.
 */
export function defaultDraft(decks: readonly DeckInfo[], host?: string): Draft {
  let draft: Draft = { seats: [], nextId: 0, visibility: 'public', links: false }
  for (let i = 0; i < MIN_SEATS; i += 1) draft = withSeat(draft, decks)
  const first = draft.seats[0]
  return host === undefined || first === undefined ? draft : editSeat(draft, first.id, { name: host })
}

/** The form with one more seat, or as it was when it is full. */
export function addSeat(draft: Draft, decks: readonly DeckInfo[]): Draft {
  return draft.seats.length >= MAX_SEATS ? draft : withSeat(draft, decks)
}

/** The form without seat `id`, or as it was when only `MIN_SEATS` are left. */
export function removeSeat(draft: Draft, id: number): Draft {
  if (draft.seats.length <= MIN_SEATS) return draft
  return { ...draft, seats: draft.seats.filter((seat) => seat.id !== id) }
}

/** The form with `count` seats (kept between `MIN_SEATS` and `MAX_SEATS`): new ones at the end, or the last ones gone. */
export function resizeSeats(draft: Draft, decks: readonly DeckInfo[], count: number): Draft {
  const target = Math.min(MAX_SEATS, Math.max(MIN_SEATS, Math.trunc(count)))
  if (target === draft.seats.length) return draft
  let next = draft
  while (next.seats.length < target) next = withSeat(next, decks)
  return next.seats.length > target ? { ...next, seats: next.seats.slice(0, target) } : next
}

/** The form with seat `id` changed by `change`. */
export function editSeat(draft: Draft, id: number, change: Partial<Pick<SeatDraft, 'name' | 'deck'>>): Draft {
  return { ...draft, seats: draft.seats.map((seat) => (seat.id === id ? { ...seat, ...change } : seat)) }
}

/** The form with who may find the game changed. */
export function chooseVisibility(draft: Draft, visibility: Visibility): Draft {
  return { ...draft, visibility }
}

/** The form with a link for every seat turned on or off; what was typed stays. */
export function chooseLinks(draft: Draft, links: boolean): Draft {
  return { ...draft, links }
}

/** Why `name` would be refused, or null. Names are trimmed first, as the server does, and counted in characters. */
export function nameProblem(name: string): string | null {
  const trimmed = name.trim()
  const length = [...trimmed].length
  if (length === 0) return 'Enter a name for this seat.'
  if (length > MAX_NAME_CHARS) return `Use at most ${MAX_NAME_CHARS} characters (this one has ${length}).`
  if (UNSAFE_IN_NAMES.test(trimmed)) return 'Remove control or invisible characters from the name.'
  return null
}

/**
 * What is wrong with each seat, keyed by seat id; seats without trouble have both fields null. Without a link per
 * seat only the first seat is the creator's to fill in: the others are open and named by whoever takes them.
 */
export function problemsOf(draft: Draft, decks: readonly DeckInfo[]): Map<number, SeatProblems> {
  const known = new Set(decks.map((deck) => deck.id))
  return new Map(
    draft.seats.map((seat, index): [number, SeatProblems] => [
      seat.id,
      draft.links || index === 0
        ? { name: nameProblem(seat.name), deck: known.has(seat.deck) ? null : 'Choose a deck.' }
        : { name: null, deck: null },
    ]),
  )
}

/** How many seats have something wrong. */
export function problemCount(problems: ReadonlyMap<number, SeatProblems>): number {
  return [...problems.values()].filter((p) => p.name !== null || p.deck !== null).length
}

/**
 * The body for `POST /api/games`: the format is always "normal" and names go out trimmed. A link per seat holds every
 * seat and is invite only; otherwise only the creator's seat is held and the others, which the server never reads a
 * name for, go out as `Player N` on the deck the form gave them.
 */
export function toRequest(draft: Draft): CreateGameRequest {
  if (draft.links) {
    return {
      format: 'normal',
      seats: draft.seats.map((seat) => ({ name: seat.name.trim(), deck: seat.deck })),
      visibility: 'invite',
    }
  }
  return {
    format: 'normal',
    seats: draft.seats.map((seat, index) => ({
      name: index === 0 ? seat.name.trim() : `Player ${index + 1}`,
      deck: seat.deck,
    })),
    visibility: draft.visibility,
    held: 1,
  }
}
