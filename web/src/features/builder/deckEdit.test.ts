// Goal: editing a draft keeps one line per card and zone, a line never drops below one copy (the last copy removes it) or pushes the deck over its card limit, a move between zones merges into the line already there, a draft built from a stored deck merges duplicate lines, and the dirty check ignores the order of lines but sees every other difference.
import { describe, expect, it } from 'vitest'
import { cardHit, deckView, lineOf } from '@/features/decks/fakeDecks'
import {
  addCard,
  bodyOf,
  copiesOf,
  decrement,
  draftOf,
  hasRoom,
  increment,
  lineIn,
  linesIn,
  MAX_DECK_CARDS,
  moveLine,
  removeLine,
  sameDeck,
  setCount,
  totalOf,
} from './deckEdit'
import type { Draft, Line } from './deckEdit'

const bolt = cardHit('Lightning Bolt')
const forest = cardHit('Forest', { type_line: 'Basic Land — Forest', mana_cost: '', mana_value: 0 })
const ring = cardHit('Sol Ring', { type_line: 'Artifact', mana_cost: '{1}', colors: [], color_identity: [] })

const empty: Draft = { name: 'Burn', format: 'modern', notes: '', lines: [] }
const line = (
  card = bolt,
  zone: Line['zone'] = 'main',
  count = 1,
  printingId: string | null = null,
): Line => ({
  zone,
  card,
  count,
  printingId,
})
const deck = (...lines: Line[]): Draft => ({ ...empty, lines })
const counts = (draft: Draft): string[] =>
  draft.lines.map((item) => `${item.zone}:${item.card.name}:${item.count}`)

describe('addCard', () => {
  it('starts a line at the end of the deck, in the default art', () => {
    const next = addCard(deck(line(forest, 'main', 20)), bolt, 'main')
    expect(counts(next)).toEqual(['main:Forest:20', 'main:Lightning Bolt:1'])
    expect(next.lines[1]?.printingId).toBeNull()
  })

  it('adds onto the line of the same card in the same zone and leaves it where it was', () => {
    const next = addCard(deck(line(bolt, 'main', 2), line(forest, 'main', 20)), bolt, 'main')
    expect(counts(next)).toEqual(['main:Lightning Bolt:3', 'main:Forest:20'])
  })

  it('keeps a card in two zones as two lines', () => {
    const next = addCard(deck(line(bolt, 'main', 4)), bolt, 'sideboard', 2)
    expect(counts(next)).toEqual(['main:Lightning Bolt:4', 'sideboard:Lightning Bolt:2'])
    expect(copiesOf(next, bolt.oracle_id)).toBe(6)
    expect(copiesOf(next, bolt.oracle_id, 'sideboard')).toBe(2)
  })

  it.each([[0], [-2], [Number.NaN], [0.4]])(
    'ignores %s copies and returns the draft it was given',
    (copies) => {
      const start = deck(line(bolt))
      expect(addCard(start, ring, 'main', copies)).toBe(start)
    },
  )

  it('floors a fractional count', () => {
    expect(counts(addCard(empty, bolt, 'main', 2.9))).toEqual(['main:Lightning Bolt:2'])
  })

  it('cuts an add back to the room under the card limit and refuses once the deck is full', () => {
    const nearly = deck(line(forest, 'main', MAX_DECK_CARDS - 1))
    const next = addCard(nearly, bolt, 'main', 3)
    expect(totalOf(next)).toBe(MAX_DECK_CARDS)
    expect(copiesOf(next, bolt.oracle_id)).toBe(1)
    expect(hasRoom(next)).toBe(false)
    expect(addCard(next, ring, 'main')).toBe(next)
    expect(addCard(next, bolt, 'main')).toBe(next)
  })

  it('changes nothing in the draft it was given', () => {
    const start = deck(line(bolt, 'main', 2))
    addCard(start, bolt, 'main')
    expect(counts(start)).toEqual(['main:Lightning Bolt:2'])
  })
})

describe('setCount, increment and decrement', () => {
  it.each([
    [5, 5],
    [1, 1],
    [2.7, 2],
  ])('sets %s copies as %s', (asked, got) => {
    expect(
      lineIn(setCount(deck(line(bolt, 'main', 3)), 'main', bolt.oracle_id, asked), 'main', bolt.oracle_id)
        ?.count,
    ).toBe(got)
  })

  it.each([[0], [-1]])('removes the line when the count is set to %s', (count) => {
    const next = setCount(deck(line(bolt, 'main', 3), line(ring)), 'main', bolt.oracle_id, count)
    expect(counts(next)).toEqual(['main:Sol Ring:1'])
  })

  it('cuts a count back to the room the rest of the deck leaves', () => {
    const start = deck(line(forest, 'main', 200), line(bolt, 'main', 10))
    expect(lineIn(setCount(start, 'main', bolt.oracle_id, 100), 'main', bolt.oracle_id)?.count).toBe(50)
  })

  it('leaves a missing line and an unchanged count alone, as the same draft', () => {
    const start = deck(line(bolt, 'main', 3))
    expect(setCount(start, 'sideboard', bolt.oracle_id, 2)).toBe(start)
    expect(setCount(start, 'main', ring.oracle_id, 2)).toBe(start)
    expect(setCount(start, 'main', bolt.oracle_id, 3)).toBe(start)
  })

  it('increments by one and decrements by one', () => {
    const start = deck(line(bolt, 'main', 3))
    expect(lineIn(increment(start, 'main', bolt.oracle_id), 'main', bolt.oracle_id)?.count).toBe(4)
    expect(lineIn(decrement(start, 'main', bolt.oracle_id), 'main', bolt.oracle_id)?.count).toBe(2)
  })

  it('removes a line when its last copy is decremented', () => {
    expect(decrement(deck(line(bolt, 'main', 1), line(ring)), 'main', bolt.oracle_id).lines).toHaveLength(1)
    expect(decrement(deck(line(bolt, 'main', 1)), 'main', bolt.oracle_id).lines).toEqual([])
  })

  it('does not increment past the card limit, and ignores a card that is not there', () => {
    const full = deck(line(bolt, 'main', MAX_DECK_CARDS))
    expect(increment(full, 'main', bolt.oracle_id)).toBe(full)
    expect(increment(full, 'main', ring.oracle_id)).toBe(full)
    expect(decrement(full, 'sideboard', bolt.oracle_id)).toBe(full)
  })
})

describe('removeLine', () => {
  it('takes the card out of one zone whatever its count and leaves its other lines', () => {
    const start = deck(line(bolt, 'main', 4), line(bolt, 'sideboard', 2), line(ring))
    expect(counts(removeLine(start, 'main', bolt.oracle_id))).toEqual([
      'sideboard:Lightning Bolt:2',
      'main:Sol Ring:1',
    ])
  })

  it('returns the same draft for a line that is not there', () => {
    const start = deck(line(bolt))
    expect(removeLine(start, 'sideboard', bolt.oracle_id)).toBe(start)
  })
})

describe('moveLine', () => {
  it('moves every copy to another zone by default', () => {
    const next = moveLine(deck(line(bolt, 'main', 3), line(ring)), 'main', bolt.oracle_id, 'sideboard')
    expect(counts(next)).toEqual(['main:Sol Ring:1', 'sideboard:Lightning Bolt:3'])
  })

  it('moves some copies and leaves the rest where they are', () => {
    const next = moveLine(deck(line(bolt, 'main', 4), line(ring)), 'main', bolt.oracle_id, 'sideboard', 1)
    expect(counts(next)).toEqual(['main:Lightning Bolt:3', 'main:Sol Ring:1', 'sideboard:Lightning Bolt:1'])
  })

  it('adds to the line the card already has in the target zone', () => {
    const start = deck(line(bolt, 'main', 2), line(bolt, 'sideboard', 1, 'target-art'))
    const next = moveLine(start, 'main', bolt.oracle_id, 'sideboard')
    expect(counts(next)).toEqual(['sideboard:Lightning Bolt:3'])
    expect(next.lines[0]?.printingId).toBe('target-art')
  })

  it('moves every copy when asked for more than the line holds', () => {
    expect(counts(moveLine(deck(line(bolt, 'main', 2)), 'main', bolt.oracle_id, 'companion', 9))).toEqual([
      'companion:Lightning Bolt:2',
    ])
  })

  it('takes the printing along to a new line', () => {
    const next = moveLine(deck(line(bolt, 'main', 2, 'art-a')), 'main', bolt.oracle_id, 'sideboard', 1)
    expect(next.lines.map((item) => item.printingId)).toEqual(['art-a', 'art-a'])
  })

  it('does not change the number of copies in the deck', () => {
    const start = deck(line(bolt, 'main', 4), line(bolt, 'sideboard', 2))
    expect(totalOf(moveLine(start, 'main', bolt.oracle_id, 'sideboard', 3))).toBe(6)
  })

  it.each([
    ['the same zone', 'main', bolt.oracle_id, 'main', undefined],
    ['a line that is not there', 'sideboard', bolt.oracle_id, 'main', undefined],
    ['a card that is not there', 'main', ring.oracle_id, 'sideboard', undefined],
    ['no copies', 'main', bolt.oracle_id, 'sideboard', 0],
    ['a negative count', 'main', bolt.oracle_id, 'sideboard', -3],
  ] as const)('returns the same draft for %s', (_what, zone, id, to, count) => {
    const start = deck(line(bolt, 'main', 2))
    expect(moveLine(start, zone, id, to, count)).toBe(start)
  })
})

describe('totals and order', () => {
  const start = deck(
    line(bolt, 'main', 4),
    line(forest, 'main', 20),
    line(bolt, 'sideboard', 2),
    line(ring, 'commander'),
  )

  it('counts copies per zone and in all', () => {
    expect(totalOf(start)).toBe(27)
    expect(totalOf(start, 'main')).toBe(24)
    expect(totalOf(start, 'sideboard')).toBe(2)
    expect(totalOf(start, 'companion')).toBe(0)
    expect(copiesOf(start, forest.oracle_id)).toBe(20)
    expect(copiesOf(start, 'oracle-nothing')).toBe(0)
  })

  it('lists the lines of a zone by name, ignoring case', () => {
    const mixed = deck(
      line(cardHit('zebra'), 'main'),
      line(cardHit('Apple'), 'main'),
      line(cardHit('banana'), 'main'),
    )
    expect(linesIn(mixed, 'main').map((item) => item.card.name)).toEqual(['Apple', 'banana', 'zebra'])
    expect(linesIn(start, 'companion')).toEqual([])
  })
})

describe('a draft from a stored deck', () => {
  it('has one line for a card in a zone, with the first printing that is named', () => {
    const view = deckView({
      cards: [
        lineOf('Forest', 2),
        lineOf('Lightning Bolt', 4, { printing_id: 'art-a' }),
        lineOf('Forest', 3, { printing_id: 'art-b' }),
        lineOf('Lightning Bolt', 1, { zone: 'sideboard' }),
      ],
    })
    const draft = draftOf(view)
    expect(counts(draft)).toEqual(['main:Forest:5', 'main:Lightning Bolt:4', 'sideboard:Lightning Bolt:1'])
    expect(draft.lines.map((item) => item.printingId)).toEqual(['art-b', 'art-a', null])
  })

  it('carries the name, format and notes', () => {
    expect(draftOf(deckView({ name: 'Burn', format: 'modern', notes: 'sideboard plan' }))).toMatchObject({
      name: 'Burn',
      format: 'modern',
      notes: 'sideboard plan',
    })
  })

  it('becomes the body of a save with the trimmed name and the lines in order', () => {
    const body = bodyOf({
      ...deck(line(forest, 'main', 20, 'art'), line(bolt, 'sideboard', 2)),
      name: '  Burn ',
    })
    expect(body).toEqual({
      name: 'Burn',
      format: 'modern',
      notes: '',
      cards: [
        { zone: 'main', oracle_id: forest.oracle_id, printing_id: 'art', count: 20 },
        { zone: 'sideboard', oracle_id: bolt.oracle_id, printing_id: null, count: 2 },
      ],
    })
  })

  it('is not changed from the deck it came from, as the dirty check sees it', () => {
    const view = deckView({ cards: [lineOf('Forest', 2), lineOf('Forest', 3)] })
    expect(sameDeck(draftOf(view), draftOf(view))).toBe(true)
  })
})

describe('sameDeck', () => {
  const base = deck(line(bolt, 'main', 4), line(ring, 'main', 1, 'art'))

  it('ignores the order of lines', () => {
    expect(sameDeck(base, deck(...[...base.lines].reverse()))).toBe(true)
  })

  it.each([
    ['the name', { ...base, name: 'Burn 2' }],
    ['the format', { ...base, format: 'legacy' }],
    ['the notes', { ...base, notes: 'x' }],
    ['a count', deck(line(bolt, 'main', 3), line(ring, 'main', 1, 'art'))],
    ['a zone', deck(line(bolt, 'sideboard', 4), line(ring, 'main', 1, 'art'))],
    ['a printing', deck(line(bolt, 'main', 4), line(ring, 'main', 1, 'other'))],
    ['a card', deck(line(bolt, 'main', 4), line(forest, 'main', 1, 'art'))],
    ['a missing line', deck(line(bolt, 'main', 4))],
  ])('sees a difference in %s', (_what, other) => {
    expect(sameDeck(base, other)).toBe(false)
  })
})
