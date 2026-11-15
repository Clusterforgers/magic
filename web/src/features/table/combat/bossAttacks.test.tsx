// Goal: whoever an attack is aimed at can see it in every layout: in boss mode (boss's side above, party below, tiles on a phone) the boss's attack at each hero, the viewer included, is a line to that hero's pod or tile as well as a badge, strong because it concerns the party; outside boss mode (a grid, or the boss table with its layout changed) an attack at a player is a badge alone; and in each the attacked seat, the viewer's own pod too, wears the attacked mark and its badge says "you".
import { describe, expect, it } from 'vitest'
import { must } from '@/test-utils/batches'
import { renderForecastTable } from '@/test-utils/forecastTable'
import type { TableOptions } from '../tableOptions'
import { withBoss, withBossAttack } from '../dev/devGame'
import { buildFixture } from '../dev/fixture'

const PHONE = { width: 390, height: 844 }

/** A boss table of `heroes` heroes (the viewer seat 0 first), the boss and `minions` minions, the boss's side attacking. */
function bossTable(heroes: number, minions: number, layout: TableOptions = {}) {
  return renderForecastTable({
    seats: heroes + 1 + minions,
    scene: (fixture) => withBossAttack(withBoss(fixture, minions)),
    layout: { attackBadges: true, ...layout },
  })
}

/** The pod or tile that stands for a seat on the table. */
const placeOf = (root: HTMLElement, seat: number): HTMLElement =>
  must(root.querySelector<HTMLElement>(`[data-seat="${seat}"], [data-tile-seat="${seat}"]`), `seat ${seat}`)

const attackLines = (root: HTMLElement): HTMLElement[] =>
  [...root.querySelectorAll<HTMLElement>('[data-layer="lines"] [data-kind="attack"]')].filter(
    (line) => line.getAttribute('data-to')?.startsWith('seat:') === true,
  )

describe('withBossAttack', () => {
  it.each([
    [3, 0],
    [3, 2],
    [1, 1],
  ])('has the boss side attack every one of %i heroes with %i minions at the start', (heroes, minions) => {
    const { view } = withBossAttack(withBoss(buildFixture({ seats: heroes + 1 + minions }), minions))
    const targets = view.combat.attackers.map((attacker) =>
      'Player' in attacker.target ? attacker.target.Player : -1,
    )
    expect(new Set(targets)).toEqual(new Set(Array.from({ length: heroes }, (_, seat) => seat)))
    expect(view.turn.active).toBe(heroes)
  })
})

describe('an attack at the party in boss mode', () => {
  it.each([
    ['desktop', {}, 3, 0],
    ['desktop with minions', {}, 3, 2],
    ['phone', { viewport: PHONE }, 3, 0],
    ['phone with minions', { viewport: PHONE }, 3, 2],
  ] as const)(
    '%s: a strong line, a badge and the attacked mark at each hero, the viewer too',
    (_, layout, heroes, minions) => {
      const table = bossTable(heroes, minions, layout)
      const root = table.container
      const lines = attackLines(root)
      for (let seat = 0; seat < heroes; seat += 1) {
        const toHero = lines.filter((line) => line.getAttribute('data-to') === `seat:${seat}`)
        expect(toHero.length, `lines at seat ${seat}`).toBeGreaterThan(0)
        for (const line of toHero) expect(line).toHaveAttribute('data-mine')
        const place = placeOf(root, seat)
        expect(place, `mark on seat ${seat}`).toHaveAttribute('data-attacked')
        expect(
          place.querySelectorAll('[data-slot="attack-badge"]').length,
          `badge on seat ${seat}`,
        ).toBeGreaterThan(0)
      }
      // Nothing attacks the boss, and a seat that nothing attacks is not marked.
      expect(lines.some((line) => line.getAttribute('data-to') === `seat:${heroes}`)).toBe(false)
      expect(placeOf(root, heroes)).not.toHaveAttribute('data-attacked')
    },
  )

  it("names the viewer in the badge on the viewer's own pod and the attacker only on the others'", () => {
    const table = bossTable(3, 0)
    const own = must(
      placeOf(table.container, 0).querySelector('[data-slot="attack-badge"]'),
      'the viewer badge',
    )
    expect(own.getAttribute('aria-label')).toMatch(/ attacks you with \d+ creatures?: /)
    const other = must(
      placeOf(table.container, 1).querySelector('[data-slot="attack-badge"]'),
      'the ally badge',
    )
    expect(other.getAttribute('aria-label')).toMatch(/ attacks with \d+ creatures?: /)
  })

  it('leaves the badges alone when the layout is changed to a grid or focus, whose boards a line would cross', () => {
    for (const mode of ['grid', 'focus'] as const) {
      const table = bossTable(3, 0, { mode })
      expect(attackLines(table.container), mode).toHaveLength(0)
      for (let seat = 0; seat < 3; seat += 1) {
        const place = placeOf(table.container, seat)
        expect(place, `${mode} mark on seat ${seat}`).toHaveAttribute('data-attacked')
        expect(
          place.querySelector('[data-slot="attack-badge"]'),
          `${mode} badge on seat ${seat}`,
        ).not.toBeNull()
      }
      table.unmount()
    }
  })
})

describe('an attack at the viewer on a table without a boss', () => {
  it.each([
    ['a grid', { attackBadges: true }],
    ['a phone', { attackBadges: true, viewport: PHONE }],
  ])("%s: the badge on the viewer's own pod, its mark and no line", (_, layout) => {
    const table = renderForecastTable({ seats: 4, scene: (fixture) => fixture, layout })
    const own = placeOf(table.container, 0)
    expect(own).toHaveAttribute('data-attacked')
    const badge = must(own.querySelector('[data-slot="attack-badge"]'), 'the viewer badge')
    expect(badge.getAttribute('aria-label')).toMatch(/ attacks you with 3 creatures: /)
    expect(attackLines(table.container)).toHaveLength(0)
    // The attacker is not under attack.
    expect(placeOf(table.container, 1)).not.toHaveAttribute('data-attacked')
  })
})
