import type { StackStops } from '@/protocol/generated'

/** What one stack-stops level reads as: the menu row and what it does. */
export interface StackStopsWords {
  label: string
  description: string
}

/**
 * The words of each level of `AutoChoices.stack_stops` (`play/stackStops.ts`). The server decides from the stack alone
 * (who controls the object on top and what it targets), so the pace shows the setting and never the hand.
 */
export const STACK_STOPS_WORDS: Record<StackStops, StackStopsWords> = {
  All: {
    label: 'Every spell and ability',
    description: 'You are asked at each spell and ability you could respond to.',
  },
  Opponents: {
    label: "Opponents' only",
    description: "Your own and your team's spells and abilities resolve without asking you.",
  },
  Aimed: {
    label: 'Only what targets me or mine',
    description:
      "Opponents' spells and abilities also resolve without asking, unless one targets you, your team or something you control.",
  },
}

/** The accessible name of the button: what the seat is asked at now. */
export function stackStopsName(level: StackStops): string {
  return `Stack stops: ${STACK_STOPS_WORDS[level].label.toLowerCase()}`
}
