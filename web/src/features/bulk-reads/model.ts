/** The bulk read list as the page shows it: the cards in groups by the equivalence they need. */
import type { BulkCard, BulkReads, BulkRule } from '@/protocol/generated'

/** One group of the page: a rule and its cards in list order. */
export interface RuleGroup {
  rule: BulkRule
  cards: BulkCard[]
}

/** The cards by their `rule`, groups in the order of `reads.rules`; a rule the list does not name comes last. */
export function groupByRule(reads: BulkReads): RuleGroup[] {
  const groups = new Map<string, RuleGroup>()
  for (const rule of reads.rules) groups.set(rule.name, { rule, cards: [] })
  for (const card of reads.cards) {
    let group = groups.get(card.rule)
    if (group === undefined) {
      group = { rule: { name: card.rule, title: card.rule.replaceAll('_', ' '), summary: '' }, cards: [] }
      groups.set(card.rule, group)
    }
    group.cards.push(card)
  }
  return [...groups.values()].filter((group) => group.cards.length > 0)
}

/** "1 card approved", "12 cards approved". */
export function wordingOnly(count: number, what: string): string {
  return `${count} ${count === 1 ? 'card' : 'cards'} ${what}`
}

/** The read page's link text: "12 wording-only, approve in bulk". */
export function bulkLinkText(count: number): string {
  return `${count} wording-only, approve in bulk`
}
