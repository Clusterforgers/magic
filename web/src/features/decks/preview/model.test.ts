// Goal: the deck preview counts a deck right: each card falls in one type group, the curve buckets spells by mana value with 7+ merged and lands left out, support counts add up, and a stored deck and a precon read into the same shape.
import { describe, expect, it } from 'vitest'
import { cardHit, deckView, lineOf, reportCard, reportOf } from '@/features/decks/fakeDecks'
import type { PreconView } from '@/protocol/generated'
import {
  byHand,
  CURVE_LAST,
  groupByKind,
  identityOfDeck,
  kindOf,
  manaCurve,
  previewFromDeck,
  previewFromPrecon,
  supportCounts,
  typeCounts,
} from './model'
import type { PreviewCard } from './model'

const card = (name: string, over: Partial<PreviewCard> = {}): PreviewCard => ({
  oracleId: `o-${name}`,
  name,
  count: 1,
  typeLine: 'Instant',
  manaValue: 1,
  manaCost: '{R}',
  identity: ['R'],
  support: 'verified',
  reasons: [],
  ...over,
})

describe('kindOf', () => {
  it.each([
    ['Creature — Elf Druid', 'creature'],
    ['Legendary Creature — Human Wizard', 'creature'],
    ['Artifact Creature — Golem', 'creature'],
    ['Enchantment Creature — Centaur', 'creature'],
    ['Legendary Planeswalker — Jace', 'planeswalker'],
    ['Instant', 'instant'],
    ['Sorcery', 'sorcery'],
    ['Artifact', 'artifact'],
    ['Legendary Artifact — Equipment', 'artifact'],
    ['Enchantment — Aura', 'enchantment'],
    ['Basic Land — Forest', 'land'],
    ['Land', 'land'],
    ['Artifact Land', 'land'],
    ['Battle — Siege', 'battle'],
    ['Sorcery // Land', 'sorcery'],
    ['Legendary Enchantment // Land', 'enchantment'],
    ['Tribal Instant — Elf', 'instant'],
    ['Conspiracy', 'other'],
    ['', 'other'],
  ] as const)('puts %j among the %s', (typeLine, kind) => {
    expect(kindOf(typeLine)).toBe(kind)
  })
})

describe('groups and counts', () => {
  const cards = [
    card('Zeta', { typeLine: 'Creature — Elf', manaValue: 3, count: 2 }),
    card('Alpha', { typeLine: 'Creature — Elf', manaValue: 3 }),
    card('Bolt', { typeLine: 'Instant', manaValue: 1, count: 4 }),
    card('Forest', { typeLine: 'Basic Land — Forest', manaValue: 0, count: 30 }),
    card('Rift', { typeLine: 'Instant', manaValue: 2 }),
    card('Giant', { typeLine: 'Creature — Giant', manaValue: 9 }),
    card('Mox', { typeLine: 'Artifact', manaValue: 0 }),
  ]

  it('groups by type in drawing order and sorts a group by mana value, then name', () => {
    const groups = groupByKind(cards)
    expect(groups.map((group) => group.kind)).toEqual(['creature', 'instant', 'artifact', 'land'])
    expect(groups[0]?.cards.map((entry) => entry.name)).toEqual(['Alpha', 'Zeta', 'Giant'])
    expect(groups[1]?.cards.map((entry) => entry.name)).toEqual(['Bolt', 'Rift'])
    expect(groups.map((group) => group.count)).toEqual([4, 5, 1, 30])
    expect(groups.map((group) => group.label)).toEqual(['Creatures', 'Instants', 'Artifacts', 'Lands'])
  })

  it('leaves a type without cards out and an empty deck without groups', () => {
    expect(groupByKind([])).toEqual([])
    expect(typeCounts(cards).map((entry) => entry.kind)).not.toContain('sorcery')
  })

  it('counts copies by type', () => {
    expect(typeCounts(cards).map((entry) => [entry.kind, entry.count])).toEqual([
      ['creature', 4],
      ['instant', 5],
      ['artifact', 1],
      ['land', 30],
    ])
  })

  it('buckets spells by mana value, merges 7 and above and leaves lands out', () => {
    const curve = manaCurve(cards)
    expect(curve).toHaveLength(CURVE_LAST + 1)
    expect(curve).toEqual([1, 4, 1, 3, 0, 0, 0, 1])
  })

  it.each([
    [0, 0],
    [6, 6],
    [7, 7],
    [8, 7],
    [15, 7],
    [2.5, 2],
  ])('puts mana value %s in bucket %s', (manaValue, bucket) => {
    const curve = manaCurve([card('X', { manaValue })])
    expect(curve[bucket]).toBe(1)
    expect(curve.reduce((a, b) => a + b, 0)).toBe(1)
  })
})

describe('support counts', () => {
  const cards = [
    card('A', { count: 90 }),
    card('B', { count: 3, support: 'manual' }),
    card('C', { count: 2, support: 'partial' }),
    card('D', { support: 'stale' }),
    card('E', { count: 4, support: 'unplayable' }),
  ]

  it('adds copies by state and in all', () => {
    expect(supportCounts(cards)).toEqual({
      total: 100,
      verified: 90,
      partial: 2,
      manual: 3,
      stale: 1,
      unplayable: 4,
    })
  })

  it('counts what is played by hand without the unplayable', () => {
    expect(byHand(supportCounts(cards))).toBe(6)
  })
})

describe('colour identity', () => {
  it('is the commanders’ identity in WUBRG order, else the deck’s own', () => {
    const deck = {
      name: 'D',
      source: 's',
      format: 'commander',
      commanders: [card('C1', { identity: ['G', 'U'] }), card('C2', { identity: ['W'] })],
      cards: [card('X', { identity: ['R'] })],
      sideboard: [],
    }
    expect(identityOfDeck(deck)).toEqual(['W', 'U', 'G'])
    expect(identityOfDeck({ ...deck, commanders: [] })).toEqual(['R'])
    expect(identityOfDeck({ ...deck, commanders: [], cards: [] })).toEqual([])
  })
})

describe('adapters', () => {
  it('reads a stored deck by board, with the report’s support and reasons per card', () => {
    const view = deckView({
      name: 'Atraxa',
      cards: [
        lineOf('Atraxa', 1, { zone: 'commander', card: cardHit('Atraxa', { color_identity: ['W', 'U'] }) }),
        lineOf('Odd Card', 2),
        lineOf('Side Card', 1, { zone: 'sideboard' }),
      ],
      report: reportOf([
        reportCard('Atraxa', { zone: 'commander' }),
        reportCard('Odd Card', {
          count: 2,
          support: 'manual',
          reasons: [{ face: 0, span: { start: 0, end: 4 }, text: 'resolve it by hand' }],
        }),
      ]),
    })
    const data = previewFromDeck(view)
    expect(data.commanders.map((entry) => entry.name)).toEqual(['Atraxa'])
    expect(data.cards.map((entry) => [entry.name, entry.count, entry.support])).toEqual([['Odd Card', 2, 'manual']])
    expect(data.cards[0]?.reasons).toEqual(['Resolve it by hand.'])
    expect(data.sideboard.map((entry) => entry.name)).toEqual(['Side Card'])
    expect(data.source).toBe('Your deck')
  })

  it('reads a precon with its source line and leaves out an entry the card data lacks', () => {
    const view: PreconView = {
      precon: {
        code: 'c18-x',
        name: 'Draconic Domination',
        set: 'c18',
        deck_type: 'Commander Deck',
        release_date: null,
        commanders: ['The Ur-Dragon'],
        card_count: 3,
        format: 'commander',
        state: 'verified',
        verified: true,
        playable: true,
        summary: deckView().report.summary,
      },
      issues: [],
      boards: [
        {
          board: 'commander',
          copies: 1,
          cards: [{ name: 'The Ur-Dragon', count: 1, printing_id: null, card: cardHit('The Ur-Dragon'), support: 'verified', reasons: [] }],
        },
        {
          board: 'main',
          copies: 2,
          cards: [
            { name: 'Sol Ring', count: 1, printing_id: null, card: cardHit('Sol Ring'), support: 'verified', reasons: [] },
            { name: 'Unknown', count: 1, printing_id: null, card: null, support: 'unplayable', reasons: [] },
          ],
        },
      ],
    }
    const data = previewFromPrecon(view)
    expect(data.source).toBe('Precon C18, Commander Deck')
    expect(data.commanders).toHaveLength(1)
    expect(data.cards.map((entry) => entry.name)).toEqual(['Sol Ring'])
  })
})
