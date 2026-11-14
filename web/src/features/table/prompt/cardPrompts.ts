/** The prompts that lay out cards in the shared sheet, which then grows over the opponents' boards (`Regions.sheetWide`). */
import type { PlayState } from '@/play/store'

/** The decision kinds whose prompt draws a row of cards to look at or pick from. */
export const CARD_PROMPTS: ReadonlySet<string> = new Set([
  'ChooseCards',
  'ChooseTargets',
  'Scry',
  'OrderCards',
  'Mulligan',
])

/** Whether the viewer's pending decision lays out cards; wrap nothing, it is a boolean. */
export function laysOutCards(state: PlayState): boolean {
  const kind = state.pending?.decision.kind
  if (kind === undefined || typeof kind !== 'object') return false
  return Object.keys(kind).some((name) => CARD_PROMPTS.has(name))
}
