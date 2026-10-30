import type { PlayerId, Step } from '@/protocol/generated'
import { parseStopSpec } from './dev/stopSpec'
import type { CommandId } from './manual/panel/commands'
import { panelOf } from './manual/panel/devPanel'
import type { TableMode } from './layout'
import { STEP_LABEL } from './turnText'

/** The steps `?step=` names, by their protocol names in lower case. */
const STEP_BY_NAME = new Map((Object.keys(STEP_LABEL) as Step[]).map((step) => [step.toLowerCase(), step]))

/** The moves `?fx=` can play on the fixture table, to look at the table effects (`dev/fxScenarios.ts`). */
export const DEV_FX = [
  'attack',
  'block',
  'damage',
  'death',
  'life',
  'enter',
  'cast',
  'rival',
  'activate',
  'resolve',
  'counter',
  'land',
  'draw',
  'equip',
] as const

/** The moves of `DEV_FX` that are moments of play (a cast and what follows), drawn by the travel layer. */
export const DEV_PLAYS = ['cast', 'rival', 'activate', 'resolve', 'counter', 'land', 'draw'] as const

/** One of `DEV_FX`. */
export type DevFx = (typeof DEV_FX)[number]

/** The longest slow-down `?slow=` accepts. */
export const MAX_DEV_SLOW = 20

/** The longest wait `?preview=` accepts, in milliseconds. */
export const MAX_DEV_PREVIEW_MS = 10_000

/** The address parameters of `/dev/table`. */
export interface DevSearch {
  /** How many seats the fixture table has, 2 or more; 4 when missing or wrong. */
  seats: number
  /** `duel`: two seats with 12 and 10 permanents of an early Commander game; `heavy`: two seats with 40 each. */
  board?: 'duel' | 'heavy'
  /** The layout to open in; the default for the seat count when missing. */
  mode?: TableMode
  /** The opponent to open focused; the one following the turn when missing. */
  focus?: PlayerId
  /** The decision to face, by name in `dev/prompts.ts` (such as `priority`); none when missing or unknown. */
  prompt?: string
  /** `you` makes it the viewer's own turn, which lights the "your turn" cue; the opponent after the viewer otherwise. */
  turn?: 'you'
  /** `1` opens the Effects bar, which is folded behind one button otherwise (`dev/DevFxControls.tsx`). */
  bar?: true
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
  /** Cards in the viewer's hand (0 to 24), to look at a large hand; 7 when missing. */
  hand?: number
  /** The step the turn is in, such as `main2` or `declareattackers`, to look at the theme of its phase; the fixture's own when missing. */
  step?: Step
  /** `1` gives the table house rules that differ from its format's own, to look at the list in the top bar. */
  rules?: true
  /** `1` gives the viewer and the rivals a permanent for every state a card shows (`dev/states.ts`), to look at them all at once. */
  states?: true
  /** `1` adds the attachments scene (`dev/equip.ts`): five attachments on one creature, an opponent's Aura on yours, yours on theirs, a Curse and an unattached Equipment. */
  equip?: true
  /** `real` identifies the cards by the card search of a running server, so the table draws real pictures. */
  cards?: 'real'
  /** Opens the manual menu of a card once the table has loaded: `1` a permanent of the viewer, `hand` a card in the viewer's hand, `opp` a permanent of an opponent. */
  menu?: DevMenu
  /** Opens a row of the Commands panel once the table has loaded: `1` the life row, or a command by its id (`draw`, `token`). */
  panel?: CommandId
}

/** The cards `?menu=` opens the manual menu on. */
export type DevMenu = 'permanent' | 'hand' | 'opponent'

function menuOf(value: unknown): DevMenu | undefined {
  // The router parses an address again with what it made of it, so the names it made stay valid.
  if (value === 1 || value === true || value === '1' || value === 'permanent') return 'permanent'
  if (value === 'hand') return 'hand'
  return value === 'opp' || value === 'opponent' ? 'opponent' : undefined
}

const whole = (value: unknown): number | undefined =>
  typeof value === 'number' && Number.isInteger(value) ? value : undefined

const isDevFx = (value: unknown): value is DevFx => DEV_FX.some((name) => name === value)

/** Reads `?seats=&board=&mode=&focus=&prompt=&turn=&step=&hand=&fx=&bar=&slow=&preview=&stops=&fc=&rules=&states=&cards=&menu=`, ignoring whatever does not make sense. The router has already parsed numbers. */
export function parseDevSearch(search: Record<string, unknown>): DevSearch {
  const seats = whole(search.seats)
  const focus = whole(search.focus)
  const mode = search.mode === 'grid' || search.mode === 'focus' ? search.mode : undefined
  const slow =
    typeof search.slow === 'number' && search.slow > 1 && search.slow <= MAX_DEV_SLOW
      ? search.slow
      : undefined
  const board = search.board === 'duel' || search.board === 'heavy' ? search.board : undefined
  return {
    // A board names a two-seat table.
    seats: board !== undefined ? 2 : seats === undefined || seats < 2 ? 4 : seats,
    board,
    mode,
    focus: focus === undefined || focus < 0 ? undefined : focus,
    prompt: typeof search.prompt === 'string' ? search.prompt : undefined,
    turn: search.turn === 'you' ? 'you' : undefined,
    fx: isDevFx(search.fx) ? search.fx : undefined,
    bar: search.bar === true || search.bar === 1 || search.bar === '1' ? true : undefined,
    slow,
    // The router validates an address again with what it made of it, so both keep the form the address has.
    stops:
      typeof search.stops === 'string' && parseStopSpec(search.stops).length > 0 ? search.stops : undefined,
    fc: search.fc === 'next' || search.fc === 'held' ? search.fc : undefined,
    cards: search.cards === 'real' ? 'real' : undefined,
    step: typeof search.step === 'string' ? STEP_BY_NAME.get(search.step.toLowerCase()) : undefined,
    hand:
      typeof search.hand === 'number' &&
      Number.isInteger(search.hand) &&
      search.hand >= 0 &&
      search.hand <= 24
        ? search.hand
        : undefined,
    menu: menuOf(search.menu),
    panel: panelOf(search.panel),
    rules: search.rules === true || search.rules === 1 || search.rules === '1' ? true : undefined,
    states: search.states === true || search.states === 1 || search.states === '1' ? true : undefined,
    // The equip move needs the attachments scene, so it brings it.
    equip:
      search.equip === true || search.equip === 1 || search.equip === '1' || search.fx === 'equip'
        ? true
        : undefined,
    preview:
      typeof search.preview === 'number' &&
      Number.isInteger(search.preview) &&
      search.preview >= 0 &&
      search.preview <= MAX_DEV_PREVIEW_MS
        ? search.preview
        : undefined,
  }
}
