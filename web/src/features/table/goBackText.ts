/** Going back a step in words: the button, the log line and why a go-back was refused. */
import type { GoBackRefusal, PlayerId, Step } from '@/protocol/generated'
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
  const whose = active === player ? 'the' : `${seat(active)}'s`
  return `${seat(player)} went back to ${whose} ${stepPhrase(step)}`
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
