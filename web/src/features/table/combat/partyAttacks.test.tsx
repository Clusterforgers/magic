// Goal: a party's shared declaration shows every hero's attackers, not only the first hero's: after the declared batch each attacker of each hero is told "Attacking" and has its line, the batches of the first hero's choice and of the party's declaration fold into a view that lists every attacker once, and while a hero still chooses the creatures the teammates chose stand as chosen (dashed) beside their own choice without joining their answer.
import { within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { combatRoleOf } from '@/features/table/cardModel'
import type { Fixture } from '@/features/table/dev/fixture'
import { withBoss } from '@/features/table/dev/devGame'
import { applyBatch } from '@/play/view'
import type {
  AttackTarget,
  CombatView,
  EventBatch,
  ObjectId,
  ObjectView,
  PlayerId,
} from '@/protocol/generated'
import { batchOf, must, tableOf } from '@/test-utils/batches'
import { creaturesOf, fixtureOf, renderCombat } from '@/test-utils/combatTable'
import { renderForecastTable } from '@/test-utils/forecastTable'

/** The creatures `seat` can attack with, in id order. */
const fighters = (fixture: Fixture, seat: PlayerId): ObjectView[] =>
  fixture.view.objects.filter(
    (object) =>
      object.zone === 'Battlefield' &&
      object.controller === seat &&
      object.types.includes('Creature') &&
      object.permanent?.summoning_sick === false,
  )

/** The combat of a party's declaration: every hero attacks the boss with the first `each` of their creatures. */
function declaration(fixture: Fixture, heroes: number, each: number): CombatView {
  const chosen = Array.from({ length: heroes }, (_, seat) => fighters(fixture, seat).slice(0, each)).flat()
  return {
    attackers: chosen.map((object) => ({
      object: object.id,
      target: { Player: heroes },
      blocked: false,
      blockers: [],
    })),
    attackers_declared: true,
    blockers_declared: [],
  }
}

const NO_COMBAT: CombatView = { attackers: [], attackers_declared: false, blockers_declared: [] }

/** The fixture before any attack is declared. */
const quiet = (fixture: Fixture): Fixture => ({ ...fixture, view: { ...fixture.view, combat: NO_COMBAT } })

/** The fixture with `count` more ready creatures on the battlefield of `seat`, copies of its first creature. */
function withCrowd(fixture: Fixture, seat: PlayerId, count: number): Fixture {
  const model = must(fighters(fixture, seat)[0])
  const first = Math.max(...fixture.view.objects.map((object) => object.id)) + 1
  const added = Array.from({ length: count }, (_, at): ObjectView => ({ ...model, id: first + at }))
  const cards = [...(fixture.view.battlefield.cards ?? []), ...added.map((object) => object.id)]
  return {
    ...fixture,
    view: {
      ...fixture.view,
      objects: [...fixture.view.objects, ...added].sort((a, b) => a.id - b.id),
      battlefield: { count: cards.length, cards },
    },
  }
}

describe('a party that attacks together', () => {
  it('a crowd of one hero beside a single attacker of the other: all of them are "Attacking", the crowd is one line', () => {
    let combat = NO_COMBAT
    const table = renderForecastTable({
      seats: 3,
      scene: (plain) => {
        const fixture = withCrowd(withBoss(plain, 0), 1, 19)
        combat = declaration(fixture, 2, 20)
        const turn = { ...fixture.view.turn, active: 0, step: 'DeclareAttackers' as const }
        return { ...fixture, view: { ...fixture.view, combat, turn } }
      },
      layout: { attackBadges: true },
    })
    const ids = combat.attackers.map((attacker) => attacker.object)
    expect(ids.length).toBeGreaterThan(20)
    for (const id of ids) {
      expect(within(table.card(id)).getByText('Attacking'), `card ${id}`).toBeInTheDocument()
    }
    const attacks = table.lines().filter((line) => line.kind === 'attack')
    expect(attacks.reduce((sum, line) => sum + line.count, 0)).toBe(ids.length)
    table.unmount()
  })

  it.each([
    [2, 1],
    [2, 3],
    [3, 2],
  ])('%i heroes with %i attackers each: every attacker is "Attacking" and has its line', (heroes, each) => {
    let combat = NO_COMBAT
    const table = renderForecastTable({
      seats: heroes + 1,
      scene: (plain) => {
        const fixture = withBoss(plain, 0)
        combat = declaration(fixture, heroes, each)
        const turn = { ...fixture.view.turn, active: 0, step: 'DeclareAttackers' as const }
        return { ...fixture, view: { ...fixture.view, combat, turn } }
      },
      layout: { attackBadges: true },
    })
    const ids = combat.attackers.map((attacker) => attacker.object)
    expect(ids).toHaveLength(heroes * each)
    for (const id of ids) {
      expect(within(table.card(id)).getByText('Attacking'), `card ${id}`).toBeInTheDocument()
    }
    const drawn = new Set(
      table
        .lines()
        .filter((line) => line.kind === 'attack')
        .map((line) => line.from),
    )
    for (const id of ids) expect(drawn, `line from ${id}`).toContain(`card:${id}`)
    table.unmount()
  })

  it("folds the first hero's choice and then the declaration into a view with every attacker", () => {
    const fixture = quiet(fixtureOf(3))
    const first = fighters(fixture, 0).slice(0, 1)
    const second = fighters(fixture, 1).slice(0, 4)
    const target: AttackTarget = { Player: 2 }
    const declared: CombatView = {
      attackers: [...first, ...second].map((object) => ({
        object: object.id,
        target,
        blocked: false,
        blockers: [],
      })),
      attackers_declared: true,
      blockers_declared: [],
    }
    // The first hero's `AttackersChosen` leaves the table's combat as it was; the declaration then lists every hero's.
    const chosen: EventBatch = batchOf(fixture.view, {
      from_seq: 10,
      events: [{ Public: { AttackersChosen: { player: 0, attackers: [[first[0]?.id ?? 0, target]] } } }],
    })
    const declaration: EventBatch = batchOf(fixture.view, {
      from_seq: 11,
      events: [
        {
          Public: {
            AttackersDeclared: {
              player: 0,
              attackers: declared.attackers.map((attacker): [ObjectId, AttackTarget] => [
                attacker.object,
                attacker.target,
              ]),
            },
          },
        },
      ],
      table: { ...tableOf(fixture.view), combat: declared },
    })
    let view = applyBatch(fixture.view, chosen)
    expect(view.combat.attackers).toEqual([])
    expect(view.combat.attackers_declared).toBe(false)
    view = applyBatch(view, declaration)
    expect(view.combat.attackers_declared).toBe(true)
    for (const object of [...first, ...second]) {
      expect(combatRoleOf(view.combat, object.id), `object ${object.id}`).toBe('attacking')
    }
    expect(view.combat.attackers.map((attacker) => attacker.object)).toEqual(
      [...first, ...second].map((object) => object.id),
    )
  })
})

describe('a hero who chooses after a teammate', () => {
  const SEATS = 3
  const BOSS: AttackTarget = { Player: 2 }

  /** Seat 0's attack prompt after seat 1 chose `chosen`, with seat 0's own `mine` offered. */
  function chooser(mine: number, chosen: number) {
    const fixture = quiet(fixtureOf(SEATS))
    const own = creaturesOf(fixture, 0).slice(0, mine)
    const teammates = creaturesOf(fixture, 1).slice(0, chosen)
    const table = renderCombat(
      {
        DeclareAttackers: {
          attackers: own.map((object) => ({ object, defenders: [BOSS] })),
          party: {
            declared: teammates.map((object): [PlayerId, ObjectId, AttackTarget] => [1, object, BOSS]),
          },
        },
      },
      { seats: SEATS, fixture, options: { attackBadges: true } },
    )
    return { table, own, teammates }
  }

  const badges = (root: HTMLElement) =>
    [...root.querySelectorAll<HTMLElement>('[data-slot="attack-badge"]')].map((badge) => ({
      from: Number(badge.getAttribute('data-from')),
      pending: badge.hasAttribute('data-pending'),
      text: badge.textContent ?? '',
    }))

  it('sees the teammates choice as chosen on the boss, before any choice of their own', () => {
    const { table, teammates } = chooser(2, 3)
    expect(teammates).toHaveLength(3)
    expect(badges(table.container)).toEqual([{ from: 1, pending: true, text: expect.stringContaining('3') }])
  })

  it('sees their own picks beside the teammates and answers with their own only', async () => {
    const { table, own } = chooser(2, 2)
    const [first] = own
    await table.user.click(table.card(must(first)))
    expect(badges(table.container).map(({ from, pending }) => ({ from, pending }))).toEqual([
      { from: 1, pending: true },
      { from: 0, pending: true },
    ])
    await table.user.click(table.button('Confirm attack'))
    expect(table.answer.mock.calls).toEqual([[{ DeclareAttackers: [[first, BOSS]] }]])
  })
})
