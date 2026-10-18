/** Pure seat arrangement for the table: who is drawn where, from the seat count, the viewer and the focus. */
import type { PlayerId } from '@/protocol/generated'

/** Grid mode draws every opponent's battlefield; focus mode draws one large and the others as tiles. */
export type TableMode = 'grid' | 'focus'

/** Up to this many seats a table starts in grid mode. */
export const GRID_MAX_SEATS = 4
/** From this many seats cards are drawn in the compact variant. */
export const COMPACT_FROM_SEATS = 8
/** The most columns grid mode uses; more opponents wrap onto further rows. */
export const GRID_MAX_COLUMNS = 3

/** What the default focus is chosen from. */
export interface FocusContext {
  /** The seat whose turn it is. */
  active: PlayerId | null
  /** The seat that holds priority. */
  holder: PlayerId | null
  /** Seats that have lost; the fallback skips them. */
  eliminated: readonly PlayerId[]
}

/** The seats of a table and how they are drawn. */
export interface SeatPlan {
  mode: TableMode
  /** The seat this client plays; drawn last, at the bottom. */
  viewer: PlayerId
  /** Every other seat in turn order, starting after the viewer. */
  opponents: PlayerId[]
  /** The opponent drawn large, in focus mode only. */
  focused: PlayerId | null
  /** Opponents whose battlefield is drawn: all of them in grid mode, only the focused one in focus mode. */
  boards: PlayerId[]
  /** Opponents drawn as compact tiles: all of them in focus mode, none in grid mode. */
  tiles: PlayerId[]
  /** Columns for the opponents' boards. */
  columns: 1 | 2 | 3
  /** Whether cards are drawn in the compact variant. */
  compact: boolean
}

/** What `arrangeSeats` needs. */
export interface SeatInput {
  seatCount: number
  viewer: PlayerId
  /** The mode to draw; the default for the seat count when left out. */
  mode?: TableMode
  /** The opponent the player chose to focus; ignored when it is the viewer or not at the table. */
  focus?: PlayerId | null
  context?: FocusContext
}

const NO_CONTEXT: FocusContext = { active: null, holder: null, eliminated: [] }

/** Grid mode for up to `GRID_MAX_SEATS` seats, focus mode for more. */
export function defaultMode(seatCount: number): TableMode {
  return seatCount <= GRID_MAX_SEATS ? 'grid' : 'focus'
}

/** Every seat but `viewer`, in turn order from the seat after it. */
export function opponentsOf(seatCount: number, viewer: PlayerId): PlayerId[] {
  if (seatCount < 2) return []
  return Array.from({ length: seatCount - 1 }, (_, index) => (viewer + 1 + index) % seatCount)
}

/** The opponent to show large when nobody chose one: the active player, else the priority holder, else the next seat. */
export function defaultFocus(
  seatCount: number,
  viewer: PlayerId,
  context: FocusContext = NO_CONTEXT,
): PlayerId | null {
  const opponents = opponentsOf(seatCount, viewer)
  const { active, holder, eliminated } = context
  if (active !== null && opponents.includes(active)) return active
  if (holder !== null && opponents.includes(holder)) return holder
  return opponents.find((seat) => !eliminated.includes(seat)) ?? opponents[0] ?? null
}

const wholeSeats = (seatCount: number): number =>
  Number.isFinite(seatCount) ? Math.max(0, Math.floor(seatCount)) : 0

/** Arranges the table; total for any input, so a bad seat number never throws. */
export function arrangeSeats(input: SeatInput): SeatPlan {
  const seatCount = wholeSeats(input.seatCount)
  const viewer = seatCount === 0 ? 0 : ((Math.trunc(input.viewer) % seatCount) + seatCount) % seatCount
  const mode = input.mode ?? defaultMode(seatCount)
  const opponents = opponentsOf(seatCount, viewer)
  const compact = seatCount >= COMPACT_FROM_SEATS
  if (mode === 'grid') {
    const columns = Math.min(GRID_MAX_COLUMNS, Math.max(1, opponents.length))
    return {
      mode,
      viewer,
      opponents,
      focused: null,
      boards: opponents,
      tiles: [],
      columns: toColumns(columns),
      compact,
    }
  }
  const chosen = input.focus ?? null
  const focused =
    chosen !== null && opponents.includes(chosen) ? chosen : defaultFocus(seatCount, viewer, input.context)
  return {
    mode,
    viewer,
    opponents,
    focused,
    boards: focused === null ? [] : [focused],
    tiles: opponents,
    columns: 1,
    compact,
  }
}

function toColumns(columns: number): 1 | 2 | 3 {
  return columns >= 3 ? 3 : columns === 2 ? 2 : 1
}
