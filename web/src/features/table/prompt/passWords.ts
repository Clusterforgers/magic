/**
 * The words on the controls of the Priority prompt, from what the table shows: the step, whose turn it is and the top
 * of the stack. Display only: what a button does is decided by the server and the engine, never by these.
 */
import type { PlayState } from '@/play/store'
import type { Step, YieldUntil } from '@/protocol/generated'
import { topOfStack } from './priorityText'

/** What the table shows that the primary button's words depend on. */
export interface PassSituation {
  /** The step of the turn. */
  step: Step
  /** Whether it is the viewer's own turn. */
  yourTurn: boolean
  /** The name of the object on top of the stack, or null when the stack is empty. */
  top: string | null
}

/**
 * What the step after this one is called ("Next: Combat"), by the step a pass leaves; null in the last window of the
 * turn, where the button says "End turn". Untap and Cleanup never give priority, so their words are never seen.
 */
const NEXT_STEP: Record<Step, string | null> = {
  Untap: 'Upkeep',
  Upkeep: 'Draw',
  Draw: 'Precombat main',
  Main1: 'Combat',
  BeginCombat: 'Declare attackers',
  DeclareAttackers: 'Declare blockers',
  DeclareBlockers: 'Combat damage',
  FirstStrikeDamage: 'Combat damage',
  CombatDamage: 'End of combat',
  EndCombat: 'Postcombat main',
  Main2: 'End step',
  End: null,
  Cleanup: null,
}

/**
 * The label of the primary button, which passes priority: "Resolve Lightning Bolt" over a spell, on the viewer's turn
 * "Next: Combat" (or "End turn" in the last window), and "Continue" on anybody else's turn. Never "pass priority".
 */
export function primaryLabel({ step, yourTurn, top }: PassSituation): string {
  if (top !== null) return `Resolve ${top}`
  if (!yourTurn) return 'Continue'
  const next = NEXT_STEP[step]
  return next === null ? 'End turn' : `Next: ${next}`
}

/** Whether the primary button already ends the turn, so a second "End turn" beside it would only repeat it. */
export function primaryEndsTurn(situation: PassSituation): boolean {
  return primaryLabel(situation) === 'End turn'
}

/** What a screen reader is told about the primary button, which the label does not say. */
export function primaryDescription({ top }: PassSituation): string {
  return top === null ? 'Passes priority.' : `Passes priority; if nobody responds, ${top} resolves.`
}

/** The words of one kind of yield. */
export interface YieldWords {
  /** The button or menu item that asks for it. */
  action: string
  /** What the table is told once it is on. */
  passing: string
  /** What a screen reader hears on the button. */
  description: string
}

/** The words of each yield, which the server holds per seat. */
export const YIELD_WORDS: Record<YieldUntil, YieldWords> = {
  Resolve: {
    action: 'Resolve everything on the stack',
    passing: 'Passing until the stack is empty',
    description: 'Passes priority until the stack is empty. You are asked again if an opponent adds to it.',
  },
  EndOfTurn: {
    action: 'End turn',
    passing: 'Passing until the end of turn',
    description: 'Passes priority until the turn ends. You are asked again if an opponent adds to the stack.',
  },
  MyTurn: {
    action: 'Pass until my next turn',
    passing: 'Passing until your next turn',
    description:
      'Passes priority until your next turn begins. You are asked again if an opponent adds to the stack.',
  },
}

/** What the controls read off the play state: the step, whose turn it is and the stack; wrap in `useShallow`. */
export function passSituation(state: PlayState): PassSituation {
  const { view, seat } = state
  return {
    step: view?.turn.step ?? 'Main1',
    yourTurn: view !== null && seat !== null && view.turn.active === seat,
    top: topOfStack(state)?.name ?? null,
  }
}
