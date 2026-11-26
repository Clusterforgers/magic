/** Made-up decks and groups for `/dev/decks`: every marker, an illegal deck, partners, a long name and a deck in no group. */
import type { CardColor, DeckGroupView, DeckListItem } from '@/protocol/generated'
import { cardHit, counts, deckIn, summary } from '../fakeDecks'
import { groupView } from '../groups/fakeGroups'

/** Text only the dev decks page holds; `devBuild.test.ts` checks that no production file contains it. */
export const DECKS_FIXTURE_MARKER = 'decks-fixture'

/** A deck name far wider than a row, for the cut-off rules. */
export const LONG_DECK_NAME = 'Grandmother Ravi Sengupta’s Extremely Long Fifty-Turn Group Hug Pillow Fort'

const commander = (name: string, colors: CardColor[]) =>
  cardHit(name, { type_line: 'Legendary Creature', colors, color_identity: colors, mana_cost: '{3}' })

/** The decks of the list, most recently saved first. */
export const FIXTURE_DECK_LIST: readonly DeckListItem[] = [
  deckIn('verified', {
    id: 'deck-atraxa',
    name: 'Atraxa Superfriends',
    updated_at: '2026-10-18T09:00:00Z',
    commanders: [commander("Atraxa, Praetors' Voice", ['W', 'U', 'B', 'G'])],
  }),
  deckIn('manual_play', {
    id: 'deck-partners',
    name: 'Tymna and Thrasios Midrange',
    updated_at: '2026-10-17T20:00:00Z',
    commanders: [commander('Tymna the Weaver', ['W', 'B']), commander('Thrasios, Triton Hero', ['G', 'U'])],
    summary: summary({
      state: 'manual_play',
      counts: counts({ verified: 94, partial: 4, manual: 2 }),
      to_read: 2,
    }),
  }),
  deckIn('read', {
    id: 'deck-read',
    name: 'Mono-Red Burn',
    format: 'modern',
    updated_at: '2026-10-16T12:00:00Z',
    summary: summary({
      format: 'modern',
      state: 'read',
      counts: counts({ total: 60, verified: 56, read: 4 }),
    }),
  }),
  deckIn('unchecked', {
    id: 'deck-changed',
    name: 'Izzet Spellslinger',
    updated_at: '2026-10-12T12:00:00Z',
    commanders: [commander('Kess, Dissident Mage', ['U', 'B', 'R'])],
  }),
  deckIn('verified', {
    id: 'deck-illegal',
    name: 'Kitchen Table Elves',
    updated_at: '2026-10-02T12:00:00Z',
    commanders: [commander('Lathril, Blade of the Elves', ['B', 'G'])],
    summary: summary({ legal: false, issues: 2, counts: counts({ total: 104, verified: 104 }) }),
  }),
  deckIn('manual_play', {
    id: 'deck-long',
    name: LONG_DECK_NAME,
    updated_at: '2026-09-21T12:00:00Z',
    summary: summary({ state: 'manual_play', counts: counts({ verified: 99, stale: 1 }) }),
  }),
  deckIn('verified', {
    id: 'deck-empty',
    name: 'New deck',
    format: 'normal',
    updated_at: '2026-09-01T12:00:00Z',
    summary: summary({
      format: 'normal',
      legal: false,
      issues: 1,
      counts: counts({ total: 0, verified: 0 }),
      zones: [],
    }),
  }),
]

/** The groups: Fun and Dungeon open, Hardcore folded; Atraxa is in two, "New deck" in none. */
export const FIXTURE_GROUPS: readonly DeckGroupView[] = [
  groupView('group-fun', 'Fun', ['deck-atraxa', 'deck-illegal', 'deck-long']),
  groupView('group-hardcore', 'Hardcore', ['deck-partners', 'deck-atraxa', 'deck-changed'], true),
  groupView('group-dungeon', 'Dungeon', ['deck-read']),
]
