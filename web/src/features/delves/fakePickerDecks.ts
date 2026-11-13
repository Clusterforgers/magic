/** Decks for the Delves picker's tests and `/dev/delve`: the player's own Commander decks in two groups, and precons. */
import { cardHit, deckItem, summary } from '@/features/decks/fakeDecks'
import { groupView } from '@/features/decks/groups/fakeGroups'
import { entryOfDeck } from '@/features/game/deckEntries'
import type { DeckEntry } from '@/features/game/deckEntries'
import type { CardColor } from '@/protocol/generated'
import type { PickerDecks } from './deckPicker'

function own(id: string, name: string, commander: string, identity: CardColor[], legal = true): DeckEntry {
  return entryOfDeck(
    deckItem({
      id,
      name,
      commanders: [cardHit(commander, { color_identity: identity })],
      pictures: [{ oracle_id: `oracle-${id}`, name: commander }],
      summary: summary({ legal, issues: legal ? 0 : 2 }),
    }),
  )
}

function precon(code: string, name: string, commander: string, identity: string[]): DeckEntry {
  return {
    ...own(`precon:${code}`, name, commander, identity as CardColor[]),
    kind: 'precon',
    set: code,
  }
}

/** The entries: three own decks (one breaking a rule) and three precons. */
export const PICKER_ENTRIES: DeckEntry[] = [
  own('d-atraxa', 'Atraxa Counters', "Atraxa, Praetors' Voice", ['W', 'U', 'B', 'G']),
  own('d-krenko', 'Goblin Swarm', 'Krenko, Mob Boss', ['R']),
  own('d-meren', 'Meren Graveyard', 'Meren of Clan Nel Toth', ['B', 'G'], false),
  precon('cmm', 'Draconic Dissent', 'Lathliss, Dragon Queen', ['U', 'B', 'R']),
  precon('x', 'Elven Council', 'Lathril, Blade of the Elves', ['G']),
  precon('ltc', 'Riders of Rohan', 'Éomer, King of Rohan', ['R', 'W']),
]

/** The player's groups: Fun holds the goblins, Hardcore the graveyard deck; Atraxa sits in none. */
export const PICKER_GROUPS = [
  groupView('fun', 'Fun', ['d-krenko']),
  groupView('hard', 'Hardcore', ['d-meren']),
]

/** Both, as the page loads them. */
export const pickerDecks = async (): Promise<PickerDecks> => ({
  entries: PICKER_ENTRIES,
  groups: PICKER_GROUPS,
})
