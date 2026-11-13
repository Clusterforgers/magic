import type { CardsTotal, ObjectId } from '@/protocol/generated'

/** The value of a card on offer under a total; 0 for a card the options do not list. */
function valueOf(from: readonly ObjectId[], total: CardsTotal, id: ObjectId): number {
  const at = from.indexOf(id)
  return at < 0 ? 0 : (total.values[at] ?? 0)
}

/** The values of the chosen cards added up. */
export function chosenSum(from: readonly ObjectId[], total: CardsTotal, chosen: readonly ObjectId[]): number {
  return chosen.reduce((sum, id) => sum + valueOf(from, total, id), 0)
}

/** The cards not chosen that would take the chosen ones over the total; none without a total. */
export function overTotal(
  from: readonly ObjectId[],
  total: CardsTotal | null | undefined,
  chosen: readonly ObjectId[],
): Set<ObjectId> {
  const shut = new Set<ObjectId>()
  if (total == null) return shut
  const sum = chosenSum(from, total, chosen)
  for (const id of from) {
    if (!chosen.includes(id) && sum + valueOf(from, total, id) > total.most) shut.add(id)
  }
  return shut
}

/** Whether the chosen cards fit the total; always without one. */
export function fitsCardsTotal(
  from: readonly ObjectId[],
  total: CardsTotal | null | undefined,
  chosen: readonly ObjectId[],
): boolean {
  return total == null || chosenSum(from, total, chosen) <= total.most
}

/** "Total power 3 of at most 4." for a choice with a total; empty without one. */
export function cardsTotalWords(
  from: readonly ObjectId[],
  total: CardsTotal | null | undefined,
  chosen: readonly ObjectId[],
): string {
  if (total == null) return ''
  return `Total ${total.stat} ${chosenSum(from, total, chosen)} of at most ${total.most}.`
}
