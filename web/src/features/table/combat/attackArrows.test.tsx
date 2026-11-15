// Goal: every layout draws an arrow from each attacker to the player or planeswalker it attacks (a duel, a grid of three to six seats, focus, a team game of 2v2 or Two-Headed Giant, a phone), together with the badge on the defender's pod or tile and its attacked mark; the arrow is strong where the viewer or a teammate attacks or is attacked and faint between two other seats, an attack of the viewer's side at both opponents of a 2v2 reaches both, and the arrows of a spell or ability at a player and a permanent are drawn in every layout as well.
import { describe, expect, it } from 'vitest'
import { withTeams } from '@/features/table/dev/endScene'
import { sidesAttackScene, SIDES_ATTACKERS } from '@/features/table/dev/combatScene'
import type { Fixture } from '@/features/table/dev/fixture'
import type { DevTeams } from '@/features/table/devSearch'
import type { TableOptions } from '@/features/table/tableOptions'
import type { ObjectId, PlayerId } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { renderForecastTable } from '@/test-utils/forecastTable'
import { selectorsOf } from './rects'

const PHONE = { width: 390, height: 844 }
const DESKTOP = { width: 1400, height: 900 }

/** The pod or tile that stands for a seat on the table. */
const placeOf = (root: HTMLElement, seat: PlayerId): HTMLElement =>
  must(
    selectorsOf({ kind: 'seat', seat })
      .map((selector) => root.querySelector<HTMLElement>(selector))
      .find((element) => element !== null && element !== undefined),
    `the pod or tile of seat ${seat}`,
  )

const badgesOf = (place: HTMLElement) => [
  ...place.querySelectorAll<HTMLElement>('[data-slot="attack-badge"]'),
]

/** What a layout is rendered with: seats, the table options and what to make of the plain fixture. */
interface Layout {
  name: string
  seats: number
  options?: TableOptions
  teams?: DevTeams
}

const LAYOUTS: readonly Layout[] = [
  { name: 'a duel', seats: 2 },
  { name: 'three seats', seats: 3 },
  { name: 'four seats', seats: 4 },
  { name: 'six seats', seats: 6 },
  { name: 'a grid of four', seats: 4, options: { mode: 'grid' } },
  { name: 'a focus of four', seats: 4, options: { mode: 'focus', focus: 1 } },
  { name: 'a phone duel', seats: 2, options: { viewport: PHONE } },
  { name: 'a phone of four', seats: 4, options: { viewport: PHONE } },
  { name: 'a 2v2', seats: 4, teams: 'teams' },
  { name: 'Two-Headed Giant', seats: 4, teams: 'two_headed_giant' },
  { name: 'a 2v2 on a phone', seats: 4, teams: 'teams', options: { viewport: PHONE } },
]

const render = (layout: Layout, scene: (fixture: Fixture) => Fixture) =>
  renderForecastTable({
    seats: layout.seats,
    scene: (fixture) => {
      const made = scene(fixture)
      return layout.teams === undefined ? made : withTeams(made, layout.teams)
    },
    layout: { attackBadges: true, viewport: DESKTOP, ...layout.options },
  })

const attackLines = (table: ReturnType<typeof render>) =>
  table.lines().filter((line) => line.kind === 'attack')

describe('an attack at a player', () => {
  it.each(LAYOUTS)(
    '$name: an arrow from each attacker to the defender, its badge and its attacked mark',
    (layout) => {
      // The fixture's own attack: the active seat attacks one defender with three creatures.
      const table = render(layout, (fixture) => fixture)
      const attackers = must(table.store.getState().view).combat.attackers
      expect(attackers.length).toBeGreaterThan(0)
      const defenders = new Set(
        attackers.flatMap((attacker) => ('Player' in attacker.target ? [attacker.target.Player] : [])),
      )
      expect(defenders.size).toBeGreaterThan(0)
      const lines = attackLines(table)
      for (const seat of defenders) {
        const toSeat = lines.filter((line) => line.to === `seat:${seat}`)
        expect(toSeat.length, `an arrow at seat ${seat}`).toBeGreaterThan(0)
        // A line stands for each attacker, or for a crowd of them as one with its count.
        expect(toSeat.reduce((sum, line) => sum + line.count, 0)).toBe(
          attackers.filter((a) => 'Player' in a.target && a.target.Player === seat).length,
        )
        const place = placeOf(table.container, seat)
        expect(place, `mark on seat ${seat}`).toHaveAttribute('data-attacked')
        expect(badgesOf(place).length, `badge on seat ${seat}`).toBeGreaterThan(0)
      }
      for (const line of lines) {
        expect(line.from, 'from an attacker').toMatch(/^card:/)
        expect(line.pending).toBe(false)
      }
      table.unmount()
    },
  )

  it('draws a line at a planeswalker or battle from the attacker as well', () => {
    const table = renderForecastTable({
      seats: 4,
      scene: (fixture) => {
        const walker = must(
          fixture.view.objects.find(
            (object) =>
              object.zone === 'Battlefield' && object.controller === 1 && !object.types.includes('Creature'),
          ),
        )
        const [first] = fixture.view.combat.attackers
        const combat = {
          ...fixture.view.combat,
          attackers: fixture.view.combat.attackers.map((attacker) =>
            attacker === first ? { ...attacker, target: { Planeswalker: walker.id } } : attacker,
          ),
        }
        return { ...fixture, view: { ...fixture.view, combat } }
      },
      layout: { attackBadges: true, viewport: DESKTOP },
    })
    const lines = attackLines(table)
    expect(lines.some((line) => line.to.startsWith('card:'))).toBe(true)
    table.unmount()
  })
})

describe('two sides at war', () => {
  const SIDES = LAYOUTS.filter((layout) => layout.teams !== undefined)

  it.each(SIDES)(
    '$name: the viewer and their teammate each attack an opponent, both arrows strong, both opponents marked',
    (layout) => {
      const table = render(layout, sidesAttackScene)
      const lines = attackLines(table)
      // Seat 0 (the viewer) attacks seat 1 and seat 2 (their teammate) attacks seat 3.
      for (const [from, to] of [
        [0, 1],
        [2, 3],
      ] as const) {
        const own = lines.filter((line) => line.to === `seat:${to}`)
        expect(own, `arrows at seat ${to}`).toHaveLength(SIDES_ATTACKERS)
        expect(own.every((line) => line.from.startsWith('card:'))).toBe(true)
        const place = placeOf(table.container, to)
        expect(place).toHaveAttribute('data-attacked')
        expect(badgesOf(place).map((badge) => Number(badge.getAttribute('data-from')))).toEqual([from])
      }
      // The arrows of the viewer's side are the viewer's own, the teammate's too.
      const strong = table.container.querySelectorAll('[data-layer="lines"] [data-kind="attack"][data-mine]')
      expect(strong).toHaveLength(2 * SIDES_ATTACKERS)
      expect(placeOf(table.container, 0)).not.toHaveAttribute('data-attacked')
      expect(placeOf(table.container, 2)).not.toHaveAttribute('data-attacked')
      table.unmount()
    },
  )

  it('fades the attack of a seat that is no teammate, as it does between two other seats', () => {
    const table = render({ name: 'four seats', seats: 4 }, sidesAttackScene)
    const lines = table.container.querySelectorAll<HTMLElement>('[data-layer="lines"] [data-kind="attack"]')
    const strong = [...lines].filter((line) => line.hasAttribute('data-mine'))
    // Seat 0's own two arrows are strong; seat 2's two arrows at seat 3, between two other seats, are not.
    expect(lines).toHaveLength(2 * SIDES_ATTACKERS)
    expect(strong.map((line) => line.getAttribute('data-to'))).toEqual(
      Array.from({ length: SIDES_ATTACKERS }, () => 'seat:1'),
    )
    table.unmount()
  })

  it('folds a crowd of one seat at one defender into one arrow with its count', () => {
    const table = render({ name: 'a 2v2', seats: 4, teams: 'teams' }, (fixture) => {
      const scene = sidesAttackScene(fixture)
      // Seat 0's creatures, copied until seven attack seat 1.
      const model = must(
        scene.view.objects.find(
          (object) =>
            object.zone === 'Battlefield' && object.controller === 0 && object.types.includes('Creature'),
        ),
      )
      const first = Math.max(...scene.view.objects.map((object) => object.id)) + 1
      const added = Array.from({ length: 5 }, (_, at) => ({ ...model, id: first + at }))
      const cards = [...(scene.view.battlefield.cards ?? []), ...added.map((object) => object.id)]
      const combat = {
        ...scene.view.combat,
        attackers: [
          ...scene.view.combat.attackers,
          ...added.map((object) => ({
            object: object.id,
            target: { Player: 1 },
            blocked: false,
            blockers: [],
          })),
        ],
      }
      return {
        ...scene,
        view: {
          ...scene.view,
          objects: [...scene.view.objects, ...added].sort((a, b) => a.id - b.id),
          battlefield: { count: cards.length, cards },
          combat,
        },
      }
    })
    const atOne = attackLines(table).filter((line) => line.to === 'seat:1')
    expect(atOne).toHaveLength(1)
    expect(atOne[0]?.count).toBe(SIDES_ATTACKERS + 5)
    // The other side of the war is untouched: its arrows stay one each.
    expect(attackLines(table).filter((line) => line.to === 'seat:3')).toHaveLength(SIDES_ATTACKERS)
    table.unmount()
  })
})

describe('a spell or ability at a player and a permanent', () => {
  /** The fixture with the top of its stack aimed at seat 1 and at a creature of seat 1. */
  const aimed = (fixture: Fixture): { fixture: Fixture; top: ObjectId; creature: ObjectId } => {
    const top = must(fixture.view.stack.cards?.at(-1))
    const creature = must(
      fixture.view.objects.find(
        (object) =>
          object.zone === 'Battlefield' && object.controller === 1 && object.types.includes('Creature'),
      ),
    ).id
    const objects = fixture.view.objects.map((object) =>
      object.id === top
        ? {
            ...object,
            targets: [[{ Player: 1 }, { Object: { id: creature, counter: 0 } }]],
            targets_legal: [[true, true]],
          }
        : object,
    )
    return { fixture: { ...fixture, view: { ...fixture.view, objects } }, top, creature }
  }

  it.each(LAYOUTS)('$name: one arrow to the player and one to the permanent', (layout) => {
    let made = { top: 0, creature: 0 }
    const table = render(layout, (fixture) => {
      const scene = aimed(fixture)
      made = scene
      return scene.fixture
    })
    // The arrows of the stack are drawn in a layer of their own, above the strip their chips stand in.
    const lines = [
      ...table.container.querySelectorAll<SVGGElement>('[data-layer="stack"] g[data-kind="target"]'),
    ].map((line) => ({ from: line.getAttribute('data-from'), to: must(line.getAttribute('data-to')) }))
    expect(lines.map((line) => line.to).sort()).toEqual([`card:${made.creature}`, 'seat:1'].sort())
    expect(lines.every((line) => line.from === `stack:${made.top}`)).toBe(true)
    table.unmount()
  })
})
