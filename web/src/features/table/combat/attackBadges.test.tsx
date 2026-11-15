// Goal: an attack at a player is drawn as a badge on the defender's pod per attacking seat (declared and still being chosen apart, the count of creatures, their names for a screen reader) instead of a line from a board to a pod, while the lines that join two boards (a block, an attack at a planeswalker) stay.
import { describe, expect, it } from 'vitest'
import type { AttackTarget, CombatView, ObjectId, PlayerId } from '@/protocol/generated'
import { NO_PENDING } from './pending'
import type { PendingCombat } from './pending'
import { attackGroups, planLines } from './plan'
import { attackWords } from './AttackBadges'
import { renderTable } from '@/test-utils/tableHarness'
import { must } from '@/test-utils/batches'

const OWNERS: Record<number, PlayerId> = { 10: 1, 11: 1, 12: 2, 20: 0, 30: 3 }
const controllerOf = (id: ObjectId): PlayerId | undefined => OWNERS[id]
const player = (seat: PlayerId): AttackTarget => ({ Player: seat })

function combat(attacks: [ObjectId, AttackTarget, ObjectId[]][]): CombatView {
  return {
    attackers: attacks.map(([object, target, blockers]) => ({ object, target, blockers })),
    attackers_declared: true,
    blockers_declared: [],
  } as unknown as CombatView
}

const pendingOf = (attacks: [ObjectId, AttackTarget][]): PendingCombat => ({
  ...NO_PENDING,
  attacks: new Map(attacks),
})

describe('attackGroups', () => {
  it.each<
    [string, CombatView | null, PendingCombat, Record<number, [PlayerId | undefined, number, boolean][]>]
  >([
    ['no combat', null, NO_PENDING, {}],
    [
      'one seat at one player',
      combat([
        [10, player(0), []],
        [11, player(0), []],
      ]),
      NO_PENDING,
      { 0: [[1, 2, false]] },
    ],
    [
      'two seats at one player',
      combat([
        [10, player(0), []],
        [12, player(0), []],
      ]),
      NO_PENDING,
      {
        0: [
          [1, 1, false],
          [2, 1, false],
        ],
      },
    ],
    [
      'one seat at two players',
      combat([
        [10, player(0), []],
        [11, player(3), []],
      ]),
      NO_PENDING,
      { 0: [[1, 1, false]], 3: [[1, 1, false]] },
    ],
    ['an attack at a planeswalker', combat([[10, { Planeswalker: 30 }, []]]), NO_PENDING, {}],
    ['a choice not sent yet', null, pendingOf([[20, player(1)]]), { 1: [[0, 1, true]] }],
  ])('groups %s', (_, view, pending, expected) => {
    const groups = attackGroups(view, pending, controllerOf)
    const shape = Object.fromEntries(
      [...groups].map(([seat, list]) => [
        seat,
        list.map((group) => [group.from, group.attackers.length, group.pending]),
      ]),
    )
    expect(shape).toEqual(expected)
  })
})

describe('planLines with seat badges', () => {
  const view = combat([
    [10, player(0), [20]],
    [11, { Planeswalker: 30 }, []],
  ])

  it('leaves out the attacks at a player and keeps the block and the attack at a planeswalker', () => {
    const lines = planLines({
      combat: view,
      forecast: null,
      pending: NO_PENDING,
      controllerOf,
      seatBadges: true,
    })
    expect(lines.map((line) => line.key)).toEqual(['attack:11', 'block:20'])
  })

  it('leaves out a pending attack at a player too', () => {
    const lines = planLines({
      combat: null,
      forecast: null,
      pending: pendingOf([
        [20, player(1)],
        [10, { Planeswalker: 30 }],
      ]),
      controllerOf,
      seatBadges: true,
    })
    expect(lines.map((line) => line.key)).toEqual(['pending-attack:10'])
  })

  it("draws the attacks at a party's seats strong, as the viewer's own, only for a viewer with a party", () => {
    // Seat 2 (the attacker of 12) attacks seat 0 (the viewer), seat 1's creature 10 attacks seat 3.
    const view = combat([
      [12, player(0), []],
      [10, player(3), []],
    ])
    const mine = (party: readonly PlayerId[] | null, viewer: PlayerId | null) =>
      planLines({ combat: view, forecast: null, pending: NO_PENDING, controllerOf, viewer, party }).map(
        (line) => line.mine === true,
      )
    expect(mine(null, 0)).toEqual([true, false])
    expect(mine([0, 3], 0)).toEqual([true, true])
    expect(mine([0, 1], 0)).toEqual([true, true])
    expect(mine([0, 3], null)).toEqual([false, false])
  })

  it('draws every attack without them', () => {
    const lines = planLines({ combat: view, forecast: null, pending: NO_PENDING, controllerOf })
    expect(lines.map((line) => line.key)).toEqual(['attack:10', 'attack:11', 'block:20'])
  })
})

describe('attackWords', () => {
  it.each([
    [false, ['Grizzly Bears'], 'Bo attacks with 1 creature: Grizzly Bears'],
    [true, ['Grizzly Bears', 'Hill Giant'], 'Bo would attack with 2 creatures: Grizzly Bears, Hill Giant'],
  ])('says a group (pending %s) in words', (pending, names, words) => {
    const group = { from: 1, attackers: names.map((_, place) => place), pending }
    expect(attackWords(group, 'Bo', names)).toBe(words)
  })
})

describe('the badge on the pod', () => {
  it.each([
    [true, 1],
    [false, 0],
  ])('stands on the attacked seat when badges are %s', (attackBadges, count) => {
    const { container, store } = renderTable({ seats: 4, options: { attackBadges } })
    const attacked = new Set(
      (store.getState().view?.combat?.attackers ?? []).flatMap((attacker) =>
        'Player' in attacker.target ? [attacker.target.Player] : [],
      ),
    )
    expect(attacked.size).toBeGreaterThan(0)
    for (const seat of attacked) {
      const pod = must(container.querySelector(`[data-seat="${seat}"]`))
      const badges = pod.querySelectorAll('[data-slot="attack-badge"]')
      expect(badges.length).toBe(count)
      if (count > 0) expect(badges[0]?.getAttribute('aria-label')).toMatch(/attacks (you )?with \d+ creature/)
    }
  })
})
