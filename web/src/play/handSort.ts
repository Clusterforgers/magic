/** The account's auto sort of the hand dock as a preference: its choices, its default and how a stored one is read. */
import type { HandGroup, HandSort, HandSortBy } from '@/protocol/generated'

/** The auto sorts in menu order, with their labels. */
export const HAND_SORTS: ReadonlyArray<{ value: HandSortBy; label: string }> = [
  { value: 'Drawn', label: 'As drawn' },
  { value: 'ManaValue', label: 'Mana value' },
  { value: 'Groups', label: 'By type' },
]

/** Every group in the order an account starts with, with its label. */
export const HAND_GROUPS: ReadonlyArray<{ value: HandGroup; label: string }> = [
  { value: 'Land', label: 'Lands' },
  { value: 'Creature', label: 'Creatures' },
  { value: 'Artifact', label: 'Artifacts' },
  { value: 'Enchantment', label: 'Enchantments' },
  { value: 'Planeswalker', label: 'Planeswalkers' },
  { value: 'Battle', label: 'Battles' },
  { value: 'Instant', label: 'Instants' },
  { value: 'Sorcery', label: 'Sorceries' },
]

/** The sort of an account that chose none: as drawn, the groups in the starting order. */
export const DEFAULT_HAND_SORT: HandSort = { by: 'Drawn', groups: HAND_GROUPS.map((group) => group.value) }

function isGroup(value: unknown): value is HandGroup {
  return HAND_GROUPS.some((group) => group.value === value)
}

/**
 * `groups` as a full order: the known groups it names, each once, then the ones it left out in the starting order. A
 * group a newer server names and this page does not know is dropped.
 */
export function fullGroups(groups: readonly unknown[]): HandGroup[] {
  const named = [...new Set(groups.filter(isGroup))]
  return [...named, ...HAND_GROUPS.map((group) => group.value).filter((group) => !named.includes(group))]
}

/** A stored or sent sort, or null for anything that is not one; its groups made a full order. */
export function readHandSort(value: unknown): HandSort | null {
  if (typeof value !== 'object' || value === null) return null
  const { by, groups } = value as Record<string, unknown>
  if (!HAND_SORTS.some((sort) => sort.value === by) || !Array.isArray(groups)) return null
  return { by: by as HandSortBy, groups: fullGroups(groups) }
}

/** The sort an older page kept under `magic.handSort`: "types" was lands, creatures, the rest; "mana" mana value. */
export function legacyHandSort(kept: string | null): HandSort | null {
  if (kept === 'types') return { ...DEFAULT_HAND_SORT, by: 'Groups' }
  if (kept === 'mana') return { ...DEFAULT_HAND_SORT, by: 'ManaValue' }
  return null
}

/** Whether two sorts order a hand the same. */
export function sameHandSort(a: HandSort, b: HandSort): boolean {
  return (
    a.by === b.by &&
    a.groups.length === b.groups.length &&
    a.groups.every((group, i) => group === b.groups[i])
  )
}
