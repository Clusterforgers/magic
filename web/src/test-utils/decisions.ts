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
  Priority: { Priority: { playable_lands: [], castable: [], mana_abilities: [], can_pass: true } },
  PayMana: {
    PayMana: {
      spell: 1,
      cost: { generic: 1, colorless: 0, colored: [0, 0, 0, 0, 0] },
      pool: { counts: [0, 0, 0, 0, 0, 0] },
      sources: [],
      auto_available: false,
    },
  },
  DeclareAttackers: { DeclareAttackers: { attackers: [] } },
  DeclareBlockers: { DeclareBlockers: { blockers: [] } },
  AssignDamage: { AssignDamage: { attacker: 1, power: 2, blockers: [] } },
  Discard: { Discard: { hand: [], count: 1 } },
}

/** A decision of the named kind for `player`. */
export function decisionFor(name: DecisionName, player: PlayerId = 0): Decision {
  return { player, kind: KINDS[name] }
}
