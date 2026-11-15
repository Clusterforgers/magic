import type { DeckGroupView } from '@/protocol/generated'

/** The groups the decks page offers in one click to an account that has none yet. */
export const PRESET_GROUPS: readonly string[] = ['Fun', 'Hardcore', 'Dungeon']

/** The key of the section of decks in no group. */
export const UNGROUPED = 'ungrouped'

/** A group as the lists need it: its id, name and deck ids. */
export type GroupLike = Pick<DeckGroupView, 'id' | 'name' | 'decks'>

/** One section of a grouped list: a group, or the decks in none (`group` null). */
export interface Section<T> {
  key: string
  group: GroupLike | null
  items: T[]
}

/**
 * The items in the groups' order, then those in no group, each section keeping the items' own order. An item in
 * several groups is in each of their sections, and in Ungrouped only when it is in none. `keepEmpty` keeps sections
 * without items (the decks page shows every group as a place to drop a deck).
 */
export function sections<T extends { id: string }>(
  items: readonly T[],
  groups: readonly GroupLike[],
  keepEmpty = false,
): Section<T>[] {
  const homes = membership(groups)
  const byGroup = new Map<string, T[]>(groups.map((group) => [group.id, []]))
  const loose: T[] = []
  for (const item of items) {
    let placed = false
    for (const id of homes.get(item.id) ?? []) {
      byGroup.get(id)?.push(item)
      placed = true
    }
    if (!placed) loose.push(item)
  }
  const all: Section<T>[] = [
    ...groups.map((group) => ({ key: group.id, group, items: byGroup.get(group.id) ?? [] })),
    { key: UNGROUPED, group: null, items: loose },
  ]
  return keepEmpty ? all : all.filter((section) => section.items.length > 0)
}

/** The ids of the groups each deck sits in, in the owner's order of the groups; a deck in none has no entry. */
export function membership(groups: readonly GroupLike[]): Map<string, string[]> {
  const homes = new Map<string, string[]>()
  for (const group of groups) {
    for (const deck of group.decks) {
      const held = homes.get(deck)
      if (held === undefined) homes.set(deck, [group.id])
      else if (!held.includes(group.id)) held.push(group.id)
    }
  }
  return homes
}

/** The ids of the groups the deck sits in, in the owner's order; empty when it is in none. */
export function groupsOf(groups: readonly GroupLike[], deckId: string): string[] {
  return groups.filter((group) => group.decks.includes(deckId)).map((group) => group.id)
}

/** The id of the first group the deck sits in, or null. */
export function groupOf(groups: readonly GroupLike[], deckId: string): string | null {
  return groupsOf(groups, deckId)[0] ?? null
}

/** The groups with the deck added to `target`, at the front; the deck's other groups stay. */
export function added<G extends GroupLike>(groups: readonly G[], deckId: string, target: string): G[] {
  return groups.map((group) =>
    group.id === target && !group.decks.includes(deckId)
      ? { ...group, decks: [deckId, ...group.decks] }
      : group,
  )
}

/** The groups with the deck taken out of `from`; the deck's other groups stay. */
export function removed<G extends GroupLike>(groups: readonly G[], deckId: string, from: string): G[] {
  return groups.map((group) =>
    group.id === from ? { ...group, decks: group.decks.filter((deck) => deck !== deckId) } : group,
  )
}

/** The groups with the one at `id` moved `by` places, kept inside the list. */
export function moved<G extends { id: string }>(groups: readonly G[], id: string, by: number): G[] {
  const from = groups.findIndex((group) => group.id === id)
  if (from < 0) return [...groups]
  const to = Math.min(groups.length - 1, Math.max(0, from + by))
  const next = [...groups]
  const [taken] = next.splice(from, 1)
  if (taken !== undefined) next.splice(to, 0, taken)
  return next
}

/** Most characters in a group's name (`MAX_DECK_GROUP_NAME_CHARS`). */
export const MAX_GROUP_NAME = 60

/** What is wrong with a group's name, in words; null when nothing is. */
export function groupNameProblem(name: string): string | null {
  const trimmed = name.trim()
  if (trimmed === '') return 'Give the group a name.'
  if ([...trimmed].length > MAX_GROUP_NAME) return `A group's name has at most ${MAX_GROUP_NAME} characters.`
  return null
}
