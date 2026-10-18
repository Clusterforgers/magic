/** The create-game form as plain data: defaults, edits, the server's own checks, and the request body. */
import { MAX_NAME_CHARS, MAX_SEATS, MIN_SEATS } from '@/lib/limits'
import type { CreateGameRequest, DeckInfo } from '@/net/api'

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
  return { seats: [...draft.seats, seat], nextId: draft.nextId + 1 }
}

/** The form as first shown: `MIN_SEATS` seats named Player 1, Player 2, ..., on different decks while the server has them. */
export function defaultDraft(decks: readonly DeckInfo[]): Draft {
  let draft: Draft = { seats: [], nextId: 0 }
  for (let i = 0; i < MIN_SEATS; i += 1) draft = withSeat(draft, decks)
  return draft
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

/** The form with seat `id` changed by `change`. */
export function editSeat(draft: Draft, id: number, change: Partial<Pick<SeatDraft, 'name' | 'deck'>>): Draft {
  return { ...draft, seats: draft.seats.map((seat) => (seat.id === id ? { ...seat, ...change } : seat)) }
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

/** What is wrong with each seat, keyed by seat id; seats without trouble have both fields null. */
export function problemsOf(draft: Draft, decks: readonly DeckInfo[]): Map<number, SeatProblems> {
  const known = new Set(decks.map((deck) => deck.id))
  return new Map(
    draft.seats.map((seat): [number, SeatProblems] => [
      seat.id,
      { name: nameProblem(seat.name), deck: known.has(seat.deck) ? null : 'Choose a deck.' },
    ]),
  )
}

/** How many seats have something wrong. */
export function problemCount(problems: ReadonlyMap<number, SeatProblems>): number {
  return [...problems.values()].filter((p) => p.name !== null || p.deck !== null).length
}

/** The body for `POST /api/games`: the format is always "normal" and names go out trimmed. */
export function toRequest(draft: Draft): CreateGameRequest {
  return {
    format: 'normal',
    seats: draft.seats.map((seat) => ({ name: seat.name.trim(), deck: seat.deck })),
  }
}
