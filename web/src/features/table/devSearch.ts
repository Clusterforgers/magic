import type { PlayerId } from '@/protocol/generated'
import { parseStopSpec } from './dev/stopSpec'
import type { TableMode } from './layout'

/** The moves `?fx=` can play on the fixture table, to look at the table effects (`dev/fxScenarios.ts`). */
export const DEV_FX = ['attack', 'block', 'damage', 'death', 'life', 'enter'] as const

/** One of `DEV_FX`. */
export type DevFx = (typeof DEV_FX)[number]

/** The longest slow-down `?slow=` accepts. */
export const MAX_DEV_SLOW = 20

/** The longest wait `?preview=` accepts, in milliseconds. */
export const MAX_DEV_PREVIEW_MS = 10_000

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
  /** Milliseconds the dev table takes to answer a preview of blocks or attacks (0 to 10 000), to look at "Checking outcome…". */
  preview?: number
  /** Stops the viewer already holds, as a spec such as `own:main1,opp:end:keep` (`dev/stopSpec.ts`); missing when it names none. */
  stops?: string
  /** The Full Control mode the viewer already holds, `next` or `held`; off when missing. */
  fc?: 'next' | 'held'
}

const whole = (value: unknown): number | undefined =>
  typeof value === 'number' && Number.isInteger(value) ? value : undefined

const isDevFx = (value: unknown): value is DevFx => DEV_FX.some((name) => name === value)

/** Reads `?seats=&mode=&focus=&prompt=&turn=&fx=&slow=&preview=&stops=&fc=`, ignoring whatever does not make sense. The router has already parsed numbers. */
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
    // The router validates an address again with what it made of it, so both keep the form the address has.
    stops:
      typeof search.stops === 'string' && parseStopSpec(search.stops).length > 0 ? search.stops : undefined,
    fc: search.fc === 'next' || search.fc === 'held' ? search.fc : undefined,
    preview:
      typeof search.preview === 'number' &&
      Number.isInteger(search.preview) &&
      search.preview >= 0 &&
      search.preview <= MAX_DEV_PREVIEW_MS
        ? search.preview
        : undefined,
  }
}
