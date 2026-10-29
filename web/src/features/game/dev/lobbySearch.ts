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
  /** `invite` makes the game invite only; a listed game when missing. */
  visibility?: 'invite'
  /** The seats that have chosen no deck yet (everybody else plays one). */
  nodeck?: number[]
  /** `1` makes this page a signed-in player, whose chooser lists stored decks beside the built-in ones. */
  /** `1` makes this page a signed-in player, whose chooser lists stored decks beside the built-in ones. */
  account?: true
  /** `commander` makes the game a Commander game: the chooser lists Commander decks and precons, not built-in decks. */
  format?: 'commander'
  /** `1` gives the game house rules that differ from its format's own. */
  rules?: true
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

/** Reads `?seats=&ready=&away=&me=&names=&visibility=&nodeck=&account=&format=&rules=`; the router has already parsed numbers and lists. */
export function parseDevLobbySearch(search: Record<string, unknown>): DevLobbySearch {
  const asked = whole(search.seats)
  const seats = asked === undefined || asked < FEWEST_SEATS || asked > MOST_SEATS ? 4 : asked
  const me = whole(search.me)
  const nodeck = seatList(search.nodeck, seats)
  return {
    seats,
    ready: seatList(search.ready, seats),
    away: seatList(search.away, seats),
    me: me === undefined || me >= seats ? 0 : me,
    names: search.names === 'long' ? 'long' : undefined,
    visibility: search.visibility === 'invite' ? 'invite' : undefined,
    nodeck: nodeck.length > 0 ? nodeck : undefined,
    // The router feeds its own answer back in, so what this returns must read as itself (`true`).
    account: search.account === true || search.account === 1 || search.account === '1' ? true : undefined,
    format: search.format === 'commander' ? 'commander' : undefined,
    rules: search.rules === true || search.rules === 1 || search.rules === '1' ? true : undefined,
  }
}
