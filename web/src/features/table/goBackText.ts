/** Going back a step in words: the button, the request to the table, the log line and why a go-back was refused. */
import type { GoBackOffer, GoBackRefusal, PlayerId, Step, TakeBackOutcome } from '@/protocol/generated'
import { STEP_LABEL } from './turnText'

/** The key that goes back a step while nothing has the keyboard. */
export const GO_BACK_KEY = 'Backspace'

/** A step inside a sentence: "precombat main phase", "beginning of combat", "end step". */
export function stepPhrase(step: Step): string {
  const label = STEP_LABEL[step].toLowerCase()
  return step === 'Main1' || step === 'Main2' ? `${label} phase` : label
}

/** The button: "Back to Precombat main". */
export function goBackLabel(step: Step): string {
  return `Back to ${STEP_LABEL[step]}`
}

/**
 * The log line of `WentBack`: "Ana went back to the precombat main phase" in their own turn, "Ana went back to Bo's
 * end step" in another seat's.
 */
export function wentBackLine(
  player: PlayerId,
  step: Step,
  active: PlayerId,
  seat: (seat: PlayerId) => string,
): string {
  return `${seat(player)} went back to ${whoseStep(player, step, active, seat)}`
}

/** The step gone back to inside a sentence: "the precombat main phase" in the asker's turn, "Bo's end step" else. */
function whoseStep(player: PlayerId, step: Step, active: PlayerId, seat: (seat: PlayerId) => string): string {
  const whose = active === player ? 'the' : `${seat(active)}'s`
  return `${whose} ${stepPhrase(step)}`
}

/** The question to a seat asked: "Ana wants to go back to the precombat main phase". */
export function goBackAsk(offer: GoBackOffer, seat: (seat: PlayerId) => string): string {
  return `${seat(offer.player)} wants to go back to ${whoseStep(offer.player, offer.step, offer.active, seat)}`
}

/** Who the table still waits on: "Waiting for Bo…", "Waiting for Bo and Cy…", "Waiting for Bo, Cy and Di…". */
export function waitingFor(waiting: readonly PlayerId[], seat: (seat: PlayerId) => string): string {
  const names = waiting.map(seat)
  const last = names.pop()
  if (last === undefined) return 'Waiting…'
  return `Waiting for ${names.length === 0 ? last : `${names.join(', ')} and ${last}`}…`
}

/** How a request to go back ended, as one sentence. */
export function goBackOutcomeWords(
  offer: GoBackOffer,
  outcome: TakeBackOutcome,
  seat: (seat: PlayerId) => string,
): string {
  switch (outcome.type) {
    case 'Approved':
      return `Allowed: ${seat(offer.player)} goes back to ${whoseStep(offer.player, offer.step, offer.active, seat)}.`
    case 'Declined':
      return `Declined by ${seat(outcome.by)}: ${seat(offer.player)} does not go back.`
    case 'TimedOut':
      return `No answer from ${seat(outcome.by)}: ${seat(offer.player)} does not go back.`
    case 'Withdrawn':
      return `Withdrawn: ${seat(offer.player)} does not go back.`
  }
}

/** Why the seat could not go back, as one sentence. */
export const GO_BACK_REFUSAL_WORDS: Record<GoBackRefusal, string> = {
  Off: 'Going back is off in this game.',
  Nothing: 'There is no earlier step to go back to.',
  Busy: 'Finish or cancel the action under way first.',
  Acted: 'Too late to go back: a card was played or an ability used since.',
  Declared: 'Too late to go back: attackers or blockers were declared since.',
  Chose: 'Too late to go back: a choice was made since.',
  Changed: 'Too late to go back: the game changed since.',
  CardDrawn: 'Too late to go back: a card was drawn since.',
  CardRevealed: 'Too late to go back: a card was revealed since.',
  LibrarySearched: 'Too late to go back: a library was searched since.',
  LibraryShuffled: 'Too late to go back: a library was shuffled since.',
  Random: 'Too late to go back: something random happened since.',
  OtherPlayerActed: 'Too late to go back: another player has acted since.',
  MovedOn: 'The game is over.',
}
