/** The words of the three questions the lobby asks before it acts, and of the notices that follow them. Pure, so tested as a table. */

/** What a confirmation is about. */
export type Question =
  | { kind: 'close' }
  | { kind: 'leave' }
  | { kind: 'remove'; seat: number; name: string }

/** One confirmation as the dialog shows it. */
export interface QuestionWords {
  title: string
  /** What happens if the answer is yes. */
  description: string
  /** The button that does it; it names what it does (and whom). */
  confirm: string
  /** The button that does not; it is first in the dialog and takes the focus. */
  cancel: string
}

/** The words of `question`. */
export function questionWords(question: Question): QuestionWords {
  switch (question.kind) {
    case 'close':
      return {
        title: 'Close this game?',
        description:
          'Everyone in the lobby is sent back to Play. The invite link and the code stop working, and the game cannot be opened again.',
        confirm: 'Close game',
        cancel: 'Keep game open',
      }
    case 'leave':
      return {
        title: 'Leave this game?',
        description:
          'Your seat opens for someone else and you go back to Play. To come back, join again from the game browser or with the code.',
        confirm: 'Leave game',
        cancel: 'Stay in the game',
      }
    case 'remove':
      return {
        title: `Remove ${question.name}?`,
        description: `${question.name} is taken out of seat ${question.seat + 1} and sent back to Play. The seat opens for someone else to take.`,
        confirm: `Remove ${question.name}`,
        cancel: `Keep ${question.name}`,
      }
  }
}

/** The notice on Play after the host closed the game. */
export const CLOSED_NOTICE = 'You closed the game. Everyone who was in the lobby is back on Play.'

/** The notice on Play after a guest gave their seat back. */
export const LEFT_NOTICE = 'You left the game.'

/** What the lobby says once a removal went through. */
export const removedNotice = (name: string, seat: number): string =>
  `${name} was removed. Seat ${seat + 1} is open.`

/** The line that replaces the controls once the game has started and nothing can be changed. */
export const STARTED_NOTICE = 'This game has already started, so the lobby can no longer be changed.'
