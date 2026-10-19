/** Events and moments for the turn cue tests: a batch as the server sends it, put together from a few words. */
import { buildFixture, NAMES } from '@/features/table/dev/fixture'
import type { Moment } from '@/features/table/fx/derive'
import { indexById } from '@/play/view'
import type { Event, EventView, HiddenEvent, PlayerId, Stage, Step, StateView } from '@/protocol/generated'

/** An event every seat sees. */
export const pub = (event: Event): EventView => ({ Public: event })

/** An event as a seat that may not see the cards learns of it. */
export const hid = (event: HiddenEvent): EventView => ({ Hidden: event })

/** A step begins. */
export const begin = (step: Step): EventView => pub({ StepBegan: { step } })

/** A seat is given priority. */
export const grant = (player: PlayerId): EventView => pub({ PriorityGranted: { player } })

/** A seat passes priority. */
export const pass = (player: PlayerId): EventView => pub({ PriorityPassed: { player } })

/** A turn begins. */
export const turnBegan = (number: number, active: PlayerId): EventView =>
  pub({ TurnBegan: { number, active } })

/** A round of priority that nobody is asked about: every seat is given it and passes, starting with the active one. */
export function round(active: PlayerId, seats = 2): EventView[] {
  return Array.from({ length: seats }, (_, offset) => (active + offset) % seats).flatMap((seat) => [
    grant(seat),
    pass(seat),
  ])
}

/** A step that begins and passes with its round of priority. */
export const stepRound = (step: Step, active: PlayerId, seats = 2): EventView[] => [
  begin(step),
  ...round(active, seats),
]

/** The name a test table gives a seat (Alice, Bo, Cy ...). */
export const nameOf = (seat: PlayerId): string => NAMES[seat] ?? `Seat ${seat}`

/** Where a turn stands. */
export interface TurnAt {
  number: number
  active: PlayerId
  step: Step
}

/** A two-seat table, seen from seat 0, at `at`; `stage` is Turns unless told otherwise. */
export function momentAt(at: TurnAt, stage: Stage = 'Turns', over: Partial<StateView> = {}): Moment {
  const { view } = buildFixture({ seats: 2, permanents: 2 })
  const shown: StateView = { ...view, stage, turn: { ...view.turn, ...at }, ...over }
  return { view: shown, objectsById: indexById(shown.objects) }
}

/**
 * The end of one turn and the start of the next with nobody asked: the End step, the next seat's Upkeep and Draw
 * (it draws a card, seen as any other seat sees it), and the first window of its Main step.
 */
export function passTurn(next: number, active: PlayerId, seats = 2): EventView[] {
  const before = (active + seats - 1) % seats
  return [
    pass(before),
    ...stepRound('End', before, seats),
    begin('Cleanup'),
    turnBegan(next, active),
    begin('Untap'),
    ...stepRound('Upkeep', active, seats),
    begin('Draw'),
    hid({ CardDrawn: { player: active } }),
    ...round(active, seats),
    begin('Main1'),
    grant(active),
  ]
}
