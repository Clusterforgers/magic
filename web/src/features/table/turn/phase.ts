/** The four phases the table themes: which steps belong to each, and the name and icon each carries beside its colour. */
import { MoonIcon, SunriseIcon, SwordsIcon, WandSparklesIcon } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import type { Step } from '@/protocol/generated'

/** A theme of the turn; it is the table root's `data-phase`, and the stylesheet names its tokens after it. */
export type PhaseId = 'beginning' | 'main' | 'combat' | 'ending'

/** A phase as the badge and the ladder show it: a name and an icon, so its colour is never the only sign. */
export interface Phase {
  id: PhaseId
  name: string
  icon: LucideIcon
}

/** Every phase by id. */
export const PHASES: Readonly<Record<PhaseId, Phase>> = {
  beginning: { id: 'beginning', name: 'Beginning', icon: SunriseIcon },
  main: { id: 'main', name: 'Main', icon: WandSparklesIcon },
  combat: { id: 'combat', name: 'Combat', icon: SwordsIcon },
  ending: { id: 'ending', name: 'Ending', icon: MoonIcon },
}

// A record over `Step`, so a step the engine adds stops compiling here until it has a theme.
const PHASE_OF: Readonly<Record<Step, PhaseId>> = {
  Untap: 'beginning',
  Upkeep: 'beginning',
  Draw: 'beginning',
  Main1: 'main',
  BeginCombat: 'combat',
  DeclareAttackers: 'combat',
  DeclareBlockers: 'combat',
  FirstStrikeDamage: 'combat',
  CombatDamage: 'combat',
  EndCombat: 'combat',
  Main2: 'main',
  End: 'ending',
  Cleanup: 'ending',
}

/**
 * The phase a step belongs to: untap, upkeep and draw are the beginning; both main phases are main; beginning of
 * combat to end of combat, first-strike damage included, are combat; the end step and cleanup are the ending. It reads
 * the step alone, which every seat sees, so every seat sees the same theme.
 */
export function phaseOf(step: Step): Phase {
  return PHASES[PHASE_OF[step]]
}
