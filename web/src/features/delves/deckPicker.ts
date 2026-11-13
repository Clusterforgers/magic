/**
 * The model of the Delves deck picker: the Commander decks a hero may bring as entries of the game lobby's chooser,
 * their sections, search and colour filter, the words of their validity and when the player last took each on a delve.
 */
import type { KeyValueStore } from '@/lib/invites'
import { decksApi } from '@/net/decks'
import { preconsApi } from '@/net/precons'
import { entryOfDeck, entryOfPrecon, filterEntries } from '@/features/game/deckEntries'
import type { DeckEntry } from '@/features/game/deckEntries'

/** The colours of the filter, in WUBRG order. */
export const COLOURS = ['W', 'U', 'B', 'R', 'G'] as const

/** A colour's name, for labels. */
export const COLOUR_NAME: Record<string, string> = {
  W: 'White',
  U: 'Blue',
  B: 'Black',
  R: 'Red',
  G: 'Green',
}

/** What the picker narrows its decks to: search words and colours every listed deck holds. */
export interface PickerFilter {
  query: string
  colours: readonly string[]
}

/** One collapsible group of the picker. */
export interface PickerSection {
  id: 'own' | 'precon'
  title: string
  decks: DeckEntry[]
}

/** When the player last took each deck on a delve, by deck id, as ISO times. */
export type LastPlayed = Readonly<Record<string, string>>

const byRecency =
  (played: LastPlayed) =>
  (a: DeckEntry, b: DeckEntry): number => {
    const at = played[a.id] ?? ''
    const bt = played[b.id] ?? ''
    if (at !== bt) return at < bt ? 1 : -1
    return a.name.localeCompare(b.name)
  }

/** The player's decks, then the precons, each narrowed by the filter and led by the decks last played; empty sections are left out. */
export function pickerSections(
  entries: readonly DeckEntry[],
  filter: PickerFilter,
  played: LastPlayed = {},
): PickerSection[] {
  const shown = filterEntries(
    entries.filter((entry) => entry.format === 'commander'),
    { query: filter.query, colours: filter.colours, support: 'all' },
  )
  const sections: PickerSection[] = [
    { id: 'own', title: 'Your decks', decks: shown.filter((e) => e.kind === 'own').sort(byRecency(played)) },
    { id: 'precon', title: 'Precons', decks: shown.filter((e) => e.kind === 'precon').sort(byRecency(played)) },
  ]
  return sections.filter((section) => section.decks.length > 0)
}

/** A deck's fitness for a delve, in words and a tone: legal and fully run, legal with cards played by hand, or not legal. */
export function deckValidity(entry: DeckEntry): { tone: 'ok' | 'warn' | 'bad'; label: string } {
  if (!entry.legal) {
    return { tone: 'bad', label: entry.issues === 1 ? '1 rule issue' : `${entry.issues} rule issues` }
  }
  if (entry.state === 'unchecked') return { tone: 'warn', label: 'Not checked yet' }
  if (entry.byHand > 0) return { tone: 'warn', label: `Legal · ${entry.byHand} by hand` }
  return { tone: 'ok', label: 'Legal' }
}

/** The `localStorage` key of the decks' last delves. */
export const PLAYED_KEY = 'magic.delves.played'

function localStore(): KeyValueStore | null {
  try {
    return window.localStorage
  } catch {
    return null
  }
}

/** Reads when each deck was last taken on a delve; nothing where storage fails or holds something else. */
export function readPlayed(store: KeyValueStore | null = localStore()): LastPlayed {
  try {
    const raw = store?.getItem(PLAYED_KEY)
    if (raw === null || raw === undefined) return {}
    const parsed: unknown = JSON.parse(raw)
    if (typeof parsed !== 'object' || parsed === null) return {}
    return Object.fromEntries(
      Object.entries(parsed).filter((pair): pair is [string, string] => typeof pair[1] === 'string'),
    )
  } catch {
    return {}
  }
}

/** Records that a deck was taken on a delve now; storage that fails is ignored. */
export function notePlayed(id: string, now: Date, store: KeyValueStore | null = localStore()): LastPlayed {
  const next = { ...readPlayed(store), [id]: now.toISOString() }
  try {
    store?.setItem(PLAYED_KEY, JSON.stringify(next))
  } catch {
    // The picker works without the record.
  }
  return next
}

/** The player's Commander decks and the precons as picker entries; a list that fails to load is left out. */
export async function loadPickerDecks(): Promise<DeckEntry[]> {
  const [precons, decks] = await Promise.all([
    preconsApi.list().catch(() => ({ precons: [] })),
    decksApi.list().catch(() => []),
  ])
  return [
    ...decks.filter((d) => d.format === 'commander').map(entryOfDeck),
    ...precons.precons.filter((p) => p.format === 'commander').map((p) => entryOfPrecon(p)),
  ]
}
