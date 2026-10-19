/** What the stop ladder shows and says: the steps it draws, how a stop is held, what a press asks for, the spoken words. */
import { STOP_STEPS } from '@/play/stops'
import type { StopStep } from '@/play/stops'
import type { FullControlMode, PlayerId, Step, Stop, StopSide } from '@/protocol/generated'
import { STEP_LABEL } from '../turnText'
import { FULL_CONTROL_STATE } from './fullControlWords'

const WITHOUT_FIRST_STRIKE: readonly StopStep[] = STOP_STEPS.filter((step) => step !== 'FirstStrikeDamage')

/**
 * The steps the ladder draws, in turn order. First-strike damage is drawn only while it is the current step or a stop
 * is held there, so a game without first strikers shows ten.
 */
export function ladderSteps(current: Step | null, stops: readonly Stop[]): readonly StopStep[] {
  const shown = current === 'FirstStrikeDamage' || stops.some((stop) => stop.step === 'FirstStrikeDamage')
  return shown ? STOP_STEPS : WITHOUT_FIRST_STRIKE
}

/** How a stop is held: not at all, for one turn (gone once its step has passed) or lasting (until cleared). */
export type StopState = 'off' | 'turn' | 'lasting'

/** The state of the stop at `side` and `step` in the server's list. */
export function stopState(stops: readonly Stop[], side: StopSide, step: Step): StopState {
  const held = stops.find((stop) => stop.side === side && stop.step === step)
  if (held === undefined) return 'off'
  return held.keep ? 'lasting' : 'turn'
}

/** Where the ladder is drawn: `row` is one line in the controls dock, `panel` wraps the steps for a popover. */
export type StopsLayout = 'row' | 'panel'

/** The two sides of the ladder, in the words of the switch that picks the row on show. */
export const SIDE_WORD: Record<StopSide, string> = { Own: 'Mine', Opponent: 'Theirs' }

/** The side whose turn it is, seen from `seat`: the viewer's own turns are `Own`, any other seat's `Opponent`. */
export function turnSide(active: PlayerId, seat: PlayerId): StopSide {
  return active === seat ? 'Own' : 'Opponent'
}

/** The arguments of a `SetStop` after the side and step. */
export interface StopRequest {
  on: boolean
  keep: boolean
}

/**
 * What a press asks for at a step in `state`. A press stands for a lasting stop (`lasting`, from shift or the Keep
 * toggle) or a one-turn stop. It sets a stop that is off, clears one that is on, and makes a one-turn stop lasting
 * when it stands for a lasting one. Clearing sends the `keep` the stop had, which the server ignores.
 */
export function pressRequest(state: StopState, lasting: boolean): StopRequest {
  switch (state) {
    case 'off':
      return { on: true, keep: lasting }
    case 'turn':
      return lasting ? { on: true, keep: true } : { on: false, keep: false }
    case 'lasting':
      return { on: false, keep: true }
  }
}

const SIDE_NAME: Record<StopSide, string> = { Own: 'Your', Opponent: "Opponents'" }

const STATE_WORDS: Record<StopState, string> = {
  off: 'off',
  turn: 'on for one turn',
  lasting: 'on every turn',
}

/** What a step is called inside a sentence: "precombat main", "end step". */
export function stepWords(step: Step): string {
  return STEP_LABEL[step].toLowerCase()
}

/** What a press does, for the end of "Press to ...". */
function pressWords(side: StopSide, state: StopState, request: StopRequest): string {
  if (!request.on) return 'turn it off'
  if (request.keep) {
    if (state === 'turn') return 'keep it every turn'
    return side === 'Own' ? 'stop here every turn' : 'stop here on every opponent turn'
  }
  return side === 'Own' ? 'stop here this turn' : 'stop here on one opponent turn'
}

/**
 * The spoken text of a step's button: "Your upkeep stop: off. Press to stop here this turn". `lasting` is whether a
 * plain press stands for a lasting stop (the Keep toggle is on).
 */
export function stopName(side: StopSide, step: Step, state: StopState, lasting: boolean): string {
  const press = pressWords(side, state, pressRequest(state, lasting))
  return `${SIDE_NAME[side]} ${stepWords(step)} stop: ${STATE_WORDS[state]}. Press to ${press}`
}

/** What the other kind of press (with shift) does, or undefined when it does what a plain press does. */
export function stopDescription(side: StopSide, state: StopState, lasting: boolean): string | undefined {
  const plain = pressWords(side, state, pressRequest(state, lasting))
  const shifted = pressWords(side, state, pressRequest(state, !lasting))
  return plain === shifted ? undefined : `Shift and press to ${shifted}.`
}

/** The name of a row of the ladder, for a screen reader; `turn` says it is the turn of that side now. */
export function rowName(side: StopSide, turn: boolean): string {
  if (side === 'Own') return turn ? 'Stops on your turns, your turn is under way' : 'Stops on your turns'
  return turn ? "Stops on opponents' turns, an opponent's turn is under way" : "Stops on opponents' turns"
}

/** What the collapsed ladder says about the stops held: "none set", "3 set". */
export function stopCount(count: number): string {
  return count === 0 ? 'none set' : `${count} set`
}

/** How many stops the server holds on `side`. */
export function stopsOn(stops: readonly Stop[], side: StopSide): number {
  return stops.filter((stop) => stop.side === side).length
}

/** The spoken name of a side of the switch: "Mine, 2 set". */
export function sideName(side: StopSide, count: number): string {
  return `${SIDE_WORD[side]}, ${stopCount(count)}`
}

/** What a side of the switch shows when pressed, for the tooltip. */
export function sideTitle(side: StopSide): string {
  return side === 'Own' ? 'Show the stops for your own turns' : "Show the stops for opponents' turns"
}

/** The name of the "Stops" button that opens the ladder where the dock has no room for a row: "Stops, 2 set, Full Control held". */
export function stopsButtonName(count: number, fullControl: FullControlMode): string {
  const held = fullControl === 'Off' ? '' : `, Full Control ${FULL_CONTROL_STATE[fullControl]}`
  return `Stops, ${stopCount(count)}${held}`
}
