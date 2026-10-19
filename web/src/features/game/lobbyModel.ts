/**
 * The lobby as words and rows: which seats there are, who is here and who is ready, the sentence that sums it up and
 * the sentence a screen reader hears when it changes. Pure, so the rules are tested without a page.
 */
import type { LobbyRoster } from '@/play/store'
import type { GameInfo } from '@/protocol/generated'

/** One seat of the lobby as the page shows it. */
export interface LobbyRow {
  seat: number
  /** The seat's name, or "Seat N" while nothing has said it. */
  name: string
  /** False while `name` is the stand-in for one the page does not know yet. */
  named: boolean
  /** This page's own seat. */
  you: boolean
  /** A client is connected to the seat (for this page's own seat: its socket is open). */
  connected: boolean
  /** The seat pressed Ready and did not take it back. It stays set when the client goes away. */
  ready: boolean
}

/** What the rows are made from. */
export interface LobbySources {
  /** What the open socket last said (the store clears it when the socket closes). */
  roster: LobbyRoster | null
  /** The last REST lookup, which names the seats before the socket opens and says who is here while it is down. */
  info: Pick<GameInfo, 'seats' | 'connected' | 'ready'> | null
  /** This page's socket is open. */
  open: boolean
  /** This page's seat, once welcomed. */
  seat: number | null
  /** How many seats the Welcome said the table has. */
  seatCount: number | null
}

/**
 * The seats to show. The roster is the server's latest word and wins while the socket is open; without it (before
 * the first one, or while reconnecting) the lookup stands in, and a seat nobody has named yet is "Seat N".
 */
export function lobbyRows({ roster, info, open, seat, seatCount }: LobbySources): LobbyRow[] {
  const count = roster?.seats.length ?? seatCount ?? info?.seats.length ?? 0
  return Array.from({ length: count }, (_, index): LobbyRow => {
    const pushed = roster?.seats[index]
    const listed = info?.seats.find((entry) => entry.seat === index)?.name
    const name = pushed?.name ?? listed
    const you = index === seat
    // A page knows best whether its own socket is open.
    const connected = you ? open : (pushed?.connected ?? info?.connected.includes(index) ?? false)
    return {
      seat: index,
      name: name ?? `Seat ${index + 1}`,
      named: name !== undefined,
      you,
      connected,
      ready: pushed?.ready ?? info?.ready.includes(index) ?? false,
    }
  })
}

/** "Ana", "Ana and Bo", "Ana, Bo and Cy"; four or more are only counted. */
function whom(rows: readonly LobbyRow[]): string {
  if (rows.length > 3) return `${rows.length} players`
  const names = rows.map((row) => (row.you ? 'you' : row.name))
  const last = names.pop()
  return names.length === 0 ? (last ?? '') : `${names.join(', ')} and ${last}`
}

/**
 * The line under the seats: "2 of 3 ready, waiting for Cy to join". Seats that are here and not ready are waited
 * on to press Ready, seats that are not here to join; a ready seat whose client is away is not waited on.
 */
export function summaryText(rows: readonly LobbyRow[]): string {
  if (rows.length === 0) return ''
  const ready = rows.filter((row) => row.ready).length
  if (ready === rows.length) return `All ${rows.length} seats are ready, starting the game`
  const toPress = rows.filter((row) => !row.ready && row.connected)
  const toJoin = rows.filter((row) => !row.ready && !row.connected)
  const waiting = [
    toPress.length > 0 ? `${whom(toPress)} to press Ready` : null,
    toJoin.length > 0 ? `${whom(toJoin)} to join` : null,
  ].filter((part) => part !== null)
  return `${ready} of ${rows.length} ready, waiting for ${waiting.join(' and ')}`
}

/** Most changes spelled out in one announcement; the rest are counted. */
const MAX_SPOKEN_CHANGES = 3

function changesOf(before: readonly LobbyRow[], after: readonly LobbyRow[]): string[] {
  const changes: string[] = []
  for (const row of after) {
    const was = before.find((earlier) => earlier.seat === row.seat)
    if (was === undefined) continue
    const flipped = was.ready === row.ready ? null : row.ready ? 'ready' : 'not ready'
    if (row.you) {
      // Whether this page's own socket is open is the connection banner's news, not the lobby's.
      if (flipped !== null) changes.push(`You are ${flipped}`)
      continue
    }
    const moved = was.connected === row.connected ? null : row.connected ? 'joined' : 'left'
    if (moved !== null && flipped !== null) changes.push(`${row.name} ${moved} and is ${flipped}`)
    else if (moved !== null) changes.push(`${row.name} ${moved}`)
    else if (flipped !== null) changes.push(`${row.name} is ${flipped}`)
  }
  return changes
}

/**
 * What to say aloud when the lobby went from `before` to `after`: the changes, then where things stand. Null when
 * nothing a person would notice changed.
 */
export function announcement(before: readonly LobbyRow[], after: readonly LobbyRow[]): string | null {
  const changes = changesOf(before, after)
  if (changes.length === 0) return null
  const spoken = changes.slice(0, MAX_SPOKEN_CHANGES)
  const more = changes.length - spoken.length
  if (more > 0) spoken.push(`${more} more ${more === 1 ? 'change' : 'changes'}`)
  return `${spoken.join(', ')}. ${summaryText(after)}`
}

/** How a seat's row reads on its own, for tests and for the row's accessible description. */
export type SeatState = 'ready' | 'waiting' | 'away'

/** Ready seats are ready whether or not their client is here; of the others, the ones here are waited on. */
export function seatState(row: Pick<LobbyRow, 'connected' | 'ready'>): SeatState {
  if (row.ready) return 'ready'
  return row.connected ? 'waiting' : 'away'
}
