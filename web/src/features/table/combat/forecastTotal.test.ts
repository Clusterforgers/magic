// Goal: the dock's total adds up only the engine's numbers, from the viewer's side, for every shape of combat (an attack through, blocks that trade, a planeswalker attacked or defended, a team, a third seat's fight, lethal either way, a choice of effects); its chips show only what is not zero, each worded and labelled; the outlook of a card carries what it deals and an attacked planeswalker's fate; a seat's walker chips list only its own walkers that are hurt.
import { describe, expect, it } from 'vitest'
import type { CombatForecast, ObjectId, PlayerId } from '@/protocol/generated'
import { creatureOutlook } from './forecast'
import { outlookText } from './forecastText'
import { forecastTotal, seatWalkers, totalChips, walkerWords } from './forecastTotal'
import type { ForecastTotal } from './forecastTotal'

/** Seat 0 controls 1..9, seat 1 controls 10..19, seat 2 controls 20..29; 9 and 19 are planeswalkers. */
const controllerOf = (id: ObjectId): PlayerId | undefined => (id < 30 ? Math.floor(id / 10) : undefined)

const player = (seat: PlayerId, damage: number, life = 20) => ({
  player: seat,
  damage,
  life,
  life_after: life - damage,
  lethal: damage > 0 && life - damage <= 0,
})

/** Seat 0 attacks seat 1: 1 (4 power) is unblocked, 2 (3/3) is blocked by 10 (2/2), 3 (2 power) attacks walker 19. */
const ATTACK: CombatForecast = {
  attackers: [
    {
      attacker: 1,
      target: { Player: 1 },
      blocked: false,
      blockers: [],
      deals: [{ source: 1, target: { Player: 1 }, amount: 4 }],
      receives: [],
    },
    {
      attacker: 2,
      target: { Player: 1 },
      blocked: true,
      blockers: [10],
      deals: [{ source: 2, target: { Object: 10 }, amount: 3 }],
      receives: [{ source: 10, target: { Object: 2 }, amount: 2 }],
    },
    {
      attacker: 3,
      target: { Planeswalker: 19 },
      blocked: false,
      blockers: [],
      deals: [{ source: 3, target: { Object: 19 }, amount: 2 }],
      receives: [],
    },
  ],
  creatures: [
    { object: 1, damage: 0, dies: false, dealt: 4 },
    { object: 2, damage: 2, dies: false, dealt: 3 },
    { object: 3, damage: 0, dies: false, dealt: 2 },
    { object: 10, damage: 3, dies: true, dealt: 2 },
  ],
  players: [player(1, 4)],
  attacked: [{ object: 19, damage: 2, dies: true }],
}

const total = (forecast: CombatForecast, side: PlayerId[]): ForecastTotal =>
  forecastTotal(forecast, new Set(side), controllerOf)

describe('forecastTotal', () => {
  it.each<[string, CombatForecast, PlayerId[], Partial<ForecastTotal>]>([
    [
      'the attacker: damage to the player and the walker, not to a blocker',
      ATTACK,
      [0],
      { deals: 4 + 2, takes: 0, lost: 0, killed: 2, dealsLethal: false, takesLethal: false },
    ],
    [
      'the defender: what their life and walker take, and their losses',
      ATTACK,
      [1],
      { deals: 0, takes: 4 + 2, lost: 2, killed: 0 },
    ],
    ['a third seat: nothing of theirs is touched', ATTACK, [2], { deals: 0, takes: 0, lost: 0, killed: 2 }],
    [
      'a team: the attacker with the defender as a teammate counts nothing as dealt',
      ATTACK,
      [0, 1],
      { deals: 0, takes: 6, lost: 2, killed: 0 },
    ],
    [
      'lethal to the defender',
      { ...ATTACK, players: [player(1, 4, 3)] },
      [0],
      { dealsLethal: true, takesLethal: false },
    ],
    [
      'lethal to the viewer',
      { ...ATTACK, players: [player(1, 4, 3)] },
      [1],
      { dealsLethal: false, takesLethal: true },
    ],
    [
      'a forecast with no planeswalker list from an older server',
      { attackers: ATTACK.attackers.slice(0, 2), creatures: ATTACK.creatures, players: ATTACK.players },
      [0],
      { deals: 4, killed: 1 },
    ],
    ['a choice of effects', { ...ATTACK, depends_on_choice: true }, [0], { choice: true }],
  ])('for %s', (_name, forecast, side, expected) => {
    expect(total(forecast, side)).toMatchObject(expected)
  })
})

describe('totalChips', () => {
  const none: ForecastTotal = {
    deals: 0,
    takes: 0,
    takesLethal: false,
    dealsLethal: false,
    lost: 0,
    killed: 0,
    choice: false,
  }

  it('shows only the numbers that are not zero, in order, each with words and a sentence', () => {
    const chips = totalChips({ ...none, deals: 6, lost: 1, killed: 2 })
    expect(chips.map((chip) => [chip.kind, chip.text])).toEqual([
      ['deals', 'Deal 6'],
      ['lost', 'Lose 1'],
      ['killed', 'Kill 2'],
    ])
    expect(chips.map((chip) => chip.label)).toEqual([
      'You would deal 6 damage',
      'You would lose 1 creature',
      '2 creatures of the other side would die',
    ])
  })

  it('says one chip when nothing would happen', () => {
    expect(totalChips(none)).toEqual([
      { kind: 'none', text: 'No damage', label: 'No damage would be dealt', lethal: false },
    ])
  })

  it('marks lethal damage in words, not only in colour', () => {
    const [deal, take] = totalChips({ ...none, deals: 5, dealsLethal: true, takes: 20, takesLethal: true })
    expect(deal).toMatchObject({ lethal: true, label: 'You would deal 5 damage, which is lethal' })
    expect(take).toMatchObject({ lethal: true, label: 'You would take 20 damage, which is lethal' })
  })

  it('speaks for a team and marks numbers that depend on a choice', () => {
    const [take] = totalChips({ ...none, takes: 3, choice: true }, true)
    expect(take).toMatchObject({ text: 'Take ≈3', label: 'Your team would take 3 damage' })
  })
})

describe('the outlook of a card', () => {
  it.each<[string, ObjectId, ReturnType<typeof creatureOutlook>]>([
    ['an unblocked attacker deals and takes nothing', 1, { damage: 0, dies: false, dealt: 4 }],
    ['a blocked attacker deals and takes', 2, { damage: 2, dies: false, dealt: 3 }],
    ['a blocker that dies deals too', 10, { damage: 3, dies: true, dealt: 2 }],
    ['an attacked planeswalker falls', 19, { damage: 2, dies: true }],
    ['a card the forecast leaves alone', 5, null],
  ])('%s', (_name, id, expected) => {
    expect(creatureOutlook(ATTACK, id)).toEqual(expected)
  })

  it.each<[string, Parameters<typeof outlookText>[0], string]>([
    ['deals only', { damage: 0, dies: false, dealt: 4 }, 'would deal 4 damage'],
    ['deals and takes', { damage: 2, dies: false, dealt: 3 }, 'would deal 3 damage and take 2 damage'],
    [
      'deals, takes and dies',
      { damage: 3, dies: true, dealt: 2 },
      'would deal 2 damage, take 3 damage and die',
    ],
    ['takes and dies', { damage: 3, dies: true }, 'would take 3 damage and die'],
  ])('is said in words when it %s', (_name, outlook, expected) => {
    expect(outlookText(outlook)).toBe(expected)
  })
})

describe('the walkers of a seat', () => {
  it('lists only its own attacked planeswalkers that take damage or fall', () => {
    const forecast: CombatForecast = {
      ...ATTACK,
      attacked: [
        { object: 19, damage: 2, dies: true },
        { object: 18, damage: 0, dies: false },
        { object: 9, damage: 1, dies: false },
      ],
    }
    expect(seatWalkers(forecast, 1, controllerOf).map((walker) => walker.object)).toEqual([19])
    expect(seatWalkers(forecast, 0, controllerOf).map((walker) => walker.object)).toEqual([9])
    expect(seatWalkers(forecast, 2, controllerOf)).toEqual([])
    expect(seatWalkers(null, 1, controllerOf)).toEqual([])
  })

  it('names the walker and its fate', () => {
    expect(walkerWords('Chandra', { object: 19, damage: 2, dies: true })).toBe(
      'Chandra would take 2 damage and die',
    )
    expect(walkerWords('Chandra', { object: 19, damage: 1, dies: false }, true)).toBe(
      'Chandra would take 1 damage, depending on a choice of effects',
    )
  })
})
