import type { LobbySeat, ServerMessage } from '@/protocol/generated'

/** One seat of a roster in a test: its name, whether a client is connected to it and whether it is ready. */
export type SeatFlags = readonly [name: string, connected: boolean, ready: boolean]

/** The `Lobby` message a server sends, one entry per seat in seat order; `needed` counts the seats not ready. */
export function lobbyMessage(seats: readonly SeatFlags[]): Extract<ServerMessage, { type: 'Lobby' }> {
  const listed: LobbySeat[] = seats.map(([name, connected, ready], seat) => ({
    seat,
    name,
    connected,
    ready,
  }))
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
  return table.map(([name, connected, ready], seat): SeatFlags => {
    const change = changes[seat]
    return change === undefined ? [name, connected, ready] : [name, change[0], change[1]]
  })
}
