import type { PlayerId } from '@/protocol/generated'
import type { TableMode } from './layout'

/** The moves `?fx=` can play on the fixture table, to look at the table effects (`dev/fxScenarios.ts`). */
export const DEV_FX = ['attack', 'block', 'damage', 'death', 'life', 'enter'] as const

/** One of `DEV_FX`. */
export type DevFx = (typeof DEV_FX)[number]

/** The longest slow-down `?slow=` accepts. */
export const MAX_DEV_SLOW = 20

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
  /** A move to play on the table once it has loaded, so its effect can be looked at; also on buttons in the page. */
  fx?: DevFx
  /** Makes every table effect this many times slower (above 1, up to 20), to look at one before it is gone. */
  slow?: number
}

const whole = (value: unknown): number | undefined =>
  typeof value === 'number' && Number.isInteger(value) ? value : undefined

const isDevFx = (value: unknown): value is DevFx => DEV_FX.some((name) => name === value)

/** Reads `?seats=&mode=&focus=&prompt=&turn=&fx=&slow=`, ignoring whatever does not make sense. The router has already parsed numbers. */
export function parseDevSearch(search: Record<string, unknown>): DevSearch {
  const seats = whole(search.seats)
  const focus = whole(search.focus)
  const mode = search.mode === 'grid' || search.mode === 'focus' ? search.mode : undefined
  const slow =
    typeof search.slow === 'number' && search.slow > 1 && search.slow <= MAX_DEV_SLOW
      ? search.slow
      : undefined
  return {
    seats: seats === undefined || seats < 2 || seats > 12 ? 4 : seats,
    mode,
    focus: focus === undefined || focus < 0 ? undefined : focus,
    prompt: typeof search.prompt === 'string' ? search.prompt : undefined,
    turn: search.turn === 'you' ? 'you' : undefined,
    fx: isDevFx(search.fx) ? search.fx : undefined,
    slow,
  }
}
