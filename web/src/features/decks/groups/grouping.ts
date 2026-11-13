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
 * The items in the groups' order, then those in no group, each section keeping the items' own order. `keepEmpty`
 * keeps sections without items (the decks page shows every group as a place to drop a deck).
 */
export function sections<T extends { id: string }>(
  items: readonly T[],
  groups: readonly GroupLike[],
  keepEmpty = false,
): Section<T>[] {
  const home = new Map<string, string>()
  for (const group of groups) for (const deck of group.decks) home.set(deck, group.id)
  const byGroup = new Map<string, T[]>(groups.map((group) => [group.id, []]))
  const loose: T[] = []
  for (const item of items) {
    const at = home.get(item.id)
    const bucket = at === undefined ? undefined : byGroup.get(at)
    if (bucket === undefined) loose.push(item)
    else bucket.push(item)
  }
  const all: Section<T>[] = [
    ...groups.map((group) => ({ key: group.id, group, items: byGroup.get(group.id) ?? [] })),
    { key: UNGROUPED, group: null, items: loose },
  ]
  return keepEmpty ? all : all.filter((section) => section.items.length > 0)
}

/** The id of the group the deck sits in, or null. */
export function groupOf(groups: readonly GroupLike[], deckId: string): string | null {
  return groups.find((group) => group.decks.includes(deckId))?.id ?? null
}

/** The groups with the deck moved into `target` (null: into none). */
export function filed<G extends GroupLike>(groups: readonly G[], deckId: string, target: string | null): G[] {
  return groups.map((group) => {
    const rest = group.decks.filter((deck) => deck !== deckId)
    return { ...group, decks: group.id === target ? [deckId, ...rest] : rest }
  })
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
