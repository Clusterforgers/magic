// Goal: the draft reads as a deck preview with each line's verdict, groups its lines by type in the order the preview draws them (by mana value for the name sort, as given for the issues sort), lists each card the game leaves to the table once with its copies and reasons, worst state first, and words how many are played by hand.
import { describe, expect, it } from 'vitest'
import { cardHit, counts } from '@/features/decks/fakeDecks'
import type { Draft, Line } from './deckEdit'
import { groupLines, handCards, handSentence, previewFromDraft } from './deckSummary'
import { checksOf } from './report'
import type { LineCheck } from './report'

const hit = (name: string, type: string, mv = 1) =>
  cardHit(name, { type_line: type, mana_value: mv, oracle_id: `o-${name}` })
const line = (name: string, type: string, count: number, over: Partial<Line> = {}): Line => ({
  zone: 'main',
  card: hit(name, type, over.card?.mana_value ?? 1),
  count,
  printingId: null,
  ...over,
})
const draftOf = (lines: Line[]): Draft => ({ name: 'D', format: 'commander', notes: '', lines })
const NONE: ReadonlyMap<string, LineCheck> = new Map()
const check = (support: LineCheck['support'], ...texts: string[]): LineCheck => ({
  support,
  reasons: texts.map((text) => ({ face: 0, span: { start: 0, end: 1 }, text })),
})

describe('previewFromDraft', () => {
  const lines = [
    line('Krenko', 'Legendary Creature — Goblin', 1, { zone: 'commander' }),
    line('Goblin', 'Creature — Goblin', 4),
    line('Mountain', 'Basic Land — Mountain', 30),
    line('Duress', 'Sorcery', 2, { zone: 'sideboard' }),
    line('Ally', 'Creature — Elf', 1, { zone: 'companion' }),
  ]

  it('puts the command zone, the main deck with the companion and the sideboard on their own boards', () => {
    const preview = previewFromDraft(draftOf(lines), NONE)
    expect(preview.commanders.map((card) => card.name)).toEqual(['Krenko'])
    expect(preview.cards.map((card) => card.name)).toEqual(['Goblin', 'Mountain', 'Ally'])
    expect(preview.sideboard.map((card) => card.name)).toEqual(['Duress'])
    expect(preview.cards[0]).toMatchObject({ count: 4, typeLine: 'Creature — Goblin', support: 'verified' })
  })

  it('carries the check of each card: its support and the reasons as sentences, none for a verified card', () => {
    const checks = new Map<string, LineCheck>([
      ['o-Goblin', check('partial', 'the engine does not run X yet')],
      ['o-Mountain', check('verified', 'ignored')],
    ])
    const preview = previewFromDraft(draftOf(lines), checks)
    expect(preview.cards[0]).toMatchObject({
      support: 'partial',
      reasons: ['The engine does not run X yet.'],
    })
    expect(preview.cards[1]).toMatchObject({ support: 'verified', reasons: [] })
  })

  it('keeps the support a card came with while the check has not seen it', () => {
    const odd = {
      ...line('Odd', 'Instant', 1),
      card: { ...hit('Odd', 'Instant'), support: 'manual' as const },
    }
    expect(previewFromDraft(draftOf([odd]), NONE).cards[0]?.support).toBe('manual')
  })
})

describe('groupLines', () => {
  const lines = [
    line('Zed', 'Creature — Elf', 1, { card: hit('Zed', 'Creature — Elf', 2) }),
    line('Abe', 'Creature — Elf', 3, { card: hit('Abe', 'Creature — Elf', 5) }),
    line('Bob', 'Creature — Elf', 1, { card: hit('Bob', 'Creature — Elf', 2) }),
    line('Bolt', 'Instant', 4),
    line('Forest', 'Basic Land — Forest', 20),
  ]

  it('groups by type in the preview order, with the copies of each group and none for an empty type', () => {
    const groups = groupLines(lines, 'name')
    expect(groups.map((group) => [group.kind, group.label, group.count])).toEqual([
      ['creature', 'Creatures', 5],
      ['instant', 'Instants', 4],
      ['land', 'Lands', 20],
    ])
  })

  it('runs a group by mana value and then name for the name sort', () => {
    expect(groupLines(lines, 'name')[0]?.lines.map((l) => l.card.name)).toEqual(['Bob', 'Zed', 'Abe'])
  })

  it('keeps the order the lines come in for the issues sort', () => {
    expect(groupLines(lines, 'issues')[0]?.lines.map((l) => l.card.name)).toEqual(['Zed', 'Abe', 'Bob'])
  })
})

describe('handCards', () => {
  it('lists the cards that are not verified, worst state first and then by name, a card in two zones once', () => {
    const lines = [
      line('Verified', 'Instant', 4),
      line('Stale One', 'Instant', 1),
      line('Manual B', 'Instant', 2),
      line('Manual A', 'Instant', 1),
      line('Manual A', 'Instant', 2, { zone: 'sideboard' }),
      line('Blocked', 'Instant', 1),
    ]
    const checks = new Map<string, LineCheck>([
      ['o-Stale One', check('stale')],
      ['o-Manual B', check('manual', 'b')],
      ['o-Manual A', check('manual', 'a one', 'a two')],
      ['o-Blocked', check('unplayable')],
    ])
    const found = handCards(draftOf(lines), checks)
    expect(found.map((entry) => [entry.card.name, entry.count, entry.support])).toEqual([
      ['Blocked', 1, 'unplayable'],
      ['Manual A', 3, 'manual'],
      ['Manual B', 2, 'manual'],
      ['Stale One', 1, 'stale'],
    ])
    expect(found[1]?.reasons).toEqual(['A one.', 'A two.'])
  })

  it('is empty for a deck the game runs whole', () => {
    expect(handCards(draftOf([line('Bolt', 'Instant', 4)]), NONE)).toEqual([])
  })

  it('reads the checks of a report by oracle id', () => {
    const report = { cards: [{ oracle_id: 'o-Bolt', support: 'manual', reasons: [] }] }
    const checks = checksOf(report as unknown as Parameters<typeof checksOf>[0])
    expect(handCards(draftOf([line('Bolt', 'Instant', 4)]), checks)[0]?.support).toBe('manual')
  })
})

describe('handSentence', () => {
  it.each([
    [counts({ total: 0 }), 'No cards to count.'],
    [counts({ total: 10, verified: 10 }), 'The whole deck runs.'],
    [counts({ total: 10, verified: 7, partial: 1, manual: 1, stale: 1 }), '3 cards played by hand'],
    [counts({ total: 10, verified: 9, manual: 1 }), '1 card played by hand'],
    [
      counts({ total: 10, verified: 8, manual: 1, unplayable: 1 }),
      '1 card played by hand, 1 cannot be played',
    ],
    [counts({ total: 10, verified: 9, unplayable: 1 }), '1 cannot be played'],
  ])('words %j as "%s"', (given, words) => {
    expect(handSentence(given)).toBe(words)
  })
})
