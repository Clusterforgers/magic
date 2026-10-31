// Goal: the not-validated list groups the cards of checked decks by deck and merges them by card (the cards held by most decks first, then by name), narrows them to a state, quotes each clause from the text of its face by spans counted in Unicode scalar values, and writes the copy text in exactly one format.
import { describe, expect, it } from 'vitest'
import { reportCard } from '../fakeDecks'
import {
  clauseLines,
  copyText,
  filterCards,
  groupByDeck,
  mergeDecks,
  quoteOf,
  stateCounts,
  waitsForRead,
} from './model'
import type { CheckedDeck, FlaggedCard } from './model'

const clause = (text: string, face = 0, start = 0, end = 4) => ({ face, span: { start, end }, text })

const deck = (id: string, name: string, cards: CheckedDeck['cards']): CheckedDeck => ({ id, name, cards })

const A = deck('a', 'Aristocrats', [
  reportCard('Zed', { support: 'manual' }),
  reportCard('Alpha', { support: 'partial', reasons: [clause('half done')] }),
  reportCard('Forest', { count: 30 }),
])
const B = deck('b', 'Bees', [
  reportCard('Zed', { support: 'manual' }),
  reportCard('Beta', { support: 'stale' }),
])
const C = deck('c', 'Clean', [reportCard('Forest', { count: 40 })])
const D = deck('d', 'Dragons', [
  reportCard('Zed', { support: 'manual' }),
  reportCard('Beta', { support: 'stale' }),
  reportCard('Gone', { support: 'unplayable' }),
])

const names = (cards: readonly { name: string }[]): string[] => cards.map((card) => card.name)

describe('groupByDeck', () => {
  it('lists each deck with its cards that are not Verified, by name, and leaves out a deck that runs whole', () => {
    const groups = groupByDeck([A, C, B])
    expect(groups.map((group) => [group.deck.name, names(group.cards)])).toEqual([
      ['Aristocrats', ['Alpha', 'Zed']],
      ['Bees', ['Beta', 'Zed']],
    ])
  })

  it('leaves out a card that runs on a read: it is not played by hand', () => {
    const read = deck('r', 'Readers', [
      reportCard('Zed', { support: 'manual' }),
      reportCard('Reader', { support: 'read' }),
      reportCard('Forest', { count: 30 }),
    ])
    expect(groupByDeck([read]).map((group) => names(group.cards))).toEqual([['Zed']])
  })

  it('names a card once even when two zones hold it', () => {
    const both = deck('x', 'Both', [
      reportCard('Zed', { support: 'manual' }),
      reportCard('Zed', { support: 'manual', zone: 'sideboard' }),
    ])
    expect(names(groupByDeck([both])[0]?.cards ?? [])).toEqual(['Zed'])
  })

  it('is empty for no decks and for decks that run whole', () => {
    expect(groupByDeck([])).toEqual([])
    expect(groupByDeck([C])).toEqual([])
  })
})

describe('mergeDecks', () => {
  it('puts the cards held by most decks first, then by name, each with its decks in the order given', () => {
    const merged = mergeDecks([A, B, C, D])
    expect(merged.map((card) => [card.name, card.decks.map((entry) => entry.name)])).toEqual([
      ['Zed', ['Aristocrats', 'Bees', 'Dragons']],
      ['Beta', ['Bees', 'Dragons']],
      ['Alpha', ['Aristocrats']],
      ['Gone', ['Dragons']],
    ])
  })

  it('breaks a tie by name without regard to case or accents', () => {
    const tied = deck('t', 'Tied', [
      reportCard('zebra', { support: 'manual' }),
      reportCard('Émile', { support: 'manual' }),
      reportCard('Apple', { support: 'manual' }),
    ])
    expect(names(mergeDecks([tied]))).toEqual(['Apple', 'Émile', 'zebra'])
  })

  it('keeps the reasons the report gave', () => {
    expect(mergeDecks([A])[0]?.reasons).toEqual([clause('half done')])
  })
})

describe('filterCards and stateCounts', () => {
  const merged = mergeDecks([A, B, D])

  it.each([
    ['all', ['Zed', 'Beta', 'Alpha', 'Gone']],
    ['partial', ['Alpha']],
    ['manual', ['Zed']],
    ['stale', ['Beta']],
    ['unplayable', ['Gone']],
  ] as const)('the %s filter keeps %j', (filter, kept) => {
    expect(names(filterCards(merged, filter))).toEqual(kept)
  })

  it('counts the states that have cards, in the order of the filters', () => {
    expect(stateCounts(merged)).toEqual([
      ['partial', 1],
      ['manual', 1],
      ['stale', 1],
      ['unplayable', 1],
    ])
    expect(stateCounts(mergeDecks([A]))).toEqual([
      ['partial', 1],
      ['manual', 1],
    ])
    expect(stateCounts([])).toEqual([])
  })
})

describe('the drafts that wait for a read', () => {
  const waiting = deck('w', 'Waiters', [
    reportCard('Zed', { support: 'manual', draft: 'pending' }),
    reportCard('Alpha', { support: 'partial', draft: 'wrong', reasons: [clause('half done')] }),
    reportCard('Beta', { support: 'stale', draft: 'pending' }),
    reportCard('Plain', { support: 'manual' }),
    reportCard('Reader', { support: 'read', draft: 'approved' }),
  ])
  const merged = mergeDecks([waiting, B])

  it('carries the state of a draft to the card, once, and lists no card that runs on a read', () => {
    expect(merged.map((card) => [card.name, card.draft])).toEqual([
      ['Beta', 'pending'],
      ['Zed', 'pending'],
      ['Alpha', 'wrong'],
      ['Plain', undefined],
    ])
  })

  it.each([
    ['all', ['Beta', 'Zed', 'Alpha', 'Plain']],
    ['to_read', ['Beta', 'Zed']],
    ['manual', ['Zed', 'Plain']],
  ] as const)('the %s filter keeps %j', (filter, kept) => {
    expect(names(filterCards(merged, filter))).toEqual(kept)
  })

  it('counts a draft that was found wrong as played by hand but not as waiting', () => {
    expect(merged.filter(waitsForRead)).toHaveLength(2)
    expect(waitsForRead({ draft: 'wrong' })).toBe(false)
    expect(waitsForRead({ draft: 'approved' })).toBe(false)
    expect(waitsForRead({})).toBe(false)
  })
})

describe('quoteOf', () => {
  const faces = ['Draw a card. 🙂 Then discard a card.\nScry 2.', 'Flying']

  it.each([
    ['a span inside the first face', 0, 0, 12, 'Draw a card.'],
    ['a character outside the Basic Multilingual Plane as one position', 0, 13, 14, '🙂'],
    ['a span after that character, trimmed', 0, 15, 20, 'Then'],
    ['a span over two lines, joined with a slash', 0, 15, 43, 'Then discard a card. / Scry 2.'],
    ['a span of the second face', 1, 0, 6, 'Flying'],
    ['a span cut back to the text', 1, 3, 99, 'ing'],
  ])('reads %s', (_, face, start, end, quote) => {
    expect(quoteOf(clause('why', face, start, end), faces)).toBe(quote)
  })

  it.each([
    ['an empty span', 0, 5, 5, ['Some text']],
    ['a reversed span', 0, 5, 2, ['Some text']],
    ['a span past the end', 0, 40, 50, ['Some text']],
    ['a face the card does not have', 2, 0, 4, ['Some text']],
    ['text that is not known', 0, 0, 4, undefined],
  ])('is null for %s', (_, face, start, end, texts) => {
    expect(quoteOf(clause('why', face, start, end), texts)).toBeNull()
  })
})

const flagged = (over: Partial<FlaggedCard> = {}): FlaggedCard => ({
  oracleId: 'oracle-x',
  name: 'Card X',
  support: 'partial',
  reasons: [clause('the draw is not implemented', 0, 0, 12)],
  decks: [{ id: 'a', name: 'Aristocrats' }],
  ...over,
})

describe('clauseLines', () => {
  it('quotes each clause beside its reason, without the final stop', () => {
    expect(clauseLines(flagged(), ['Draw a card. Then more.'])).toEqual([
      { key: '0:0-12:0', quote: 'Draw a card.', text: 'The draw is not implemented' },
    ])
  })

  it('shows the reason alone while the text is not known', () => {
    expect(clauseLines(flagged(), undefined).map((line) => line.quote)).toEqual([null])
  })

  it('names the face when a clause is not on the front', () => {
    const back = flagged({ reasons: [clause('it flips', 1, 0, 4)] })
    expect(clauseLines(back, ['Front', 'Flip it']).map((line) => [line.quote, line.text])).toEqual([
      ['Flip', 'Back face: it flips'],
    ])
  })

  it.each([
    ['manual', 'Played by hand'],
    ['partial', 'Partly played by hand'],
    ['stale', 'Its text changed since the rules were written, so it is played by hand'],
    ['unplayable', 'The game cannot play it'],
  ] as const)('leaves the whole text of a %s card with no clause, and says "%s"', (support, text) => {
    const whole = flagged({ support, reasons: [] })
    expect(clauseLines(whole, ['Line one\nLine two', 'Back'])).toEqual([
      { key: 'whole', quote: 'Line one / Line two / Back', text },
    ])
    expect(clauseLines(whole, undefined)[0]?.quote).toBeNull()
  })
})

describe('copyText', () => {
  const texts = new Map([
    ['oracle-x', ['Draw a card. Then more.']],
    ['oracle-y', ['Whenever you cast a spell,\nyou win.']],
  ])
  const x = flagged({
    decks: [
      { id: 'a', name: 'Aristocrats' },
      { id: 'b', name: 'Bees' },
    ],
  })
  const y = flagged({ oracleId: 'oracle-y', name: 'Card Y', support: 'manual', reasons: [], decks: [] })

  it('writes one block a card, a line a clause, blocks apart by an empty line', () => {
    expect(copyText([x, y], texts)).toBe(
      [
        'Card X (Partial) - decks: Aristocrats, Bees',
        '  - "Draw a card.": The draw is not implemented',
        '',
        'Card Y (Manual)',
        '  - "Whenever you cast a spell, / you win.": Played by hand',
      ].join('\n'),
    )
  })

  it('writes the reason alone for a clause whose text is not known, and a line for every clause', () => {
    const two = flagged({
      reasons: [clause('first thing', 0, 0, 4), clause('second thing', 0, 5, 9)],
    })
    expect(copyText([two], new Map())).toBe(
      ['Card X (Partial) - decks: Aristocrats', '  - First thing', '  - Second thing'].join('\n'),
    )
  })

  it('is empty for no cards', () => {
    expect(copyText([], texts)).toBe('')
  })
})
