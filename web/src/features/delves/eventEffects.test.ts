// Goal: an event option reads from its outcome data alone: gold, life, a relic and a card become toned chips, a gamble becomes a chance with both odds, a sure cost the hero cannot pay says why the option is shut, and a result names who did what with what really moved.
import { describe, expect, it } from 'vitest'
import type { DelveOptionView, Outcome, Resolved } from '@/protocol/generated'
import { hero } from './fakeDelves'
import {
  blockedReason,
  chipOf,
  optionViews,
  partOf,
  resultSentence,
  resultViews,
  signed,
} from './eventEffects'

describe('chips', () => {
  it.each<[Outcome, string, string, string]>([
    [{ Gold: 40 }, 'gold', 'gain', '+40 gold'],
    [{ Gold: -25 }, 'gold', 'cost', '−25 gold'],
    [{ Life: -8 }, 'life', 'cost', '−8 life'],
    [{ Life: 8 }, 'life', 'gain', '+8 life'],
    [{ Life: 0 }, 'life', 'flat', 'no life change'],
    ['Relic', 'relic', 'gain', 'a relic'],
    ['CardPick', 'card', 'gain', 'a card for your Satchel'],
  ])('draws %j as a %s chip, a %s: %s', (outcome, kind, tone, text) => {
    expect(chipOf(outcome)).toEqual({ kind, tone, text })
  })

  it('has no chip for a gamble, which is a chance', () => {
    expect(chipOf({ Gamble: { odds: 50, win: [], lose: [] } })).toBeNull()
  })

  it.each([
    [0, '0'],
    [5, '+5'],
    [-12, '−12'],
  ])('writes %d as %s with a real minus', (n, text) => {
    expect(signed(n)).toBe(text)
  })
})

describe('chances', () => {
  it('shows a gamble with the odds of a win and of a loss', () => {
    const part = partOf([{ Gold: 20 }, { Gamble: { odds: 60, win: ['Relic'], lose: [{ Life: -8 }] } }])
    expect(part.chips.map((c) => c.text)).toEqual(['+20 gold'])
    expect(part.chances).toHaveLength(1)
    const [chance] = part.chances
    expect(chance?.odds).toBe(60)
    expect(chance?.win.chips.map((c) => c.text)).toEqual(['a relic'])
    expect(chance?.lose.chips.map((c) => c.text)).toEqual(['−8 life'])
  })

  it('keeps an empty branch empty and reads a chance inside a branch', () => {
    const part = partOf([
      { Gamble: { odds: 50, win: [], lose: [{ Gamble: { odds: 25, win: ['CardPick'], lose: [] } }] } },
    ])
    expect(part.chances[0]?.win).toEqual({ chips: [], chances: [] })
    expect(part.chances[0]?.lose.chances[0]?.odds).toBe(25)
  })
})

describe('what blocks an option', () => {
  it.each<[{ gold: number; life: number }, number, number, string | null]>([
    [{ gold: 25, life: 0 }, 25, 40, null],
    [{ gold: 25, life: 0 }, 10, 40, 'Needs 25 gold: you have 10'],
    [{ gold: 0, life: 8 }, 0, 9, null],
    [{ gold: 0, life: 8 }, 0, 8, 'Needs more than 8 life: you have 8'],
    [{ gold: 0, life: 0 }, 0, 1, null],
  ])('%j with %d gold and %d life: %s', (needs, gold, life, reason) => {
    expect(blockedReason(needs, gold, life)).toBe(reason)
  })

  it('pairs every option with its chips and its reason', () => {
    const options: DelveOptionView[] = [
      { label: 'Pay', outcomes: [{ Gold: -25 }], needs: { gold: 25, life: 0 }, summary: '−25 gold' },
      { label: 'Leave', outcomes: [], needs: { gold: 0, life: 0 }, summary: 'nothing' },
    ]
    const [pay, leave] = optionViews(options, 3, 40)
    expect(pay?.blocked).toBe('Needs 25 gold: you have 3')
    expect(pay?.part.chips[0]?.tone).toBe('cost')
    expect(leave?.blocked).toBeNull()
  })
})

describe('results', () => {
  const heroes = [hero({ name: 'Ana' }), hero({ name: 'Bo' }), hero({ name: 'Cy' })]
  const results: Resolved[] = [
    { hero: 1, what: 'chose “Push past”', outcomes: [{ Life: -8 }, { Gold: 40 }], rolls: [] },
    {
      hero: 0,
      what: 'chose “Dive”',
      outcomes: [{ Life: -8 }],
      rolls: [{ odds: 60, won: false }],
    },
    { hero: 2, what: 'took the relic Phoenix Feather', outcomes: [], rolls: [] },
  ]

  it('puts the viewer first as You and names the others', () => {
    const rows = resultViews(results, heroes, 0)
    expect(rows.map((r) => r.who)).toEqual(['You', 'Bo', 'Cy'])
    expect(rows[0]?.rolls).toEqual(['Lost the 60% gamble'])
  })

  it('reads a result as one sentence', () => {
    const rows = resultViews(results, heroes, 0)
    expect(rows.map(resultSentence)).toEqual([
      'You chose “Dive”: Lost the 60% gamble, −8 life.',
      'Bo chose “Push past”: −8 life, +40 gold.',
      'Cy took the relic Phoenix Feather.',
    ])
  })

  it('names everyone when the viewer is not in the party', () => {
    expect(resultViews(results, heroes, undefined).map((r) => r.who)).toEqual(['Bo', 'Ana', 'Cy'])
  })
})
