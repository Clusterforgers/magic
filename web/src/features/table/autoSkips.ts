import type {
  AutoSkip,
  AutoYield,
  ObjectId,
  ObjectView,
  PlayerId,
  TriggerEntry,
  TriggerKey,
} from '@/protocol/generated'

/**
 * The rules by which the server answers this seat's questions about its own triggers (`AutoSkip`), keyed as the server
 * keys them: the yes or no of an own ability by its key on the stack, the order of triggers by their sources' names
 * and ability indices. Pure helpers for the prompts and the list in the Full Control menu.
 */

/** The key the server tells two rules apart by: the ability of an answer, the sorted set of an order. */
export function ruleKey(rule: AutoSkip): string {
  if (rule.kind === 'Answer') return `Answer\n${rule.ability.name}\n${rule.ability.text}`
  const set = rule.triggers.map((t) => `${t.name}\n${t.ability}`).sort()
  return `Order\n${set.join('\n')}`
}

/**
 * The key of the ability whose question `source` asks this seat, as the server reads it: the top of the stack when it
 * is an ability of the seat from that source with a name every player reads; null for any other question.
 */
export function answerKeyOf(
  top: ObjectView | undefined,
  seat: PlayerId | null,
  source: ObjectId,
): AutoYield | null {
  if (top === undefined || top.kind !== 'Ability' || top.name === '') return null
  if (top.controller !== seat || top.source !== source) return null
  return { name: top.name, text: top.rules_text ?? '' }
}

/** The order rule for `entries` put on the stack in `order` (indices, first on first), or null when a source has no name. */
export function orderRule(
  entries: readonly TriggerEntry[],
  names: readonly (string | undefined)[],
  order: readonly number[],
): AutoSkip | null {
  const triggers: TriggerKey[] = []
  for (const index of order) {
    const entry = entries[index]
    const name = names[index]
    if (entry === undefined || name === undefined || name === '') return null
    triggers.push({ name, ability: entry.ability })
  }
  return triggers.length < 2 ? null : { kind: 'Order', triggers }
}

/** What the list says of one rule, and the accessible name of its remove button. */
export function ruleWords(rule: AutoSkip): { label: string; remove: string } {
  if (rule.kind === 'Answer') {
    const answer = rule.yes ? 'Yes' : 'No'
    return {
      label: `${rule.ability.name}: always ${answer}`,
      remove: `Stop answering ${answer} for ${rule.ability.name}${rule.ability.text === '' ? '' : `: ${rule.ability.text}`}`,
    }
  }
  const names = rule.triggers.map((t) => t.name).join(', then ')
  return { label: `Order: ${names}`, remove: `Stop ordering ${names}` }
}

/** What the switch does, read with its name. */
export const AUTO_SKIP_DESCRIPTION =
  'Passes for you at once whenever a triggered ability is on top of the stack, for every game. Full Control asks anyway.'

/** The name of the list button: how many per-card rules the seat holds. */
export function autoSkipListName(count: number): string {
  if (count === 0) return 'Auto-skip list, empty'
  return `Auto-skip list, ${count} ${count === 1 ? 'rule' : 'rules'}`
}
