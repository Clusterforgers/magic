// Goal: the dev forecast agrees with the cards it is drawn for (damage is power, lethal damage goes to each blocker in turn and the rest to the last, marked damage counts, a creature dies at its toughness, a player at zero life), and the two dev scenes built on it hold what the screens must be shown: every outcome, a double block, an unblocked attacker and a lethal player in `forecast`, and the viewer on lethal damage with a double block by another defender in `block`, at every seat count and from either side of the table, all of it valid on the wire.
import { describe, expect, it } from 'vitest'
import { isCombatForecast, isStateView } from '@/net/parse'
import type { AttackerView, CombatView, ObjectId, StateView } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { blockOutcome } from '../combat/outcome'
import type { Outcome } from '../combat/outcome'
import { forecastScene } from './combatScene'
import { devForecast } from './devForecast'
import { fixtureGame } from './devGame'
import { buildFixture } from './fixture'

/** The fixture's first creatures of a seat with stats, damage and a combat of our own, for hand-made cases. */
function table(
  stats: Record<number, [power: number, toughness: number, marked?: number]>,
  attackers: AttackerView[],
  lives: Record<number, number> = {},
): StateView {
  const base = buildFixture({ seats: 3 }).view
  const objects = base.objects.map((object) => {
    const given = stats[object.id]
    if (given === undefined || object.permanent === null) return object
    return {
      ...object,
      power: given[0],
      toughness: given[1],
      permanent: { ...object.permanent, damage: given[2] ?? 0 },
    }
  })
  const combat: CombatView = { attackers, attackers_declared: true, blockers_declared: [] }
  const players = base.players.map((player) => ({ ...player, life: lives[player.seat] ?? player.life }))
  return { ...base, objects, combat, players }
}

const creaturesOf = (seat: number, count: number): ObjectId[] =>
  buildFixture({ seats: 3 })
    .view.objects.filter(
      (object) =>
        object.controller === seat && object.zone === 'Battlefield' && object.types.includes('Creature'),
    )
    .slice(0, count)
    .map((object) => object.id)

const [A, B, C] = creaturesOf(1, 3) as [ObjectId, ObjectId, ObjectId]
const [X, Y, Z] = creaturesOf(0, 3) as [ObjectId, ObjectId, ObjectId]
const at0 = { Player: 0 }

describe('devForecast', () => {
  it('has nothing to say about no combat', () => {
    expect(devForecast(table({}, []))).toEqual({ attackers: [], creatures: [], players: [] })
  })

  it('sends an unblocked attacker at the player: damage is power, and a creature without damage does not die', () => {
    const forecast = devForecast(
      table({ [A]: [4, 4] }, [{ object: A, target: at0, blocked: false, blockers: [] }], { 0: 20 }),
    )
    expect(forecast.attackers[0]).toMatchObject({
      attacker: A,
      blocked: false,
      deals: [{ source: A, target: at0, amount: 4 }],
      receives: [],
    })
    expect(forecast.creatures).toEqual([{ object: A, damage: 0, dies: false }])
    expect(forecast.players).toEqual([{ player: 0, damage: 4, life: 20, life_after: 16, lethal: false }])
  })

  it.each<[string, [number, number], [number, number], Outcome]>([
    ['the attacker is bigger', [4, 4], [1, 2], 'blocker-dies'],
    ['the blocker is bigger', [1, 1], [3, 3], 'attacker-dies'],
    ['they are even', [2, 2], [2, 2], 'both-die'],
    ['neither can hurt the other', [1, 4], [1, 4], 'both-survive'],
    ['the attacker has no power', [0, 3], [1, 1], 'both-survive'],
  ])('settles a single block when %s', (_name, attacker, blocker, expected) => {
    const view = table({ [A]: attacker, [X]: blocker }, [
      { object: A, target: at0, blocked: true, blockers: [X] },
    ])
    const forecast = devForecast(view)
    expect(blockOutcome(forecast, A, X)).toBe(expected)
    expect(forecast.attackers[0]?.deals).toEqual([{ source: A, target: { Object: X }, amount: attacker[0] }])
    expect(forecast.attackers[0]?.receives).toEqual([
      { source: X, target: { Object: A }, amount: blocker[0] },
    ])
    expect(forecast.players).toEqual([{ player: 0, damage: 0, life: 40, life_after: 40, lethal: false }])
  })

  it.each<[string, number, number[], number[]]>([
    ['more than lethal to the first', 7, [3, 2], [3, 4]],
    ['exactly lethal to both', 5, [3, 2], [3, 2]],
    ['short of lethal to the first', 2, [3, 2], [2, 0]],
    ['lethal to the first and a little on the second', 4, [3, 2], [3, 1]],
    ['no power', 0, [3, 2], [0, 0]],
    ['three blockers', 9, [2, 2, 2], [2, 2, 5]],
  ])(
    'divides %s over a double block: lethal damage in turn, the rest on the last',
    (_name, power, toughness, shares) => {
      const blockers = [X, Y, Z].slice(0, toughness.length)
      const stats: Record<number, [number, number]> = { [A]: [power, 9] }
      blockers.forEach((id, index) => (stats[id] = [1, toughness[index] ?? 1]))
      const forecast = devForecast(table(stats, [{ object: A, target: at0, blocked: true, blockers }]))
      expect(forecast.attackers[0]?.deals.map((hit) => hit.amount)).toEqual(shares)
      expect(forecast.attackers[0]?.deals.map((hit) => hit.target)).toEqual(
        blockers.map((id) => ({ Object: id })),
      )
      expect(forecast.attackers[0]?.receives).toHaveLength(blockers.length)
    },
  )

  it('counts damage already marked: a blocker with 1 damage marked dies to the rest of its toughness', () => {
    const view = table({ [A]: [2, 5], [X]: [1, 3, 1] }, [
      { object: A, target: at0, blocked: true, blockers: [X] },
    ])
    const forecast = devForecast(view)
    expect(forecast.creatures.find((creature) => creature.object === X)).toEqual({
      object: X,
      damage: 2,
      dies: true,
    })
  })

  it('divides by what is left of each blocker once damage is marked', () => {
    const view = table({ [A]: [3, 9], [X]: [1, 3, 1], [Y]: [1, 3] }, [
      { object: A, target: at0, blocked: true, blockers: [X, Y] },
    ])
    expect(devForecast(view).attackers[0]?.deals.map((hit) => hit.amount)).toEqual([2, 1])
  })

  it('kills a creature with no toughness without any damage', () => {
    const view = table({ [A]: [1, 0] }, [{ object: A, target: at0, blocked: false, blockers: [] }])
    expect(devForecast(view).creatures).toEqual([{ object: A, damage: 0, dies: true }])
  })

  it.each<[string, number, number, number, boolean]>([
    ['leaves the player alive', 20, 6, 14, false],
    ['takes the player to exactly zero', 6, 6, 0, true],
    ['takes the player below zero', 5, 6, -1, true],
    ['does no damage at all', 0, 0, 0, false],
  ])('%s: life %i and damage %i', (_name, life, power, after, lethal) => {
    const view = table({ [A]: [power, 2] }, [{ object: A, target: at0, blocked: false, blockers: [] }], {
      0: life,
    })
    expect(devForecast(view).players).toEqual([{ player: 0, damage: power, life, life_after: after, lethal }])
  })

  it('lists a player whose attackers are all blocked, with no damage and no lethal mark', () => {
    const view = table(
      { [A]: [9, 9], [X]: [1, 1] },
      [{ object: A, target: at0, blocked: true, blockers: [X] }],
      {
        0: 1,
      },
    )
    expect(devForecast(view).players).toEqual([
      { player: 0, damage: 0, life: 1, life_after: 1, lethal: false },
    ])
  })

  it('aims damage at a planeswalker as an object, and lists no player and no creature for it', () => {
    const walker = must(
      buildFixture({ seats: 3 }).view.objects.find(
        (object) =>
          object.zone === 'Battlefield' && object.controller === 2 && !object.types.includes('Creature'),
      ),
    )
    const view = table({ [A]: [3, 3] }, [
      { object: A, target: { Planeswalker: walker.id }, blocked: false, blockers: [] },
    ])
    const forecast = devForecast(view)
    expect(forecast.attackers[0]?.deals).toEqual([{ source: A, target: { Object: walker.id }, amount: 3 }])
    expect(forecast.creatures.map((creature) => creature.object)).toEqual([A])
    expect(forecast.players).toEqual([])
  })

  it('lists several players in seat order, and creatures in id order', () => {
    const view = table({ [A]: [1, 1], [B]: [2, 2], [C]: [3, 3] }, [
      { object: C, target: { Player: 2 }, blocked: false, blockers: [] },
      { object: A, target: at0, blocked: false, blockers: [] },
      { object: B, target: { Player: 2 }, blocked: false, blockers: [] },
    ])
    const forecast = devForecast(view)
    expect(forecast.players.map((player) => [player.player, player.damage])).toEqual([
      [0, 1],
      [2, 5],
    ])
    expect(forecast.creatures.map((creature) => creature.object)).toEqual([A, B, C].sort((a, b) => a - b))
  })

  it('gives a forecast the wire accepts', () => {
    const view = table({ [A]: [4, 4], [X]: [3, 3], [Y]: [2, 2], [B]: [1, 1] }, [
      { object: A, target: at0, blocked: true, blockers: [X, Y] },
      { object: B, target: at0, blocked: false, blockers: [] },
    ])
    expect(isCombatForecast(devForecast(view))).toBe(true)
  })
})

const COUNTS = [2, 3, 4, 8, 12]

describe.each(COUNTS)('forecastScene at %i seats', (seats) => {
  const scene = forecastScene(buildFixture({ seats }))
  const { view } = scene
  const forecast = must(view.combat_forecast)
  const defender = must(view.combat.attackers[0]).target

  it('is a valid table that says which seat defends and that its blocks are made', () => {
    expect(isStateView(view)).toBe(true)
    expect(isCombatForecast(forecast)).toBe(true)
    expect(view.turn.step).toBe('DeclareBlockers')
    expect(view.combat.attackers_declared).toBe(true)
    expect(view.combat.blockers_declared).toEqual(['Player' in defender ? defender.Player : -1])
  })

  it('has five attackers at one defender: one double block, three single blocks and one unblocked', () => {
    const shapes = view.combat.attackers.map((attacker) => attacker.blockers.length)
    expect(shapes).toEqual([2, 1, 1, 1, 0])
    expect(view.combat.attackers.map((attacker) => attacker.blocked)).toEqual([true, true, true, true, false])
    expect(new Set(view.combat.attackers.map((attacker) => JSON.stringify(attacker.target))).size).toBe(1)
  })

  it('has the attackers of the active seat, tapped, and blockers of the defender, untapped', () => {
    const owner = (id: ObjectId) => must(view.objects.find((object) => object.id === id))
    const defending = 'Player' in defender ? defender.Player : -1
    for (const attacker of view.combat.attackers) {
      expect(owner(attacker.object)).toMatchObject({
        controller: view.turn.active,
        permanent: { tapped: true },
      })
      for (const blocker of attacker.blockers) {
        expect(owner(blocker)).toMatchObject({ controller: defending, permanent: { tapped: false } })
      }
    }
    expect(defending).not.toBe(view.turn.active)
  })

  it('shows every outcome of a block, a double block and an unblocked attacker', () => {
    const outcomes = view.combat.attackers.flatMap((attacker) =>
      attacker.blockers.map((blocker) => blockOutcome(forecast, attacker.object, blocker)),
    )
    expect(outcomes).toEqual(['both-die', 'attacker-dies', 'blocker-dies', 'both-survive', 'attacker-dies'])
    expect(new Set(outcomes)).toEqual(new Set(['attacker-dies', 'blocker-dies', 'both-die', 'both-survive']))
    expect(forecast.attackers.filter((attacker) => !attacker.blocked)).toHaveLength(1)
  })

  it('puts the defender on lethal damage from the attacker that was not blocked', () => {
    const defending = 'Player' in defender ? defender.Player : -1
    expect(forecast.players).toEqual([
      { player: defending, damage: 6, life: 5, life_after: -1, lethal: true },
    ])
    expect(view.players[defending]?.life).toBe(5)
  })

  it('is what the dev forecast says of its own table', () => {
    expect(devForecast(view)).toEqual(forecast)
  })

  it('leaves everything else of the fixture alone: same seats, objects and log', () => {
    const plain = buildFixture({ seats })
    expect(scene.view.players).toHaveLength(plain.view.players.length)
    expect(scene.view.objects.map((object) => object.id)).toEqual(
      plain.view.objects.map((object) => object.id),
    )
    expect(scene.log).toEqual(plain.log)
  })
})

describe('forecastScene from the attacker side', () => {
  it('has the viewer attack the next seat when it is the viewer turn', () => {
    const game = fixtureGame({ seats: 3, prompt: 'forecast', turn: 'you' })
    const view = must(game.session.store.getState().view)
    expect(view.turn.active).toBe(0)
    const [first] = view.combat.attackers
    expect(first?.target).toEqual({ Player: 1 })
    for (const attacker of view.combat.attackers) {
      expect(must(view.objects.find((object) => object.id === attacker.object)).controller).toBe(0)
    }
    expect(must(view.combat_forecast).players[0]).toMatchObject({ player: 1, lethal: true })
  })
})

describe.each(COUNTS)('withBlockForecast at %i seats (prompt=block)', (seats) => {
  const game = fixtureGame({ seats, prompt: 'block' })
  const view = must(game.session.store.getState().view)
  const forecast = must(view.combat_forecast)

  it('is a valid table carrying a forecast that agrees with its cards', () => {
    expect(isStateView(view)).toBe(true)
    expect(devForecast(view)).toEqual(forecast)
  })

  it('leaves the viewer on lethal damage from the attackers aimed at them, none of which is blocked', () => {
    const aimed = view.combat.attackers.filter(
      (attacker) => 'Player' in attacker.target && attacker.target.Player === view.viewer,
    )
    expect(aimed.length).toBe(seats === 2 ? 6 : 4)
    for (const attacker of aimed) expect(attacker).toMatchObject({ blocked: false, blockers: [] })
    const mine = must(forecast.players.find((player) => player.player === view.viewer))
    expect(mine.lethal).toBe(true)
    expect(mine.life).toBeGreaterThanOrEqual(1)
    expect(mine.life_after).toBeLessThanOrEqual(0)
  })

  it('has another defender double-block one attacker when there is a third seat, and no block otherwise', () => {
    const blocked = view.combat.attackers.filter((attacker) => attacker.blocked)
    if (seats === 2) {
      expect(blocked).toEqual([])
      expect(view.combat.blockers_declared).toEqual([])
      return
    }
    expect(blocked).toHaveLength(1)
    const [only] = blocked
    expect(only?.blockers).toHaveLength(2)
    expect(only?.target).not.toEqual({ Player: view.viewer })
    expect(view.combat.blockers_declared).toHaveLength(1)
    const sides = new Set(
      (only?.blockers ?? []).map((id) => must(view.objects.find((object) => object.id === id)).controller),
    )
    expect([...sides]).toEqual(view.combat.blockers_declared)
    expect(forecast.attackers.find((attacker) => attacker.attacker === only?.object)?.blockers).toEqual(
      only?.blockers,
    )
  })
})
