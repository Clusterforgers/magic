// Goal: the chooser's rows read a stored deck, a precon and a built-in deck into one shape with the right facts, support chip and colours, and the filters (search words, colours, support) and the mark of the seat's deck pick the right rows.
import { describe, expect, it } from 'vitest'
import { cardHit, deckIn } from '@/features/decks/fakeDecks'
import { counts } from '@/features/decks/fakeDecks'
import type { PreconListItem } from '@/protocol/generated'
import {
  entryFacts,
  entryOfBuiltIn,
  entryOfDeck,
  entryOfPrecon,
  entrySupport,
  filterEntries,
  fitsFormat,
  markedId,
  NO_FILTER,
} from './deckEntries'
import type { DeckEntry } from './deckEntries'

const precon = (over: Partial<PreconListItem> = {}): PreconListItem => ({
  code: 'c18-x',
  name: 'Draconic Domination',
  set: 'c18',
  deck_type: 'Commander Deck',
  release_date: null,
  commanders: ['The Ur-Dragon'],
  card_count: 100,
  format: 'commander',
  state: 'verified',
  verified: true,
  playable: true,
  summary: deckIn('verified').summary,
  ...over,
})

describe('entries', () => {
  it('reads a stored deck with the commanders’ pictures and identity from its list item', () => {
    const entry = entryOfDeck(
      deckIn('verified', {
        id: 'd1',
        name: 'Atraxa',
        commanders: [
          cardHit('Atraxa', { color_identity: ['G', 'W', 'U'] }),
          cardHit('Tymna', { color_identity: ['B'] }),
        ],
      }),
    )
    expect(entry).toMatchObject({ id: 'd1', kind: 'own', cards: 100, identity: ['W', 'U', 'B', 'G'] })
    expect(entry.commanders.map((card) => card.name)).toEqual(['Atraxa', 'Tymna'])
    expect(entryFacts(entry)).toBe('Commander · 100 cards')
  })

  it('knows no identity for a deck without a commander', () => {
    expect(entryOfDeck(deckIn('verified', { format: 'modern' })).identity).toBeNull()
  })

  it('reads a precon by its code, with set first in the facts and no pictures or colours', () => {
    const entry = entryOfPrecon(precon())
    expect(entry).toMatchObject({ id: 'precon:c18-x', kind: 'precon', identity: null, commanders: [] })
    expect(entry.commanderNames).toEqual(['The Ur-Dragon'])
    expect(entryFacts(entry)).toBe('C18 · Commander · 100 cards')
  })

  it('reads a built-in deck as a Normal deck the game runs whole', () => {
    const entry = entryOfBuiltIn({
      id: 'mono-red',
      name: 'Mono Red Giants',
      color: 'Red',
      size: 60,
      cards: [{ name: 'Forest', count: 24 }],
    })
    expect(entry).toMatchObject({ kind: 'builtin', format: 'normal', counts: null })
    expect(entryFacts(entry)).toBe('Normal · 60 cards')
    expect(entrySupport(entry)).toEqual({ state: 'verified', label: 'Whole deck runs' })
  })
})

describe('the support chip', () => {
  it.each([
    ['verified', counts({ total: 100, verified: 100 }), 'verified', 'Whole deck runs'],
    [
      'manual_play',
      counts({ total: 100, verified: 91, manual: 6, partial: 2, stale: 1 }),
      'manual_play',
      '9 played by hand',
    ],
    ['read', counts({ total: 100, verified: 96, read: 4 }), 'read', 'Runs on reads'],
    [
      'manual_play',
      counts({ total: 100, verified: 90, read: 4, manual: 6 }),
      'manual_play',
      '6 played by hand',
    ],
    ['blocked', counts({ total: 100, verified: 98, unplayable: 2 }), 'blocked', 'Cannot be taken'],
    ['unchecked', counts({}), 'unchecked', 'Not checked yet'],
  ] as const)('says %s in words', (state, tally, shown, label) => {
    const entry = entryOfDeck(deckIn(state, { summary: { ...deckIn(state).summary, counts: tally } }))
    expect(entrySupport(entry)).toEqual({ state: shown, label })
  })

  it.each([0, 3])(
    'carries the %i cards to read of a deck and a precon, and a built-in deck has none',
    (toRead) => {
      const summary = { ...deckIn('manual_play').summary, to_read: toRead }
      expect(entryOfDeck(deckIn('manual_play', { summary })).toRead).toBe(toRead)
      expect(entryOfPrecon(precon({ summary })).toRead).toBe(toRead)
      expect(entryOfBuiltIn({ id: 'b', name: 'Mono Red', color: 'Red', size: 60, cards: [] }).toRead).toBe(0)
    },
  )

  it('treats a precon the game cannot take as blocked whatever its state says', () => {
    expect(entrySupport(entryOfPrecon(precon({ playable: false }))).label).toBe('Cannot be taken')
  })
})

describe('filters', () => {
  const entries: DeckEntry[] = [
    entryOfDeck(
      deckIn('verified', {
        id: 'a',
        name: 'Atraxa Value',
        commanders: [cardHit('Atraxa', { color_identity: ['W', 'U', 'B', 'G'] })],
      }),
    ),
    entryOfDeck(
      deckIn('manual_play', {
        id: 'b',
        name: 'Krenko Goblins',
        commanders: [cardHit('Krenko, Mob Boss', { color_identity: ['R'] })],
        summary: {
          ...deckIn('manual_play').summary,
          counts: counts({ total: 100, verified: 97, manual: 3 }),
        },
      }),
    ),
    entryOfPrecon(precon()),
  ]
  const ids = (found: DeckEntry[]): string[] => found.map((entry) => entry.id)

  it.each([
    ['', ['a', 'b', 'precon:c18-x']],
    ['atraxa', ['a']],
    ['KRENKO mob', ['b']],
    ['c18', ['precon:c18-x']],
    ['ur-dragon', ['precon:c18-x']],
    ['commander deck', ['precon:c18-x']],
    ['goblins value', []],
  ])('searches %j over name, set, commander and kind', (query, expected) => {
    expect(ids(filterEntries(entries, { ...NO_FILTER, query }))).toEqual(expected)
  })

  it.each([
    [['R'], ['b']],
    [['W', 'G'], ['a']],
    [['R', 'U'], []],
  ])(
    'keeps the decks whose identity holds every colour of %j, and drops those with none known',
    (colours, expected) => {
      expect(ids(filterEntries(entries, { ...NO_FILTER, colours }))).toEqual(expected)
    },
  )

  it.each([
    ['verified', ['a', 'precon:c18-x']],
    ['hand', ['b']],
    ['all', ['a', 'b', 'precon:c18-x']],
  ] as const)('filters by support %s', (support, expected) => {
    expect(ids(filterEntries(entries, { ...NO_FILTER, support }))).toEqual(expected)
  })

  it('lists a deck that runs on reads with the decks that run whole, not with those played by hand', () => {
    const reads = entryOfDeck(
      deckIn('read', {
        id: 'r',
        summary: { ...deckIn('read').summary, counts: counts({ total: 100, verified: 96, read: 4 }) },
      }),
    )
    expect(ids(filterEntries([reads], { ...NO_FILTER, support: 'verified' }))).toEqual(['r'])
    expect(ids(filterEntries([reads], { ...NO_FILTER, support: 'hand' }))).toEqual([])
  })

  it('takes the format of the game: a Commander game takes decks with a commander', () => {
    expect(fitsFormat({ format: 'commander' }, true)).toBe(true)
    expect(fitsFormat({ format: 'freeform' }, true)).toBe(false)
    expect(fitsFormat({ format: 'modern' }, false)).toBe(true)
    expect(fitsFormat({ format: 'commander' }, false)).toBe(false)
  })
})

describe('the mark of the seat’s deck', () => {
  const rows = [
    { id: 'a', name: 'Burn', format: 'modern' },
    { id: 'b', name: 'Burn', format: 'pauper' },
    { id: 'c', name: 'Mill', format: 'pauper' },
  ]
  it.each([
    [{ name: 'Mill', format: 'pauper' }, null, 'c'],
    [{ name: 'Burn', format: 'pauper' }, null, 'b'],
    [{ name: 'Burn', format: 'legacy' }, null, null],
    [{ name: 'Burn', format: 'modern' }, 'c', 'c'],
    [{ name: 'Burn', format: 'modern' }, 'gone', 'a'],
    [null, null, null],
    [undefined, null, null],
  ])('marks %j (answered %j) as %j', (current, answered, expected) => {
    expect(markedId(rows, current, answered)).toBe(expected)
  })

  it('marks nothing when two decks share name and format', () => {
    expect(
      markedId(
        [...rows, { id: 'd', name: 'Mill', format: 'pauper' }],
        { name: 'Mill', format: 'pauper' },
        null,
      ),
    ).toBeNull()
  })
})
