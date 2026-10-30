/**
 * The words on the controls of the Priority prompt, from what the table shows: the step, whose turn it is and the top
 * of the stack. Display only: what a button does is decided by the server and the engine, never by these.
 */
import type { PlayState } from '@/play/store'
import type { Step, YieldUntil } from '@/protocol/generated'
import type { DockForm } from '../dockLayout'
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

/** What a step is called after the one before it: in full, and in the short form of a narrow dock. */
interface StepWords {
  full: string
  short: string
}

const words = (full: string, short: string = full): StepWords => ({ full, short })

/**
 * What the step after this one is called ("Next: Combat"), by the step a pass leaves; null in the last window of the
 * turn, where the button says "End turn". Untap and Cleanup never give priority, so their words are never seen.
 */
const NEXT_STEP: Record<Step, StepWords | null> = {
  Untap: words('Upkeep'),
  Upkeep: words('Draw'),
  Draw: words('Precombat main', 'Main 1'),
  Main1: words('Combat'),
  BeginCombat: words('Declare attackers', 'Attackers'),
  DeclareAttackers: words('Declare blockers', 'Blockers'),
  DeclareBlockers: words('Combat damage', 'Damage'),
  FirstStrikeDamage: words('Combat damage', 'Damage'),
  CombatDamage: words('End of combat', 'End combat'),
  EndCombat: words('Postcombat main', 'Main 2'),
  Main2: words('End step'),
  End: null,
  Cleanup: null,
}

/**
 * The label of the primary button, which passes priority: "Resolve Lightning Bolt" over a spell, on the viewer's turn
 * "Next: Combat" (or "End turn" in the last window), and "Continue" on anybody else's turn. Never "pass priority".
 * A dock without room for whole words (`short`, `compact`) leaves the spell's name to the stack strip, where it is
 * the top chip ("Resolve"), and names the next step in one or two short words ("Next: Attackers").
 */
export function primaryLabel({ step, yourTurn, top }: PassSituation, form: DockForm = 'full'): string {
  const whole = form === 'full'
  if (top !== null) return whole ? `Resolve ${top}` : 'Resolve'
  if (!yourTurn) return 'Continue'
  const next = NEXT_STEP[step]
  if (next === null) return 'End turn'
  return `Next: ${whole ? next.full : next.short}`
}

/** Whether the primary button already ends the turn, so a second "End turn" beside it would only repeat it. */
export function primaryEndsTurn(situation: PassSituation): boolean {
  return primaryLabel(situation) === 'End turn'
}

/**
 * Whether Enter asks for the end-of-turn yield, as the "End turn" button beside the primary one does. It does not where
 * that button is not drawn: under Full Control, which the server does not let a yield run beside, and in the last
 * window of the turn, where the primary button already ends it. Enter then passes once, like Space.
 */
export function enterYields(situation: PassSituation, fullControl: boolean): boolean {
  return !fullControl && !primaryEndsTurn(situation)
}

/** What the menu of ways to pass says about the two keys. */
export const KEY_LEGEND = 'Space passes once, Enter ends the turn'

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
