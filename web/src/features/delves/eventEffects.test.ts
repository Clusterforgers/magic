// Goal: an event option reads from its outcome data alone: gold, life, life cap, a relic of a rarity, a card, a loss and a commander bane become toned chips split into cost and reward, a gamble becomes a chance with both odds, a need the hero cannot meet (gold, life, life cap, a relic, a commander) says why the option is shut, and a result names who did what with what really moved.
import { describe, expect, it } from 'vitest'
import type { CommanderFacts, DelveOptionView, Needs, Outcome, Rarity, Resolved } from '@/protocol/generated'
import { hero, needs } from './fakeDelves'
import type { HeroFacts } from './eventEffects'
import {
  blockedReason,
  chipOf,
  heroFacts,
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
    [{ MaxLife: -5 }, 'cap', 'cost', '−5 max life for the run'],
    [{ MaxLife: 5 }, 'cap', 'gain', '+5 max life for the run'],
    ['LoseCard', 'card', 'cost', 'give up a card of your deck or Satchel'],
    ['LoseRelic', 'relic', 'cost', 'give up a relic'],
    [{ Bane: { Tax: 1 } }, 'bane', 'cost', 'Your commander costs {1} more to cast'],
    [{ Bane: { Shrink: 2 } }, 'bane', 'cost', 'Your commander gets −2/−2'],
  ])('draws %j as a %s chip, a %s: %s', (outcome, kind, tone, text) => {
    expect(chipOf(outcome)).toEqual({ kind, tone, text })
  })

  it.each<Rarity>(['Common', 'Rare', 'Mythic'])('draws a %s relic with its rarity', (rarity) => {
    expect(chipOf({ RelicOf: rarity })).toEqual({
      kind: 'relic',
      tone: 'gain',
      text: `a ${rarity} relic`,
      rarity,
    })
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
  const creature: CommanderFacts = {
    name: 'Ruric',
    creature: true,
    planeswalker: false,
    power: 4,
    generic: 2,
    keywords: [],
  }
  const walker: CommanderFacts = { ...creature, name: 'Teferi', creature: false, planeswalker: true }

  it.each<[Partial<Needs>, HeroFacts, string | null]>([
    [{ gold: 25 }, { gold: 25, life: 40 }, null],
    [{ gold: 25 }, { gold: 10, life: 40 }, 'Needs 25 gold: you have 10'],
    [{ life: 8 }, { gold: 0, life: 9 }, null],
    [{ life: 8 }, { gold: 0, life: 8 }, 'Needs more than 8 life: you have 8'],
    [{}, { gold: 0, life: 1 }, null],
    [{ max_life: 5 }, { gold: 0, life: 30, maxLife: 25 }, null],
    [{ max_life: 5 }, { gold: 0, life: 20, maxLife: 24 }, 'Needs a max life of 25: yours is 24'],
    [{ max_life: 5 }, { gold: 0, life: 20 }, null],
    [{ relic: true }, { gold: 0, life: 40, relics: 1 }, null],
    [{ relic: true }, { gold: 0, life: 40, relics: 0 }, 'Needs a relic to give up: you have none'],
    [{ commander: true }, { gold: 0, life: 40, commander: walker }, null],
    [{ commander: true }, { gold: 0, life: 40, commander: null }, 'Needs a commander: your deck has none'],
    [{ commander: true }, { gold: 0, life: 40 }, null],
    [{ commander: true, creature_commander: true }, { gold: 0, life: 40, commander: creature }, null],
    [
      { commander: true, creature_commander: true },
      { gold: 0, life: 40, commander: walker },
      'Needs a creature commander: Teferi is not a creature',
    ],
  ])('%j for %j: %s', (over, facts, reason) => {
    expect(blockedReason(needs(over), facts)).toBe(reason)
  })

  it('reads the hero: life cap, relics and first commander', () => {
    const facts = heroFacts(
      hero({
        gold: 7,
        life: 12,
        life_cap: -10,
        relics: ['idol'],
        deck: { id: 'd', name: 'D', colors: 'R', commanders: [walker, creature] },
      }),
    )
    expect(facts).toEqual({ gold: 7, life: 12, maxLife: 30, relics: 1, commander: walker })
    expect(
      heroFacts(hero({ deck: { id: 'd', name: 'D', colors: 'R', commanders: [] } })).commander,
    ).toBeNull()
    expect(heroFacts(hero()).commander).toBeUndefined()
  })

  it('pairs every option with its chips and its reason', () => {
    const options: DelveOptionView[] = [
      {
        label: 'Pay',
        outcomes: [{ Gold: -25 }],
        needs: needs({ gold: 25 }),
        summary: '−25 gold',
        cost: '25 gold',
        reward: 'nothing',
      },
      {
        label: 'Leave',
        outcomes: [],
        needs: needs(),
        summary: 'nothing',
        cost: 'nothing',
        reward: 'nothing',
      },
    ]
    const [pay, leave] = optionViews(options, { gold: 3, life: 40 })
    expect(pay?.blocked).toBe('Needs 25 gold: you have 3')
    expect(pay?.part.chips[0]?.tone).toBe('cost')
    expect(leave?.blocked).toBeNull()
  })
})

describe('cost and reward', () => {
  const split = (outcomes: Outcome[], commander?: CommanderFacts) => {
    const option: DelveOptionView = {
      label: 'x',
      outcomes,
      needs: needs(),
      summary: '',
      cost: '',
      reward: '',
    }
    const [view] = optionViews([option], { gold: 0, life: 40, commander })
    return {
      cost: view?.cost.map((c) => c.text),
      reward: view?.reward.chips.map((c) => c.text),
      chances: view?.reward.chances.length,
    }
  }

  it('puts what is paid on the cost side and what is gained on the reward side', () => {
    expect(split([{ Gold: -25 }, 'CardPick', { Life: -3 }])).toEqual({
      cost: ['−25 gold', '−3 life'],
      reward: ['a card for your Satchel'],
      chances: 0,
    })
  })

  it('leaves a free option without a cost and keeps a gamble on the reward side', () => {
    expect(split([{ Gamble: { odds: 40, win: ['Relic'], lose: [{ Life: -5 }] } }])).toEqual({
      cost: [],
      reward: [],
      chances: 1,
    })
  })

  it('names the hero’s commander in a bane', () => {
    const ruric: CommanderFacts = {
      name: 'Ruric',
      creature: true,
      planeswalker: false,
      power: 4,
      generic: 2,
      keywords: [],
    }
    expect(split([{ Bane: { Tax: 1 } }], ruric).cost).toEqual(['Ruric costs {1} more to cast'])
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
