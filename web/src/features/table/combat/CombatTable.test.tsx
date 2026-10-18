// Goal: over a table whose combat carries the engine's forecast, the overlay, the cards, the seat panels and the legend all tell the same story and tell it in words as well as colours: one muted line per attacker and one strong line per blocker, coloured by what happens to that pair; doomed cards and "-N" badges exactly where the forecast puts them; "life -> life after" and a lethal mark on the attacked seat; a key listing only the kinds of line on screen; sentences for a screen reader; lines that follow the layout when it moves and skip what is not on screen; and all of it gone when combat ends or the forecast does.
import { act, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { CombatForecast, ObjectId, StateView } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { renderForecastTable } from '@/test-utils/forecastTable'
import type { ForecastTable } from '@/test-utils/forecastTable'
import { boxOf, fakeRects } from '@/test-utils/rects'
import { renderTable } from '@/test-utils/tableHarness'
import { forecastScene } from '../dev/combatScene'
import { buildFixture } from '../dev/fixture'
import { RectProviderContext } from './rects'
import type { Rect } from './geometry'
import { GameTable } from '../GameTable'

const NO_COMBAT: StateView['combat'] = { attackers: [], attackers_declared: false, blockers_declared: [] }

/** The first point of a path, which for an arrowhead is its tip. */
const firstPoint = (path: string): { x: number; y: number } => {
  const [x, y] = (path.match(/-?\d+(\.\d+)?/g) ?? []).map(Number)
  return { x: x ?? Number.NaN, y: y ?? Number.NaN }
}

const distanceTo = (rect: Rect, point: { x: number; y: number }): number =>
  Math.hypot(
    Math.max(rect.x - point.x, 0, point.x - (rect.x + rect.width)),
    Math.max(rect.y - point.y, 0, point.y - (rect.y + rect.height)),
  )

const nameOf = (table: ForecastTable, id: ObjectId): string =>
  must(must(table.store.getState().objectsById)[id]).name

/** The scene: Bo attacks Alice (the viewer) with five creatures, as `combatScene.ts` sets them out. */
function scene(seats = 2) {
  const table = renderForecastTable({ seats })
  const view = must(table.store.getState().view)
  const attackers = view.combat.attackers
  const blockers = attackers.flatMap((attacker) => attacker.blockers)
  const forecast = table.forecast()
  return { table, view, attackers, blockers, forecast }
}

describe('the lines over the table', () => {
  it('draws a muted line per attacker to the seat it attacks, then a strong line per blocker to the attacker it blocks', () => {
    const { table, attackers, blockers } = scene()
    const lines = table.lines()
    expect(lines.map((line) => line.key)).toEqual([
      ...attackers.map((attacker) => `attack:${attacker.object}`),
      ...blockers.map((blocker) => `block:${blocker}`),
    ])
    expect(lines.filter((line) => line.kind === 'attack')).toHaveLength(5)
    expect(lines.filter((line) => line.kind === 'block')).toHaveLength(5)
    expect(lines.some((line) => line.pending)).toBe(false)
  })

  it('colours each block by what happens to its two creatures, and leaves the attacks plain', () => {
    const { table, attackers } = scene()
    const outcome = (blocker: ObjectId) =>
      table.lines().find((line) => line.key === `block:${blocker}`)?.outcome
    const [doubled, hit, even, big, free] = attackers
    expect(doubled?.blockers.map(outcome)).toEqual(['both-die', 'attacker-dies'])
    expect(hit?.blockers.map(outcome)).toEqual(['blocker-dies'])
    expect(even?.blockers.map(outcome)).toEqual(['both-survive'])
    expect(big?.blockers.map(outcome)).toEqual(['attacker-dies'])
    expect(free?.blockers).toEqual([])
    expect(
      table
        .lines()
        .filter((line) => line.kind === 'attack')
        .map((line) => line.outcome),
    ).toEqual([null, null, null, null, null])
  })

  it('runs each line between the edges of the boxes it joins, the arrowhead on the end that is attacked or blocked', () => {
    const { table, attackers } = scene()
    const [first] = attackers
    const card = boxOf({ kind: 'card', id: must(first).object })
    const panel = boxOf({ kind: 'seat', seat: 0 })
    const group = must(table.container.querySelector('[data-layer="lines"] g[data-line^="attack:"]'))
    const start = firstPoint(must(group.querySelector('.combat-line')?.getAttribute('d')))
    const tip = firstPoint(must(group.querySelector('.combat-head')?.getAttribute('d')))
    expect(distanceTo(card, start)).toBeGreaterThan(0)
    expect(distanceTo(card, start)).toBeLessThanOrEqual(3.01)
    expect(distanceTo(panel, tip)).toBeGreaterThan(0)
    expect(distanceTo(panel, tip)).toBeLessThanOrEqual(3.01)

    const block = must(table.container.querySelector('[data-layer="lines"] g[data-line^="block:"]'))
    const blocker = must(first?.blockers[0])
    const blockTip = firstPoint(must(block.querySelector('.combat-head')?.getAttribute('d')))
    expect(distanceTo(boxOf({ kind: 'card', id: first?.object ?? 0 }), blockTip)).toBeLessThanOrEqual(3.01)
    expect(distanceTo(boxOf({ kind: 'card', id: blocker }), blockTip)).toBeGreaterThan(3)
  })

  it('is decoration: hidden from assistive technology, never takes the pointer, nothing in it can be focused', () => {
    const { table } = scene()
    const overlay = must(table.overlay())
    expect(overlay).toHaveAttribute('aria-hidden', 'true')
    expect(overlay).toHaveClass('pointer-events-none')
    expect(overlay.querySelectorAll('svg')).toHaveLength(2)
    for (const svg of overlay.querySelectorAll('svg')) expect(svg).toHaveAttribute('focusable', 'false')
    expect(overlay.querySelectorAll('a, button, input, [tabindex]')).toHaveLength(0)
  })

  it('draws the same lines again, faint, above the centre strip, so a line does not vanish behind the prompt', () => {
    const { table } = scene()
    const strong = [...table.container.querySelectorAll('[data-layer="lines"] g')].map((g) => g.outerHTML)
    const faint = [...table.container.querySelectorAll('[data-layer="ghost"] g')].map((g) => g.outerHTML)
    expect(faint).toEqual(strong)
    expect(table.container.querySelector('[data-layer="ghost"]')).toHaveClass('opacity-30')
  })

  it('is inside the table column, which is positioned, with the centre strip stacked above the lines', () => {
    const { table } = scene()
    const overlay = must(table.overlay())
    expect(overlay.parentElement).toHaveClass('relative')
    const centre = must(table.container.querySelector('[data-slot="banner"]')).closest('.z-20')
    expect(centre).not.toBeNull()
    expect(overlay.querySelector('[data-layer="lines"]')).toHaveClass('z-10')
    expect(overlay.querySelector('[data-layer="ghost"]')).toHaveClass('z-30')
  })

  it('draws a line again where a box has moved, and measures once per frame however much happens', () => {
    const shift = { x: 0 }
    const rects = fakeRects((anchor) => {
      const box = boxOf(anchor)
      return { ...box, x: box.x + shift.x }
    })
    const table = renderTable({
      fixture: forecastScene(buildFixture({ seats: 2 })),
      ui: (
        <RectProviderContext value={rects.provider}>
          <GameTable />
        </RectProviderContext>
      ),
    })
    const path = (): string =>
      must(table.container.querySelector('[data-layer="lines"] g .combat-line')?.getAttribute('d'))
    const before = path()
    const calls = rects.measure.mock.calls.length
    shift.x = 40
    act(() => {
      for (let i = 0; i < 8; i += 1) window.dispatchEvent(new Event('resize'))
    })
    expect(rects.waiting()).toBe(1)
    act(() => rects.flush())
    expect(rects.measure.mock.calls.length).toBe(calls + 1)
    const moved = path()
    expect(moved).not.toBe(before)
    // The same boxes again change nothing.
    act(() => {
      window.dispatchEvent(new Event('resize'))
      rects.flush()
    })
    expect(rects.measure.mock.calls.length).toBe(calls + 2)
    expect(path()).toBe(moved)
  })

  it('measures the anchors it draws, once each, and again when the combat gains an attacker', () => {
    const { table, attackers } = scene()
    const first = table
      .lastAnchors()
      .map((anchor) => (anchor.kind === 'seat' ? `seat:${anchor.seat}` : `card:${anchor.id}`))
    expect(new Set(first).size).toBe(first.length)
    expect(first).toContain('seat:0')
    expect(first).toContain(`card:${must(attackers[0]).object}`)
    const calls = table.measure.mock.calls.length
    const spare = must(
      Object.values(table.store.getState().objectsById).find(
        (object) =>
          object.controller === 1 &&
          object.zone === 'Battlefield' &&
          object.types.includes('Creature') &&
          !attackers.some((attacker) => attacker.object === object.id),
      ),
    )
    const combat = must(table.store.getState().view).combat
    table.setView({
      combat: {
        ...combat,
        attackers: [
          ...combat.attackers,
          { object: spare.id, target: { Player: 0 }, blocked: false, blockers: [] },
        ],
      },
    })
    expect(table.measure.mock.calls.length).toBe(calls + 1)
    expect(table.lastAnchors().map((anchor) => (anchor.kind === 'card' ? anchor.id : -1))).toContain(spare.id)
    expect(table.lines().map((line) => line.key)).toContain(`attack:${spare.id}`)
  })

  it('leaves out a line whose end is not on screen, and draws it when it is', () => {
    const { table, attackers } = scene()
    const lost = must(attackers[1]).object
    table.hide({ kind: 'card', id: lost })
    window.dispatchEvent(new Event('resize'))
    act(() => table.flush())
    const keys = table.lines().map((line) => line.key)
    expect(keys).not.toContain(`attack:${lost}`)
    expect(keys.filter((key) => key.startsWith('block:'))).toHaveLength(4)
    expect(keys).toContain(`attack:${must(attackers[0]).object}`)
  })
})

describe('without a forecast', () => {
  it('still draws the declared combat, with blocks that are not coloured, and a key of two kinds', () => {
    const { table } = scene()
    table.setView({ combat_forecast: null })
    expect(table.lines()).toHaveLength(10)
    expect(table.lines().every((line) => line.outcome === null)).toBe(true)
    expect(table.doomed()).toEqual([])
    expect(table.life(0)).toBeNull()
    const key = must(table.container.querySelector('[data-slot="combat-key"]'))
    expect([...key.querySelectorAll('li')].map((item) => item.textContent)).toEqual(['Attack', 'Block'])
    expect(table.container.querySelector('[data-slot="forecast-sentences"]')).toBeNull()
  })
})

describe('the cards', () => {
  it('look doomed exactly where the forecast says a creature dies, and nowhere else', () => {
    const { table, forecast } = scene()
    const dying = forecast.creatures.filter((creature) => creature.dies).map((creature) => creature.object)
    expect(dying.length).toBeGreaterThanOrEqual(4)
    expect(table.doomed().sort((a, b) => a - b)).toEqual(dying.sort((a, b) => a - b))
    for (const id of dying) {
      const card = table.card(id)
      expect(card).toHaveAttribute('data-doomed', 'true')
      expect(card.querySelector('[data-slot="forecast-cracks"]')).not.toBeNull()
      expect(card.querySelector('[data-slot="forecast-doomed"]')).toHaveTextContent('Dies')
    }
    const surviving = forecast.creatures
      .filter((creature) => !creature.dies)
      .map((creature) => creature.object)
    for (const id of surviving) {
      expect(table.card(id)).not.toHaveAttribute('data-doomed')
      expect(table.card(id).querySelector('[data-slot="forecast-cracks"]')).toBeNull()
    }
  })

  it('show a "-N" badge for every creature that would take damage, and only those', () => {
    const { table, forecast } = scene()
    const hurt = forecast.creatures.filter((creature) => creature.damage > 0)
    expect(hurt.length).toBeGreaterThan(4)
    expect(table.damage()).toEqual(new Map(hurt.map((creature) => [creature.object, `-${creature.damage}`])))
  })

  it('say it in words too: a name that ends with what would happen to the creature', () => {
    const { table, forecast } = scene()
    for (const creature of forecast.creatures) {
      const name = table.card(creature.object).getAttribute('aria-label') ?? ''
      if (creature.damage > 0 && creature.dies)
        expect(name).toMatch(new RegExp(`, would take ${creature.damage} damage and die$`))
      else if (creature.damage > 0)
        expect(name).toMatch(new RegExp(`, would take ${creature.damage} damage$`))
      else expect(name).not.toMatch(/would/)
    }
  })

  it('say nothing for a creature the forecast does not list', () => {
    const { table, forecast } = scene()
    const listed = new Set(forecast.creatures.map((creature) => creature.object))
    const other = must(
      [...table.container.querySelectorAll<HTMLElement>('button[data-object-id]')].find(
        (card) =>
          !listed.has(Number(card.getAttribute('data-object-id'))) &&
          card.closest('[aria-label$="creatures"]'),
      ),
    )
    expect(other).not.toHaveAttribute('data-doomed')
    expect(other.querySelector('[data-slot="forecast"]')).toBeNull()
    expect(other.getAttribute('aria-label')).not.toMatch(/would/)
  })

  it('keep a tapped attacker readable: the marks sit on the card as seen, not on its square button', () => {
    const { table, attackers } = scene()
    const attacker = table.card(must(attackers[0]).object)
    expect(attacker).toHaveAttribute('data-tapped', 'true')
    const marks = must(attacker.querySelector('[data-slot="forecast"]'))
    expect(marks.className).toContain('h-(--card-w)')
    const untapped = table.card(must(must(attackers[0]).blockers[0]))
    expect(untapped).not.toHaveAttribute('data-tapped')
    expect(untapped.querySelector('[data-slot="forecast"]')?.className).not.toContain('h-(--card-w)')
  })

  it('go when the forecast does', () => {
    const { table } = scene()
    table.setView({ combat_forecast: null })
    expect(table.doomed()).toEqual([])
    expect(table.damage().size).toBe(0)
    expect(table.container.querySelectorAll('[data-slot="forecast"]')).toHaveLength(0)
  })
})

describe('the seat panels', () => {
  it('show the attacked seat life as "life -> life after" with a lethal mark when it would end the game', () => {
    const { table } = scene()
    const panel = must(table.life(0))
    expect(panel).toHaveAttribute('data-lethal', 'true')
    expect(panel).toHaveTextContent('5→-1')
    expect(panel).toHaveTextContent('Lethal')
    expect(within(panel).getByText('Life would go from 5 to -1, which is lethal')).toHaveClass('sr-only')
  })

  it('show nothing on a seat nobody attacks', () => {
    const { table } = scene(3)
    expect(table.life(0)).not.toBeNull()
    expect(table.life(1)).toBeNull()
    expect(table.life(2)).toBeNull()
  })

  it('show damage that is survived without a lethal mark, and an attack that was all blocked as no damage', () => {
    const { table, forecast } = scene()
    const survived: CombatForecast = {
      ...forecast,
      players: [{ player: 0, damage: 4, life: 20, life_after: 16, lethal: false }],
    }
    table.setView({ combat_forecast: survived })
    const panel = must(table.life(0))
    expect(panel).toHaveTextContent('20→16')
    expect(panel).not.toHaveAttribute('data-lethal')
    expect(panel).not.toHaveTextContent('Lethal')
    table.setView({
      combat_forecast: {
        ...forecast,
        players: [{ player: 0, damage: 0, life: 20, life_after: 20, lethal: false }],
      },
    })
    expect(must(table.life(0))).toHaveTextContent('No combat damage')
    expect(screen.getByText('Takes no combat damage')).toHaveClass('sr-only')
  })

  it('go when the forecast does', () => {
    const { table } = scene()
    table.setView({ combat_forecast: null })
    expect(table.life(0)).toBeNull()
  })
})

describe('the key and the sentences', () => {
  it('list every kind of line on screen once, with its words, and no other', () => {
    const { table } = scene()
    const key = must(table.container.querySelector('[data-slot="combat-key"]'))
    expect(key).toHaveAttribute('aria-hidden', 'true')
    expect([...key.querySelectorAll('li')].map((item) => item.textContent)).toEqual([
      'Attack',
      'Attacker dies',
      'Blocker dies',
      'Both die',
      'Both survive',
    ])
    for (const swatch of key.querySelectorAll('li svg')) expect(swatch).toHaveClass('combat-group')
  })

  it('list only the outcomes that occur', () => {
    const { table, attackers } = scene()
    const [doubled] = attackers
    const view = must(table.store.getState().view)
    table.setView({ combat: { ...view.combat, attackers: [must(doubled)] } })
    const key = must(table.container.querySelector('[data-slot="combat-key"]'))
    expect([...key.querySelectorAll('li')].map((item) => item.textContent)).toEqual([
      'Attack',
      'Attacker dies',
      'Both die',
    ])
  })

  it('tell a screen reader what the forecast says, the seat first and then each attacker, with the names of the cards', () => {
    const { table, attackers, view } = scene()
    const list = screen.getByRole('list', { name: 'Combat forecast' })
    const sentences = within(list)
      .getAllByRole('listitem')
      .map((item) => item.textContent)
    expect(sentences).toHaveLength(6)
    expect(sentences[0]).toBe('You would take 6 damage, from 5 to -1 life, which is lethal.')
    const [doubled] = attackers
    const [first, second] = must(doubled).blockers
    const bo = (id: ObjectId) => `Bo's ${nameOf(table, id)}`
    const mine = (id: ObjectId) => `your ${nameOf(table, id)}`
    const attacker = must(doubled).object
    expect(sentences[1]).toBe(
      `${bo(attacker)} is blocked by ${mine(must(first))} and ${mine(must(second))}: ${bo(attacker)} would die, ${mine(must(first))} would die, ${mine(must(second))} would survive.`,
    )
    const free = must(attackers[4]).object
    expect(sentences[5]).toBe(`${bo(free)} is unblocked and would deal 6 damage to you.`)
    expect(view.players[0]?.life).toBe(5)
    expect(list).toHaveClass('sr-only')
  })

  it('go with the forecast: no key line for outcomes and no sentences', () => {
    const { table } = scene()
    table.setView({ combat_forecast: null })
    expect(screen.queryByRole('list', { name: 'Combat forecast' })).toBeNull()
  })
})

describe('when combat ends', () => {
  it('takes away the lines, the marks, the life forecast, the key and the sentences, and brings them back with the next combat', () => {
    const { table, view, forecast } = scene()
    expect(table.overlay()).not.toBeNull()
    table.setView({ combat: NO_COMBAT, combat_forecast: null })
    expect(table.overlay()).toBeNull()
    expect(table.container.querySelector('[data-layer]')).toBeNull()
    expect(table.doomed()).toEqual([])
    expect(table.damage().size).toBe(0)
    expect(table.life(0)).toBeNull()
    expect(table.container.querySelector('[data-slot="combat-legend"]')).toBeNull()
    expect(table.container.querySelector('[data-slot="combat-overlay"]')).toBeNull()
    table.setView({ combat: view.combat, combat_forecast: forecast })
    expect(table.overlay()).not.toBeNull()
    expect(table.lines()).toHaveLength(10)
    expect(table.doomed().length).toBeGreaterThan(0)
  })

  it('stops measuring too: nothing is listening once the overlay is gone', () => {
    const { table } = scene()
    table.setView({ combat: NO_COMBAT, combat_forecast: null })
    const calls = table.measure.mock.calls.length
    window.dispatchEvent(new Event('resize'))
    expect(table.waiting()).toBe(0)
    act(() => table.flush())
    expect(table.measure.mock.calls.length).toBe(calls)
  })

  it('keeps the marks when only the forecast goes: the lines and the key stay, plain', () => {
    const { table } = scene()
    table.setView({ combat_forecast: null })
    expect(table.lines()).toHaveLength(10)
    expect(table.doomed()).toEqual([])
  })
})
