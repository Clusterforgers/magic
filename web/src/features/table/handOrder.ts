/**
 * The order the viewer's own hand dock draws its cards in (pure): an auto sort over the server's hand, or a custom
 * order the viewer dragged, merged with the cards that came and went since. The server's hand order is untouched.
 */
import type { CardType, ObjectId } from '@/protocol/generated'

/** The auto sorts of the hand: lands, creatures, other spells; mana value; the server's order ("as drawn"). */
export type HandSort = 'types' | 'mana' | 'drawn'

/** What the dock draws: one of the auto sorts, or the order the viewer dragged. */
export type HandOrderMode = HandSort | 'custom'

/** The auto sorts in menu order, with their labels. */
export const HAND_SORTS: ReadonlyArray<{ value: HandSort; label: string }> = [
  { value: 'types', label: 'Lands, creatures, spells' },
  { value: 'mana', label: 'Mana value' },
  { value: 'drawn', label: 'As drawn' },
]

/** True for a stored value that names an auto sort. */
export function isHandSort(value: unknown): value is HandSort {
  return value === 'types' || value === 'mana' || value === 'drawn'
}

/** What a sort reads of a card. */
export interface HandFacts {
  types: readonly CardType[]
  manaValue: number
  name: string
}

/** The group of a card under "Lands, creatures, spells": 0 a land (a land creature too), 1 a creature, 2 the rest. */
export function handGroup(types: readonly CardType[]): number {
  if (types.includes('Land')) return 0
  if (types.includes('Creature')) return 1
  return 2
}

/**
 * The hand in the order of `sort`. Under `types` lands, then creatures, then other spells, each group by mana value
 * then name; under `mana` by mana value then name; `drawn` keeps `ids`. Ties and unknown cards keep the server order.
 */
export function sortHand(
  ids: readonly ObjectId[],
  facts: (id: ObjectId) => HandFacts | undefined,
  sort: HandSort,
): ObjectId[] {
  if (sort === 'drawn') return [...ids]
  const keyed = ids.map((id, index) => {
    const fact = facts(id)
    return {
      id,
      index,
      group: sort !== 'types' ? 0 : fact === undefined ? 2 : handGroup(fact.types),
      value: fact?.manaValue ?? 0,
      name: fact?.name ?? '',
    }
  })
  keyed.sort(
    (a, b) => a.group - b.group || a.value - b.value || a.name.localeCompare(b.name) || a.index - b.index,
  )
  return keyed.map((entry) => entry.id)
}

/**
 * A custom order brought up to date with the hand: the cards still in hand keep their custom places, cards that left
 * drop out, and cards new to the hand follow at the end in the server's order.
 */
export function mergeCustom(custom: readonly ObjectId[], ids: readonly ObjectId[]): ObjectId[] {
  const inHand = new Set(ids)
  const kept = [...new Set(custom)].filter((id) => inHand.has(id))
  const placed = new Set(kept)
  return [...kept, ...ids.filter((id) => !placed.has(id))]
}

/** `order` with the card at `from` taken out and put at `to`; an index out of range changes nothing. */
export function moveCard(order: readonly ObjectId[], from: number, to: number): ObjectId[] {
  const moved = [...order]
  if (from < 0 || from >= order.length || to < 0 || to >= order.length || from === to) return moved
  const [card] = moved.splice(from, 1)
  if (card !== undefined) moved.splice(to, 0, card)
  return moved
}

/** A stored custom order: a list of object ids, or null for anything else. */
export function parseCustom(raw: string | null | undefined): ObjectId[] | null {
  if (raw == null) return null
  try {
    const value: unknown = JSON.parse(raw)
    if (!Array.isArray(value) || !value.every((id) => Number.isInteger(id))) return null
    return value as ObjectId[]
  } catch {
    return null
  }
}
