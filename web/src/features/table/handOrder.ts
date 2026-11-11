/**
 * The order the viewer's own hand dock draws its cards in (pure): the account's auto sort over the server's hand, or a
 * custom order the viewer dragged, merged with the cards that came and went since. The server's hand order is untouched.
 */
import type { CardType, HandGroup, HandSort, HandSortBy, ObjectId } from '@/protocol/generated'

export type { HandGroup, HandSort, HandSortBy }
export { DEFAULT_HAND_SORT, HAND_GROUPS, HAND_SORTS } from '@/play/handSort'

/** What the dock draws: one of the auto sorts, or the order the viewer dragged. */
export type HandOrderMode = HandSortBy | 'custom'

/** `groups` with the group at `from` moved to `to`; an index out of range changes nothing. */
export function moveGroup(groups: readonly HandGroup[], from: number, to: number): HandGroup[] {
  const moved = [...groups]
  if (from < 0 || from >= groups.length || to < 0 || to >= groups.length || from === to) return moved
  const [group] = moved.splice(from, 1)
  if (group !== undefined) moved.splice(to, 0, group)
  return moved
}

/** What a sort reads of a card. */
export interface HandFacts {
  types: readonly CardType[]
  manaValue: number
  name: string
}

/** The place of a card among `groups`: the first group it belongs to (a land creature under "Land" when lands come first), after all of them when none. */
export function handGroup(types: readonly CardType[], groups: readonly HandGroup[]): number {
  const at = groups.findIndex((group) => types.includes(group))
  return at === -1 ? groups.length : at
}

/**
 * The hand in the order of `sort`. Under `Groups` each card in the first of the account's groups it belongs to, each
 * group by mana value then name; under `ManaValue` by mana value then name; `Drawn` keeps `ids`. Ties and unknown cards
 * keep the server order.
 */
export function sortHand(
  ids: readonly ObjectId[],
  facts: (id: ObjectId) => HandFacts | undefined,
  sort: HandSort,
): ObjectId[] {
  if (sort.by === 'Drawn') return [...ids]
  const keyed = ids.map((id, index) => {
    const fact = facts(id)
    return {
      id,
      index,
      group:
        sort.by !== 'Groups'
          ? 0
          : fact === undefined
            ? sort.groups.length
            : handGroup(fact.types, sort.groups),
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
