import type { DecisionName } from '../turnText'
import { AssignDamagePrompt } from './kinds/AssignDamage'
import { BottomCardsPrompt } from './kinds/BottomCards'
import { ChooseCardsPrompt } from './kinds/ChooseCards'
import { ChooseCostsPrompt } from './kinds/ChooseCosts'
import { ChooseModePrompt } from './kinds/ChooseMode'
import { ChooseNumberPrompt } from './kinds/ChooseNumber'
import { ChooseOptionPrompt } from './kinds/ChooseOption'
import { ChooseReplacementPrompt } from './kinds/ChooseReplacement'
import { ChooseTargetsPrompt } from './kinds/ChooseTargets'
import { DeclareAttackersPrompt } from './kinds/DeclareAttackers'
import { DeclareBlockersPrompt } from './kinds/DeclareBlockers'
import { DiscardPrompt } from './kinds/Discard'
import { DivideAmountPrompt } from './kinds/DivideAmount'
import { MulliganPrompt } from './kinds/Mulligan'
import { OrderCardsPrompt } from './kinds/OrderCards'
import { OrderTriggersPrompt } from './kinds/OrderTriggers'
import { PayManaPrompt } from './kinds/PayMana'
import { PriorityPrompt } from './kinds/Priority'
import { YesNoPrompt } from './kinds/YesNo'
import type { PromptComponent } from './types'

/**
 * The prompt of every decision kind, one line each; a new kind stops compiling here until it has one. A kind that is
 * not built yet points at a placeholder in `kinds/`, whose body is replaced with the real prompt (this file stays).
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
  ChooseTargets: ChooseTargetsPrompt,
  ChooseMode: ChooseModePrompt,
  ChooseNumber: ChooseNumberPrompt,
  DivideAmount: DivideAmountPrompt,
  ChooseCards: ChooseCardsPrompt,
  OrderCards: OrderCardsPrompt,
  YesNo: YesNoPrompt,
  ChooseOption: ChooseOptionPrompt,
  ChooseCosts: ChooseCostsPrompt,
  OrderTriggers: OrderTriggersPrompt,
  ChooseReplacement: ChooseReplacementPrompt,
}
