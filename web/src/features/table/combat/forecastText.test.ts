// Goal: the forecast reads as plain sentences for every shape of combat (an unblocked attacker, a block, a double and a triple block, a blocked attacker with no blocker left, an attack on a planeswalker, creatures the forecast forgot, players with damage, none or lethal), one sentence per attacker and per attacked player in declared order, and a card's spoken name and a seat's life line gain the right words.
import { describe, expect, it } from 'vitest'
import type {
  AttackerForecast,
  CombatForecast,
  CreatureForecast,
  ObjectId,
  PlayerForecast,
} from '@/protocol/generated'
import { forecastSentences, lifeWords, listOf, outlookWords } from './forecastText'
import type { ForecastNames } from './forecastText'

const CREATURES: Record<number, string> = {
  1: "Bo's Hill Giant",
  2: 'your Grizzly Bears',
  3: 'your Elf',
  4: 'your Goblin',
  5: "Cy's Wall",
  9: "Bo's Jace",
}
const PLAYERS = ['you', 'Bo', 'Cy']

const names: ForecastNames = {
  creature: (id) => CREATURES[id] ?? `card ${id}`,
  player: (seat) => PLAYERS[seat] ?? `seat ${seat}`,
  target: (target) =>
    'Player' in target ? (PLAYERS[target.Player] ?? '?') : (CREATURES[target.Planeswalker] ?? '?'),
}

const attack = (over: Partial<AttackerForecast> & { attacker: ObjectId }): AttackerForecast => ({
  target: { Player: 0 },
  blocked: false,
  blockers: [],
  deals: [],
  receives: [],
  ...over,
})
const fate = (object: ObjectId, dies: boolean, damage = 1): CreatureForecast => ({ object, damage, dies })
const hit = (damage: number, life: number, lethal = false, player = 0): PlayerForecast => ({
  player,
  damage,
  life,
  life_after: life - damage,
  lethal,
})
const forecast = (
  attackers: AttackerForecast[],
  creatures: CreatureForecast[] = [],
  players: PlayerForecast[] = [],
): CombatForecast => ({ attackers, creatures, players })

describe('forecastSentences', () => {
  it.each<[string, CombatForecast, string[]]>([
    ['an empty forecast', forecast([]), []],
    [
      'an unblocked attacker',
      forecast([attack({ attacker: 1, deals: [{ source: 1, target: { Player: 0 }, amount: 4 }] })]),
      ["Bo's Hill Giant is unblocked and would deal 4 damage to you."],
    ],
    [
      "the viewer's own attacker, which starts a sentence with a capital",
      forecast([
        attack({
          attacker: 2,
          target: { Player: 1 },
          deals: [{ source: 2, target: { Player: 1 }, amount: 2 }],
        }),
      ]),
      ['Your Grizzly Bears is unblocked and would deal 2 damage to Bo.'],
    ],
    [
      'an unblocked attacker with no power',
      forecast([attack({ attacker: 1, deals: [{ source: 1, target: { Player: 0 }, amount: 0 }] })]),
      ["Bo's Hill Giant is unblocked and would deal no damage to you."],
    ],
    [
      'an unblocked attacker that deals nothing at all',
      forecast([attack({ attacker: 1 })]),
      ["Bo's Hill Giant is unblocked and would deal no damage to you."],
    ],
    [
      'an unblocked attacker aimed at a planeswalker',
      forecast([
        attack({
          attacker: 1,
          target: { Planeswalker: 9 },
          deals: [{ source: 1, target: { Object: 9 }, amount: 3 }],
        }),
      ]),
      ["Bo's Hill Giant is unblocked and would deal 3 damage to Bo's Jace."],
    ],
    [
      'an unblocked attacker aimed at another player',
      forecast([
        attack({
          attacker: 1,
          target: { Player: 2 },
          deals: [{ source: 1, target: { Player: 2 }, amount: 2 }],
        }),
      ]),
      ["Bo's Hill Giant is unblocked and would deal 2 damage to Cy."],
    ],
    [
      'a block where only the blocker dies',
      forecast([attack({ attacker: 1, blocked: true, blockers: [2] })], [fate(1, false), fate(2, true)]),
      [
        "Bo's Hill Giant is blocked by your Grizzly Bears: Bo's Hill Giant would survive, your Grizzly Bears would die.",
      ],
    ],
    [
      'a block where only the attacker dies',
      forecast([attack({ attacker: 1, blocked: true, blockers: [2] })], [fate(1, true), fate(2, false)]),
      [
        "Bo's Hill Giant is blocked by your Grizzly Bears: Bo's Hill Giant would die, your Grizzly Bears would survive.",
      ],
    ],
    [
      'a block where both die',
      forecast([attack({ attacker: 1, blocked: true, blockers: [2] })], [fate(1, true), fate(2, true)]),
      [
        "Bo's Hill Giant is blocked by your Grizzly Bears: Bo's Hill Giant would die, your Grizzly Bears would die.",
      ],
    ],
    [
      'a double block',
      forecast(
        [attack({ attacker: 1, blocked: true, blockers: [2, 3] })],
        [fate(1, true), fate(2, true), fate(3, false)],
      ),
      [
        "Bo's Hill Giant is blocked by your Grizzly Bears and your Elf: Bo's Hill Giant would die, your Grizzly Bears would die, your Elf would survive.",
      ],
    ],
    [
      'a triple block',
      forecast([attack({ attacker: 1, blocked: true, blockers: [2, 3, 4] })], []),
      ["Bo's Hill Giant is blocked by your Grizzly Bears, your Elf and your Goblin."],
    ],
    [
      'a block whose creatures the forecast forgot, except one',
      forecast([attack({ attacker: 1, blocked: true, blockers: [2, 3] })], [fate(3, true)]),
      ["Bo's Hill Giant is blocked by your Grizzly Bears and your Elf: your Elf would die."],
    ],
    [
      'an attacker that stays blocked after its blockers left',
      forecast([attack({ attacker: 1, blocked: true, blockers: [] })]),
      ["Bo's Hill Giant was blocked but nothing blocks it now, so it would deal no damage."],
    ],
    [
      'a player who survives',
      forecast([], [], [hit(4, 20)]),
      ['You would take 4 damage, from 20 to 16 life.'],
    ],
    [
      'a player who takes lethal damage',
      forecast([], [], [hit(7, 5, true)]),
      ['You would take 7 damage, from 5 to -2 life, which is lethal.'],
    ],
    [
      'a player whose attackers were all blocked',
      forecast([], [], [hit(0, 20)]),
      ['You would take no damage.'],
    ],
    [
      'two players',
      forecast([], [], [hit(3, 40, false, 1), hit(5, 4, true, 2)]),
      [
        'Bo would take 3 damage, from 40 to 37 life.',
        'Cy would take 5 damage, from 4 to -1 life, which is lethal.',
      ],
    ],
    [
      'a whole combat: the players first, then the attackers in declared order',
      forecast(
        [
          attack({ attacker: 1, blocked: true, blockers: [2] }),
          attack({
            attacker: 5,
            target: { Player: 1 },
            deals: [{ source: 5, target: { Player: 1 }, amount: 1 }],
          }),
        ],
        [fate(1, false), fate(2, true)],
        [hit(1, 40, false, 1)],
      ),
      [
        'Bo would take 1 damage, from 40 to 39 life.',
        "Bo's Hill Giant is blocked by your Grizzly Bears: Bo's Hill Giant would survive, your Grizzly Bears would die.",
        "Cy's Wall is unblocked and would deal 1 damage to Bo.",
      ],
    ],
  ])('says %s', (_name, given, expected) => {
    expect(forecastSentences(given, names)).toEqual(expected)
  })
})

describe('listOf', () => {
  it.each<[string[], string]>([
    [[], ''],
    [['a'], 'a'],
    [['a', 'b'], 'a and b'],
    [['a', 'b', 'c'], 'a, b and c'],
    [['a', 'b', 'c', 'd'], 'a, b, c and d'],
  ])('%j reads %j', (items, expected) => {
    expect(listOf(items)).toBe(expected)
  })
})

describe('outlookWords', () => {
  it.each<[string, Parameters<typeof outlookWords>[0], string]>([
    ['nothing to say', null, ''],
    ['damage that is survived', { damage: 3, dies: false }, ', would take 3 damage'],
    ['one point of damage', { damage: 1, dies: false }, ', would take 1 damage'],
    ['damage that kills', { damage: 3, dies: true }, ', would take 3 damage and die'],
    ['a death without damage', { damage: 0, dies: true }, ', would die'],
  ])('for %s', (_name, outlook, expected) => {
    expect(outlookWords(outlook)).toBe(expected)
  })
})

describe('lifeWords', () => {
  it.each<[string, PlayerForecast, string]>([
    ['damage', hit(4, 20), 'Life would go from 20 to 16'],
    ['lethal damage', hit(7, 5, true), 'Life would go from 5 to -2, which is lethal'],
    ['none', hit(0, 20), 'Takes no combat damage'],
  ])('for %s', (_name, player, expected) => {
    expect(lifeWords(player)).toBe(expected)
  })
})
