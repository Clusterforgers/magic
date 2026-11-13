/** The Delves pages' words and helpers (pure, apart from the deck lists): errors, statuses and relic roles. */
import { sections } from '@/features/decks/groups/grouping'
import type { GroupLike } from '@/features/decks/groups/grouping'
import { lastGroup, rememberGroup } from '@/features/decks/groups/lastGroup'
import { deckGroupsApi } from '@/net/deckGroups'
import { decksApi } from '@/net/decks'
import { DelveError } from '@/net/delves'
import { preconsApi } from '@/net/precons'
import type { DeckListItem } from '@/protocol/generated'

/** How often the run page reads the run again, in milliseconds. */
export const POLL_MS = 4000

export function errorText(error: unknown): string {
  return error instanceof DelveError ? error.message : 'Something went wrong. Try again.'
}

/** What the run is doing, in a word or two. */
export function statusText(status: string): string {
  const words: Record<string, string> = {
    lobby: 'Gathering',
    map: 'Choosing a path',
    fight: 'Fighting',
    room: 'Taking loot',
    victory: 'Victory',
    defeat: 'Defeated',
    abandoned: 'Abandoned',
  }
  return words[status] ?? status
}

/** The words a relic's role is shown with; a risky relic says that it helps the boss too. */
export function roleLabel(role: string): string {
  return role === 'Risky' ? 'Risky: helps the boss too' : role
}

/** A deck a hero may bring: its id for the server, its name and the group the choice lists it under. */
export interface DeckOption {
  value: string
  label: string
  /** The heading it is listed under: a deck group of the player's, "Ungrouped", "Your decks" or "Precons". */
  group?: string
  /** The id of the deck group it sits in, if any. */
  groupId?: string
}

/**
 * The player's own Commander decks in their deck groups (the last used group first, then the owner's order, then
 * Ungrouped; "Your decks" when the player has no groups), then the precons.
 */
export function groupedOptions(
  decks: readonly DeckListItem[],
  groups: readonly GroupLike[],
  precons: readonly { code: string; name: string }[],
  last: string | null,
): DeckOption[] {
  const own = decks.filter((d) => d.format === 'commander')
  const ordered = [...groups].sort((a, b) => Number(b.id === last) - Number(a.id === last))
  const mine = sections(own, ordered).flatMap((section) =>
    section.items.map((d) => ({
      value: d.id,
      label: d.name,
      groupId: section.group?.id,
      group: section.group?.name ?? (groups.length === 0 ? 'Your decks' : 'Ungrouped'),
    })),
  )
  return [...mine, ...precons.map((p) => ({ value: `precon:${p.code}`, label: p.name, group: 'Precons' }))]
}

/** The precons and the player's own Commander decks in their groups, for the deck choice. */
export async function deckOptions(): Promise<DeckOption[]> {
  const [precons, decks, groups] = await Promise.all([
    preconsApi.list().catch(() => ({ precons: [] })),
    decksApi.list().catch(() => []),
    deckGroupsApi.list().catch(() => []),
  ])
  return groupedOptions(decks, groups, precons.precons, lastGroup())
}

/** Remembers the group of the deck just chosen, so the next choice lists that group first. */
export function rememberChoice(decks: readonly DeckOption[], value: string): void {
  const chosen = decks.find((d) => d.value === value)
  if (chosen !== undefined && chosen.group !== 'Precons') rememberGroup(chosen.groupId ?? null)
}

/** The options under their headings, in order; an option without a heading goes under "Decks". */
export function optionGroups(decks: readonly DeckOption[]): [string, DeckOption[]][] {
  const by = new Map<string, DeckOption[]>()
  for (const d of decks) {
    const group = d.group ?? 'Decks'
    by.set(group, [...(by.get(group) ?? []), d])
  }
  return [...by.entries()]
}
