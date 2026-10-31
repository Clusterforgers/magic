// Goal: the words of the decks page and the builder are right in every case: a format key becomes its name, a check that found a card the game does not run whole is told from one that did not, the check state of a deck and the support state of a card become words, the counts of a check become one line ("96 of 100 verified, 4 played by hand"), a deck's note counts what is played by hand or what blocks it and leaves a changed deck to its marker, and an import summary counts cards per zone; the format list is the one the server checks.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import type { Counts, DeckState, Support } from '@/protocol/generated'
import { counts, deckIn, importResult, lineOf, summary } from './fakeDecks'
import {
  BLOCKED_MEANING,
  cardsText,
  checkNote,
  countsText,
  DECK_FORMATS,
  formatLabel,
  hasUnvalidated,
  importSummary,
  SOURCE_LABEL,
  STATE_LABEL,
  SUPPORT_LABEL,
  unvalidatedLink,
} from './format'

describe('unvalidatedLink', () => {
  it.each<[string, Partial<Counts>, string]>([
    ['nothing to show', {}, ''],
    ['a partial card', { partial: 1 }, 'Show the 1 card played by hand'],
    [
      'manual, partial and stale copies together',
      { manual: 2, partial: 1, stale: 1 },
      'Show the 4 cards played by hand',
    ],
    ['one unplayable card', { unplayable: 1 }, 'Show the 1 unplayable card'],
    ['unplayable cards', { unplayable: 2 }, 'Show the 2 unplayable cards'],
    ['both', { manual: 7, unplayable: 1 }, 'Show the 7 cards played by hand and 1 unplayable card'],
  ])('names %s as "%s"', (_what, held, words) => {
    expect(unvalidatedLink(counts(held))).toBe(words)
  })
})

describe('hasUnvalidated', () => {
  it.each([
    [{}, false],
    [{ partial: 1 }, true],
    [{ manual: 3 }, true],
    [{ stale: 1 }, true],
    [{ unplayable: 1 }, true],
  ])('is decided by the counts %j: %s', (over, expected) => {
    expect(hasUnvalidated(counts(over))).toBe(expected)
  })
})

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
    ['manual_play', 'Played by hand'],
    ['blocked', 'Blocked'],
    ['unchecked', 'Unchecked'],
  ] satisfies ReadonlyArray<[DeckState, string]>)('words %s as %s', (state, word) => {
    expect(STATE_LABEL[state]).toBe(word)
  })
})

describe('SUPPORT_LABEL', () => {
  it.each([
    ['verified', 'Verified'],
    ['partial', 'Partial'],
    ['manual', 'Manual'],
    ['stale', 'Stale'],
    ['unplayable', 'Unplayable'],
  ] satisfies ReadonlyArray<[Support, string]>)('words %s as %s', (support, word) => {
    expect(SUPPORT_LABEL[support]).toBe(word)
  })
})

describe('countsText', () => {
  it.each([
    ['a clean deck', counts(), '100 of 100 verified'],
    ['manual cards', counts({ verified: 97, manual: 3 }), '97 of 100 verified, 3 played by hand'],
    [
      'the example of the design',
      counts({ verified: 96, manual: 3, partial: 1 }),
      '96 of 100 verified, 4 played by hand',
    ],
    ['stale cards', counts({ verified: 98, stale: 2 }), '98 of 100 verified, 2 played by hand'],
    [
      'every state (the three kinds played by hand are added up)',
      counts({ total: 10, verified: 4, partial: 3, manual: 1, stale: 1, unplayable: 1 }),
      '4 of 10 verified, 5 played by hand, 1 unplayable',
    ],
    ['an unplayable card', counts({ verified: 99, unplayable: 1 }), '99 of 100 verified, 1 unplayable'],
    ['no cards', counts({ total: 0, verified: 0 }), '0 of 0 verified'],
  ] satisfies ReadonlyArray<[string, Counts, string]>)('words %s', (_what, given, text) => {
    expect(countsText(given)).toBe(text)
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
    ['manual cards', counts({ verified: 97, manual: 3 }), '97 of 100 verified, 3 played by hand'],
    [
      'manual and partial',
      counts({ verified: 96, manual: 3, partial: 1 }),
      '96 of 100 verified, 4 played by hand',
    ],
    [
      'all three kinds',
      counts({ verified: 94, manual: 3, partial: 2, stale: 1 }),
      '94 of 100 verified, 6 played by hand',
    ],
    [
      'only stale cards',
      counts({ total: 60, verified: 59, stale: 1 }),
      '59 of 60 verified, 1 played by hand',
    ],
    [
      'no cards verified',
      counts({ total: 40, verified: 0, manual: 40 }),
      '0 of 40 verified, 40 played by hand',
    ],
  ])('counts %s in a deck that is played by hand', (_what, deckCounts, note) => {
    const deck = deckIn('manual_play', { summary: summary({ state: 'manual_play', counts: deckCounts }) })
    expect(checkNote(deck)).toBe(note)
  })

  it.each([
    ['one issue', 1, 0, '1 legality issue. '],
    ['several issues', 3, 0, '3 legality issues. '],
    ['one unplayable card', 0, 1, '1 unplayable card. '],
    ['unplayable cards', 0, 2, '2 unplayable cards. '],
    ['both', 2, 1, '2 legality issues, 1 unplayable card. '],
    ['neither', 0, 0, ''],
  ])('says what blocks a deck with %s, and what blocked means', (_what, issues, unplayable, what) => {
    const deck = deckIn('blocked', {
      summary: summary({ state: 'blocked', issues, counts: counts({ unplayable }) }),
    })
    expect(checkNote(deck)).toBe(`${what}${BLOCKED_MEANING}.`)
  })

  it.each([
    [true, null],
    [false, 'Not checked yet'],
  ])(
    'for an unchecked deck with changed=%s says %j (a changed deck says it in its marker)',
    (changed, note) => {
      expect(checkNote(deckIn('unchecked', { changed }))).toBe(note)
    },
  )
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
