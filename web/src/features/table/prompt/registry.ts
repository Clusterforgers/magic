import type { DecisionName } from '../turnText'
import { AssignDamagePrompt } from './kinds/AssignDamage'
import { BottomCardsPrompt } from './kinds/BottomCards'
import { DeclareAttackersPrompt } from './kinds/DeclareAttackers'
import { DeclareBlockersPrompt } from './kinds/DeclareBlockers'
import { DiscardPrompt } from './kinds/Discard'
import { MulliganPrompt } from './kinds/Mulligan'
import { PayManaPrompt } from './kinds/PayMana'
import { PriorityPrompt } from './kinds/Priority'
import { UnsupportedPrompt } from './PromptLayout'
import type { PromptComponent } from './types'

/**
 * The prompt of every decision kind, one line each; a new kind stops compiling here until it has one. A kind that is
 * not built yet points at the placeholder, which says so and offers nothing; its task replaces the line with a prompt
 * in `kinds/` (this file stays).
 */
export const PROMPTS: { [K in DecisionName]: PromptComponent<K> } = {
  Mulligan: MulliganPrompt,
  BottomCards: BottomCardsPrompt,
  Priority: PriorityPrompt,
  PayMana: PayManaPrompt,
  DeclareAttackers: DeclareAttackersPrompt,
  DeclareBlockers: DeclareBlockersPrompt,
  AssignDamage: AssignDamagePrompt,
  Discard: DiscardPrompt,
  ChooseTargets: UnsupportedPrompt,
  ChooseMode: UnsupportedPrompt,
  ChooseNumber: UnsupportedPrompt,
  DivideAmount: UnsupportedPrompt,
  ChooseCards: UnsupportedPrompt,
  OrderCards: UnsupportedPrompt,
  YesNo: UnsupportedPrompt,
  ChooseOption: UnsupportedPrompt,
  ChooseCosts: UnsupportedPrompt,
  OrderTriggers: UnsupportedPrompt,
  ChooseReplacement: UnsupportedPrompt,
}
