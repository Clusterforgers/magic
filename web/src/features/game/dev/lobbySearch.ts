/** The address parameters of `/dev/lobby`, read leniently: whatever makes no sense is ignored. */
export interface DevLobbySearch {
  /** How many seats the table has, 2 to 12; 4 when missing or wrong. */
  seats: number
  /** The seats that have pressed Ready. */
  ready: number[]
  /** The seats whose client is away (everybody else is connected). */
  away: number[]
  /** The seat this page plays; 0 when missing or wrong. */
  me: number
  /** `long` gives the seats names of the longest length the server accepts. */
  names?: 'long'
}

const FEWEST_SEATS = 2
const MOST_SEATS = 12

const whole = (value: unknown): number | undefined =>
  typeof value === 'number' && Number.isInteger(value) && value >= 0 ? value : undefined

/** Seat numbers from `3`, `"0,2"` or `[0, 2]`, in order, once each and below `seats`. */
function seatList(value: unknown, seats: number): number[] {
  const parts: unknown[] = Array.isArray(value)
    ? value
    : typeof value === 'string'
      ? value.split(',').map((part) => (part.trim() === '' ? Number.NaN : Number(part)))
      : [value]
  const found = parts.flatMap((part) => {
    const seat = whole(part)
    return seat !== undefined && seat < seats ? [seat] : []
  })
  return [...new Set(found)].sort((a, b) => a - b)
}

/** Reads `?seats=&ready=&away=&me=&names=`; the router has already parsed numbers and lists. */
export function parseDevLobbySearch(search: Record<string, unknown>): DevLobbySearch {
  const asked = whole(search.seats)
  const seats = asked === undefined || asked < FEWEST_SEATS || asked > MOST_SEATS ? 4 : asked
  const me = whole(search.me)
  return {
    seats,
    ready: seatList(search.ready, seats),
    away: seatList(search.away, seats),
    me: me === undefined || me >= seats ? 0 : me,
    names: search.names === 'long' ? 'long' : undefined,
  }
}
