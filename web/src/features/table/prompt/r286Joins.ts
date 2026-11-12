// The creatures a declaration of attackers or blockers holds against their join rules: a creature that can take part only
// beside enough others of a kind ("can't attack unless at least two other creatures attack"), or only alone.
import type { JoinRule } from '@/protocol/generated/JoinRule'
import type { ObjectId } from '@/protocol/generated/ObjectId'

/** The words a creature's join rule adds to its note, with `verb` "attack" or "block". */
export function joinWords(rule: JoinRule, verb: 'attack' | 'block'): string {
  if (rule.alone) return `can ${verb} only alone`
  const others = rule.at_least === 1 ? 'another creature' : `at least ${rule.at_least} other creatures`
  return `can't ${verb} unless ${others} also ${verb}${rule.at_least === 1 ? 's' : ''}`
}

/** The declared creatures whose join rule the declaration breaks. */
export function brokenJoins(
  rules: readonly JoinRule[] | undefined,
  declared: readonly ObjectId[],
): JoinRule[] {
  const chosen = new Set(declared)
  return (rules ?? []).filter((rule) => {
    if (!chosen.has(rule.object)) return false
    if (rule.alone) return chosen.size > 1
    const joined = rule.among.filter((other) => other !== rule.object && chosen.has(other)).length
    return joined < rule.at_least
  })
}
