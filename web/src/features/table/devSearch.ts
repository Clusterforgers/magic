import type { PlayerId } from '@/protocol/generated'
import type { TableMode } from './layout'

/** The address parameters of `/dev/table`. */
export interface DevSearch {
  /** How many seats the fixture table has, 2 to 12; 4 when missing or wrong. */
  seats: number
  /** The layout to open in; the default for the seat count when missing. */
  mode?: TableMode
  /** The opponent to open focused; the one following the turn when missing. */
  focus?: PlayerId
  /** The decision to face, by name in `dev/prompts.ts` (such as `priority`); none when missing or unknown. */
  prompt?: string
  /** `you` makes it the viewer's own turn, which lights the "your turn" cue; the opponent after the viewer otherwise. */
  turn?: 'you'
}

const whole = (value: unknown): number | undefined =>
  typeof value === 'number' && Number.isInteger(value) ? value : undefined

/** Reads `?seats=&mode=&focus=&prompt=&turn=`, ignoring whatever does not make sense. The router has already parsed numbers. */
export function parseDevSearch(search: Record<string, unknown>): DevSearch {
  const seats = whole(search.seats)
  const focus = whole(search.focus)
  const mode = search.mode === 'grid' || search.mode === 'focus' ? search.mode : undefined
  return {
    seats: seats === undefined || seats < 2 || seats > 12 ? 4 : seats,
    mode,
    focus: focus === undefined || focus < 0 ? undefined : focus,
    prompt: typeof search.prompt === 'string' ? search.prompt : undefined,
    turn: search.turn === 'you' ? 'you' : undefined,
  }
}
