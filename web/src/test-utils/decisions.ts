/** Valid decisions of every kind, for tests of the prompt layer. */
import type { Decision, DecisionKind, PlayerId } from '@/protocol/generated'
import type { DecisionName } from '@/features/table/turnText'

/** One minimal, valid decision kind of each name. */
export const KINDS: Record<DecisionName, DecisionKind> = {
  Mulligan: {
    Mulligan: {
      hand: [],
      mulligans_taken: 0,
      can_mulligan: true,
      free_mulligans: 0,
      bottom_if_keep: 0,
      bottom_if_mulligan: 1,
    },
  },
  BottomCards: { BottomCards: { hand: [], count: 1 } },
  Priority: {
    Priority: { playable_lands: [], castable: [], mana_abilities: [], can_act: false, can_pass: true },
  },
  PayMana: {
    PayMana: {
      spell: 1,
      cost: [{ Generic: 1 }],
      pool: { counts: [0, 0, 0, 0, 0, 0] },
      sources: [],
      auto_available: false,
      can_cancel: true,
      life: 20,
      max_life: 0,
    },
  },
  DeclareAttackers: { DeclareAttackers: { attackers: [] } },
  DeclareBlockers: { DeclareBlockers: { blockers: [] } },
  AssignDamage: { AssignDamage: { attacker: 1, power: 2, blockers: [] } },
  Discard: { Discard: { hand: [], count: 1 } },
  ChooseTargets: { ChooseTargets: { source: 1, spec: 0, legal: [], min: 1, max: 1, can_cancel: true } },
  ChooseMode: { ChooseMode: { source: 1, modes: [], min: 1, max: 1, repeat: false, can_cancel: true } },
  ChooseNumber: { ChooseNumber: { source: 1, min: 0, max: null } },
  DivideAmount: { DivideAmount: { source: 1, total: 2, among: [] } },
  ChooseCards: { ChooseCards: { source: 1, from: [], min: 1, max: 1 } },
  OrderCards: { OrderCards: { source: 1, cards: [] } },
  YesNo: { YesNo: { source: 1 } },
  ChooseOption: { ChooseOption: { source: 1, options: [] } },
  ChooseCosts: { ChooseCosts: { source: 1, optional: [] } },
  OrderTriggers: { OrderTriggers: { triggers: [] } },
  ChooseReplacement: { ChooseReplacement: { candidates: [] } },
}

/** A decision of the named kind for `player`. */
export function decisionFor(name: DecisionName, player: PlayerId = 0): Decision {
  return { player, kind: KINDS[name] }
}
