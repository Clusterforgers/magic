// Goal: the words of the decks page are right in every case: a format key becomes its name, the check state of a deck becomes a word and a one-line note that counts what needs manual steps, what blocks the deck or that it changed, and an import summary counts cards per zone; the format list is the one the server checks.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import type { DeckState } from '@/protocol/generated'
import { counts, deckIn, importResult, lineOf, summary } from './fakeDecks'
import {
  cardsText,
  checkNote,
  DECK_FORMATS,
  formatLabel,
  importSummary,
  SOURCE_LABEL,
  STATE_LABEL,
} from './format'

describe('formatLabel', () => {
  it.each([
    ['commander', 'Commander'],
    ['freeform', 'Freeform'],
    ['penny', 'Penny Dreadful'],
    ['oldschool', 'Old School'],
    ['someday', 'someday'],
    ['constructor', 'constructor'],
    ['__proto__', '__proto__'],
    ['', ''],
  ])('names %j as %j', (key, label) => {
    expect(formatLabel(key)).toBe(label)
  })

  it('names every format the picker offers, each differently', () => {
    const labels = DECK_FORMATS.map(formatLabel)
    expect(new Set(labels).size).toBe(DECK_FORMATS.length)
    for (const [index, key] of DECK_FORMATS.entries()) expect(labels[index]).not.toBe(key)
  })

  it('offers the formats the server checks, in its order', () => {
    const source = readFileSync(
      join(import.meta.dirname, '../../../../crates/mtg-cards/src/deck/legality.rs'),
      'utf8',
    )
    const list = /SUPPORTED_FORMATS: \[&str; \d+\] = \[([^\]]*)\]/.exec(source)?.[1] ?? ''
    expect([...list.matchAll(/"([a-z]+)"/g)].map((match) => match[1])).toEqual(DECK_FORMATS)
  })
})

describe('STATE_LABEL', () => {
  it.each([
    ['verified', 'Verified'],
    ['manual_play', 'Manual play'],
    ['blocked', 'Blocked'],
    ['unchecked', 'Unchecked'],
  ] satisfies ReadonlyArray<[DeckState, string]>)('words %s as %s', (state, word) => {
    expect(STATE_LABEL[state]).toBe(word)
  })
})

describe('cardsText', () => {
  it.each([
    [0, '0 cards'],
    [1, '1 card'],
    [2, '2 cards'],
    [100, '100 cards'],
  ])('says %i as %j', (count, text) => {
    expect(cardsText(count)).toBe(text)
  })
})

describe('checkNote', () => {
  it('has nothing to add for a verified deck', () => {
    expect(checkNote(deckIn('verified'))).toBeNull()
  })

  it.each([
    ['manual cards', counts({ verified: 97, manual: 3 }), '97 of 100 verified, 3 manual'],
    [
      'manual and partial',
      counts({ verified: 96, manual: 3, partial: 1 }),
      '96 of 100 verified, 3 manual, 1 partial',
    ],
    [
      'all three kinds',
      counts({ verified: 94, manual: 3, partial: 2, stale: 1 }),
      '94 of 100 verified, 3 manual, 2 partial, 1 stale',
    ],
    ['only stale cards', counts({ total: 60, verified: 59, stale: 1 }), '59 of 60 verified, 1 stale'],
    ['no cards verified', counts({ total: 40, verified: 0, manual: 40 }), '0 of 40 verified, 40 manual'],
  ])('counts %s in a deck that is played by hand', (_what, deckCounts, note) => {
    const deck = deckIn('manual_play', { summary: summary({ state: 'manual_play', counts: deckCounts }) })
    expect(checkNote(deck)).toBe(note)
  })

  it.each([
    ['one issue', 1, 0, '1 legality issue'],
    ['several issues', 3, 0, '3 legality issues'],
    ['one unplayable card', 0, 1, '1 unplayable card'],
    ['unplayable cards', 0, 2, '2 unplayable cards'],
    ['both', 2, 1, '2 legality issues, 1 unplayable card'],
    ['neither', 0, 0, 'Cannot be played as it stands'],
  ])('says what blocks a deck with %s', (_what, issues, unplayable, note) => {
    const deck = deckIn('blocked', {
      summary: summary({ state: 'blocked', issues, counts: counts({ unplayable }) }),
    })
    expect(checkNote(deck)).toBe(note)
  })

  it.each([
    [true, 'Changed since the last check'],
    [false, 'Not checked yet'],
  ])('for an unchecked deck with changed=%s says %j', (changed, note) => {
    expect(checkNote(deckIn('unchecked', { changed }))).toBe(note)
  })
})

describe('importSummary', () => {
  it.each([
    [
      'one zone',
      importResult({ cards: [lineOf('Lightning Bolt', 4), lineOf('Mountain', 20)] }),
      'Read as Arena: 24 cards.',
    ],
    ['an empty list', importResult({ cards: [] }), 'Read as Arena: 0 cards.'],
    [
      'a single card',
      importResult({ cards: [lineOf('Sol Ring', 1)], source: 'text' }),
      'Read as plain text: 1 card.',
    ],
    [
      'main, sideboard and a commander',
      importResult({
        source: 'moxfield',
        cards: [
          lineOf('Forest', 60),
          lineOf('Tormod', 1, { zone: 'commander' }),
          lineOf('Duress', 15, { zone: 'sideboard' }),
        ],
      }),
      'Read as Moxfield: 76 cards (60 in the main deck, 15 in the sideboard, 1 commander).',
    ],
    [
      'two commanders and a companion',
      importResult({
        source: 'archidekt',
        cards: [
          lineOf('Thrasios', 1, { zone: 'commander' }),
          lineOf('Tymna', 1, { zone: 'commander' }),
          lineOf('Lurrus', 1, { zone: 'companion' }),
          lineOf('Island', 98),
        ],
      }),
      'Read as Archidekt: 101 cards (98 in the main deck, 2 commanders, 1 companion).',
    ],
    [
      'lines of one card add up',
      importResult({ cards: [lineOf('Island', 10), lineOf('Island', 10)] }),
      'Read as Arena: 20 cards.',
    ],
  ])('summarises %s', (_what, result, text) => {
    expect(importSummary(result)).toBe(text)
  })

  it('has a name for every kind of list the server reads', () => {
    expect(Object.keys(SOURCE_LABEL).sort()).toEqual([
      'archidekt',
      'arena',
      'moxfield',
      'mtgo',
      'mtgo_dek',
      'text',
    ])
  })
})
