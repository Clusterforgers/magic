// Goal: the lines the overlay draws follow from the combat alone, whatever its shape (nothing, an unblocked attack, an attack on a planeswalker, a single block in each of the four outcomes, a double block with a verdict per blocker, several attackers, a blocked attacker whose blockers left, no forecast, a forecast that forgets a creature) and from the choices a prompt has not sent yet, which come last and are marked; attacks are muted under blocks, and nothing is drawn before attackers are declared.
import { describe, expect, it } from 'vitest'
import type {
  AttackerView,
  AttackTarget,
  CombatForecast,
  CombatView,
  CreatureForecast,
  ObjectId,
} from '@/protocol/generated'
import { OUTCOMES, blockOutcome, diesIn, outcomeOf } from './outcome'
import type { Outcome } from './outcome'
import { NO_PENDING } from './pending'
import type { PendingCombat } from './pending'
import { anchorKey, anchorsOf, planLines } from './plan'
import type { LineSpec } from './plan'

const attacker = (
  object: ObjectId,
  target: AttackTarget,
  blockers: ObjectId[] = [],
  blocked = blockers.length > 0,
): AttackerView => ({ object, target, blocked, blockers })
const combat = (attackers: AttackerView[], declared = true): CombatView => ({
  attackers,
  attackers_declared: declared,
  blockers_declared: [],
})
const fate = (object: ObjectId, dies: boolean, damage = 0): CreatureForecast => ({ object, damage, dies })
const forecastOf = (...creatures: CreatureForecast[]): CombatForecast => ({
  attackers: [],
  creatures,
  players: [],
})

/** A line as one readable string: key, from > to, and what marks it. */
const show = (line: LineSpec): string =>
  `${line.key} ${anchorKey(line.from)}>${anchorKey(line.to)}${line.pending ? ' pending' : ''}${line.outcome === undefined ? '' : ` ${line.outcome}`}`

const plan = (
  view: CombatView | null,
  forecast: CombatForecast | null = null,
  pending: PendingCombat = NO_PENDING,
  controllerOf: (id: ObjectId) => number | undefined = () => undefined,
): LineSpec[] => planLines({ combat: view, forecast, pending, controllerOf })

const P0: AttackTarget = { Player: 0 }
const P2: AttackTarget = { Player: 2 }

describe('outcomeOf', () => {
  it.each<[boolean, boolean, Outcome]>([
    [true, false, 'attacker-dies'],
    [false, true, 'blocker-dies'],
    [true, true, 'both-die'],
    [false, false, 'both-survive'],
  ])('attacker dies %s and blocker dies %s is %s', (attackerDies, blockerDies, expected) => {
    expect(outcomeOf(attackerDies, blockerDies)).toBe(expected)
  })

  it('names each of the four outcomes once', () => {
    expect(new Set(OUTCOMES).size).toBe(4)
  })

  it('reads a block off the forecast, and says nothing when either creature is not listed', () => {
    const forecast = forecastOf(fate(1, true), fate(2, false))
    expect(blockOutcome(forecast, 1, 2)).toBe('attacker-dies')
    expect(blockOutcome(forecast, 1, 3)).toBeUndefined()
    expect(blockOutcome(forecast, 3, 2)).toBeUndefined()
    expect(diesIn(forecast, 2)).toBe(false)
    expect(diesIn(forecast, 9)).toBeUndefined()
  })
})

describe('planLines for the declared combat', () => {
  it.each<[string, CombatView | null, string[]]>([
    ['no combat at all', null, []],
    ['a combat with no attacker', combat([]), []],
    ['attackers that are not declared yet', combat([attacker(1, P0)], false), []],
    ['one unblocked attacker', combat([attacker(1, P0)]), ['attack:1 card:1>seat:0']],
    [
      'an attacker aimed at a planeswalker',
      combat([attacker(1, { Planeswalker: 9 })]),
      ['attack:1 card:1>card:9'],
    ],
    [
      'three attackers at two players, none blocked',
      combat([attacker(1, P0), attacker(2, P0), attacker(3, P2)]),
      ['attack:1 card:1>seat:0', 'attack:2 card:2>seat:0', 'attack:3 card:3>seat:2'],
    ],
    [
      'a blocked attacker whose blocker left: the attack stays, there is no block',
      combat([attacker(1, P0, [], true)]),
      ['attack:1 card:1>seat:0'],
    ],
    ['one block', combat([attacker(1, P0, [2])]), ['attack:1 card:1>seat:0', 'block:2 card:2>card:1']],
    [
      'a double block',
      combat([attacker(1, P0, [2, 3])]),
      ['attack:1 card:1>seat:0', 'block:2 card:2>card:1', 'block:3 card:3>card:1'],
    ],
    [
      'attacks first and blocks after, whatever the order of the attackers',
      combat([attacker(1, P0, [4]), attacker(2, P0), attacker(3, P2, [5, 6])]),
      [
        'attack:1 card:1>seat:0',
        'attack:2 card:2>seat:0',
        'attack:3 card:3>seat:2',
        'block:4 card:4>card:1',
        'block:5 card:5>card:3',
        'block:6 card:6>card:3',
      ],
    ],
  ])('%s', (_name, view, expected) => {
    expect(plan(view).map(show)).toEqual(expected)
  })

  it.each<[boolean, boolean, Outcome]>([
    [true, false, 'attacker-dies'],
    [false, true, 'blocker-dies'],
    [true, true, 'both-die'],
    [false, false, 'both-survive'],
  ])('colours a single block by the forecast: attacker dies %s, blocker dies %s', (a, b, outcome) => {
    const lines = plan(combat([attacker(1, P0, [2])]), forecastOf(fate(1, a), fate(2, b)))
    expect(lines.map(show)).toEqual(['attack:1 card:1>seat:0', `block:2 card:2>card:1 ${outcome}`])
  })

  it('gives each blocker of a double block its own outcome', () => {
    const forecast = forecastOf(fate(1, true), fate(2, true), fate(3, false))
    const lines = plan(combat([attacker(1, P0, [2, 3])]), forecast)
    expect(lines.filter((line) => line.kind === 'block').map(show)).toEqual([
      'block:2 card:2>card:1 both-die',
      'block:3 card:3>card:1 attacker-dies',
    ])
  })

  it('colours several blocks at once, one outcome per pair', () => {
    const forecast = forecastOf(
      fate(1, false),
      fate(2, true),
      fate(3, true),
      fate(4, false),
      fate(5, false),
      fate(6, false),
    )
    const lines = plan(combat([attacker(1, P0, [2]), attacker(3, P0, [4]), attacker(5, P2, [6])]), forecast)
    expect(lines.filter((line) => line.kind === 'block').map((line) => line.outcome)).toEqual([
      'blocker-dies',
      'attacker-dies',
      'both-survive',
    ])
  })

  it('leaves the attack lines without an outcome: only a block has one', () => {
    const lines = plan(combat([attacker(1, P0, [2])]), forecastOf(fate(1, true), fate(2, true)))
    expect(lines.filter((line) => line.kind === 'attack').map((line) => line.outcome)).toEqual([undefined])
  })

  it('draws blocks without an outcome when there is no forecast', () => {
    const lines = plan(combat([attacker(1, P0, [2, 3])]), null)
    expect(lines.map((line) => line.outcome)).toEqual([undefined, undefined, undefined])
  })

  it('draws a block without an outcome when the forecast forgot one of its creatures', () => {
    const lines = plan(combat([attacker(1, P0, [2, 3])]), forecastOf(fate(1, false), fate(2, true)))
    expect(lines.filter((line) => line.kind === 'block').map(show)).toEqual([
      'block:2 card:2>card:1 blocker-dies',
      'block:3 card:3>card:1',
    ])
  })

  it('keys every line apart, and the same way for the same combat', () => {
    const view = combat([attacker(1, P0, [4]), attacker(2, P0), attacker(3, P2, [5, 6])])
    const keys = plan(view).map((line) => line.key)
    expect(new Set(keys).size).toBe(keys.length)
    expect(plan(view).map((line) => line.key)).toEqual(keys)
  })

  it('knows who controls each card, so a card that is not drawn can stand in as its controller', () => {
    const controllers: Record<number, number> = { 1: 1, 2: 0, 9: 3 }
    const lines = plan(
      combat([attacker(1, { Planeswalker: 9 }, [2])]),
      null,
      NO_PENDING,
      (id) => controllers[id],
    )
    expect(lines.map((line) => [line.from, line.to])).toEqual([
      [
        { kind: 'card', id: 1, seat: 1 },
        { kind: 'card', id: 9, seat: 3 },
      ],
      [
        { kind: 'card', id: 2, seat: 0 },
        { kind: 'card', id: 1, seat: 1 },
      ],
    ])
  })
})

describe('planLines for choices not sent yet', () => {
  const blocks = (...pairs: [ObjectId, ObjectId][]): PendingCombat => ({
    ...NO_PENDING,
    blocks: new Map(pairs),
  })
  const attacks = (...pairs: [ObjectId, AttackTarget][]): PendingCombat => ({
    ...NO_PENDING,
    attacks: new Map(pairs),
  })

  it.each<[string, PendingCombat, string[]]>([
    ['nothing chosen', NO_PENDING, []],
    ['one block', blocks([2, 1]), ['pending-block:2 card:2>card:1 pending']],
    [
      'two blockers on one attacker and one on another, in the order made',
      blocks([2, 1], [3, 1], [4, 7]),
      [
        'pending-block:2 card:2>card:1 pending',
        'pending-block:3 card:3>card:1 pending',
        'pending-block:4 card:4>card:7 pending',
      ],
    ],
    ['one attack on a player', attacks([5, P2]), ['pending-attack:5 card:5>seat:2 pending']],
    [
      'attacks on a player and on a planeswalker',
      attacks([5, P2], [6, { Planeswalker: 9 }]),
      ['pending-attack:5 card:5>seat:2 pending', 'pending-attack:6 card:6>card:9 pending'],
    ],
  ])('draws %s as dashed lines, with no outcome', (_name, pending, expected) => {
    const lines = plan(null, null, pending)
    expect(lines.map(show)).toEqual(expected)
    expect(lines.every((line) => line.pending && line.outcome === undefined)).toBe(true)
  })

  it('puts them after the declared combat, on top of it', () => {
    const lines = plan(
      combat([attacker(1, P0, [2])]),
      forecastOf(fate(1, false), fate(2, false)),
      blocks([3, 1]),
    )
    expect(lines.map(show)).toEqual([
      'attack:1 card:1>seat:0',
      'block:2 card:2>card:1 both-survive',
      'pending-block:3 card:3>card:1 pending',
    ])
  })

  it('draws a choice even while attackers are not declared: that is when attacks are chosen', () => {
    const lines = plan(combat([], false), null, attacks([5, P0]))
    expect(lines.map(show)).toEqual(['pending-attack:5 card:5>seat:0 pending'])
  })
})

describe('anchorsOf', () => {
  it('lists each card and seat once, in the order the lines first touch them', () => {
    const lines = plan(combat([attacker(1, P0, [2]), attacker(3, P0, [4])]))
    expect(anchorsOf(lines).map(anchorKey)).toEqual(['card:1', 'seat:0', 'card:3', 'card:2', 'card:4'])
  })

  it('is empty for no lines', () => {
    expect(anchorsOf([])).toEqual([])
  })
})
