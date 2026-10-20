import type { LobbySeat, SeatDeck, ServerMessage } from '@/protocol/generated'

/** The deck every named seat of a test roster plays unless the test says otherwise. */
export const SEAT_DECK: SeatDeck = { name: 'Mono Green Stompers', format: 'normal', manual_cards: 0 }

/**
 * One seat of a roster in a test: its name, whether a client is connected to it, whether it is ready and, last, its
 * deck: left out for the usual one, `null` for none. An open seat (an empty name) never has one.
 */
export type SeatFlags = readonly [name: string, connected: boolean, ready: boolean, deck?: SeatDeck | null]

/** The `Lobby` message a server sends, one entry per seat in seat order; `needed` counts the seats not ready. */
export function lobbyMessage(seats: readonly SeatFlags[]): Extract<ServerMessage, { type: 'Lobby' }> {
  const listed: LobbySeat[] = seats.map(([name, connected, ready, deck], seat) => {
    const chosen = name === '' ? null : deck === undefined ? SEAT_DECK : deck
    return { seat, name, connected, ready, ...(chosen === null ? {} : { deck: chosen }) }
  })
  return { type: 'Lobby', seats: listed, needed: listed.filter((entry) => !entry.ready).length }
}

/** `count` seats named after the alphabet (Ann, Bea, Cy...), none connected and none ready. */
export function emptyTable(count: number): SeatFlags[] {
  const names = ['Ann', 'Bea', 'Cy', 'Di', 'Ed', 'Flo', 'Gus', 'Hal', 'Ivy', 'Jo', 'Kit', 'Lu']
  return names.slice(0, count).map((name): SeatFlags => [name, false, false])
}

/** `table` with the seats at `changes` replaced: `{ 1: [true, true] }` has seat 1 connected and ready. */
export function withSeats(
  table: readonly SeatFlags[],
  changes: Readonly<Record<number, readonly [connected: boolean, ready: boolean]>>,
): SeatFlags[] {
  return table.map(([name, connected, ready, deck], seat): SeatFlags => {
    const change = changes[seat]
    const flags: [string, boolean, boolean] =
      change === undefined ? [name, connected, ready] : [name, change[0], change[1]]
    return deck === undefined ? flags : [...flags, deck]
  })
}

/** `table` with the deck of the seats at `decks` replaced: `{ 1: null }` leaves seat 1 without one. */
export function withDecks(
  table: readonly SeatFlags[],
  decks: Readonly<Record<number, SeatDeck | null>>,
): SeatFlags[] {
  return table.map((entry, seat): SeatFlags => {
    const [name, connected, ready] = entry
    const deck = decks[seat]
    return deck === undefined ? entry : [name, connected, ready, deck]
  })
}
