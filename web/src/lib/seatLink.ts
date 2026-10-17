/**
 * Seat links: `/game/<id>#seat=<n>&token=<token>`. The token rides in the fragment, which a browser never sends to a
 * server (not in the request, not in a Referer), so only this page's script ever sees it.
 */
import { MAX_SEATS } from './limits'

/** The seat a link opens and the secret that proves the right to it. */
export interface SeatLink {
  seat: number
  token: string
}

// Printable ASCII without spaces; the server's tokens are 64 hex digits, and a wrong one is simply refused.
const TOKEN_SHAPE = /^[\x21-\x7e]{1,512}$/
const SEAT_SHAPE = /^(0|[1-9]\d?)$/

/** The path of a seat link, with the token only in the fragment. */
export function seatLinkPath(game: string, seat: number, token: string): string {
  return `/game/${encodeURIComponent(game)}#seat=${seat}&token=${encodeURIComponent(token)}`
}

/** The full address of a seat link on `origin` (such as `https://play.example`). */
export function seatLinkUrl(origin: string, game: string, seat: number, token: string): string {
  return `${origin}${seatLinkPath(game, seat, token)}`
}

/** The fragment of an address, without the `#`; empty when it has none. */
export function fragmentOf(href: string): string {
  const at = href.indexOf('#')
  return at === -1 ? '' : href.slice(at + 1)
}

/** Reads the seat and token from a fragment (a leading `#` is fine); null when either is missing or malformed. */
export function parseSeatFragment(fragment: string): SeatLink | null {
  const params = new URLSearchParams(fragment.startsWith('#') ? fragment.slice(1) : fragment)
  const seat = params.get('seat')
  const token = params.get('token')
  if (seat === null || token === null || !SEAT_SHAPE.test(seat) || !TOKEN_SHAPE.test(token)) return null
  const number = Number(seat)
  return number < MAX_SEATS ? { seat: number, token } : null
}
