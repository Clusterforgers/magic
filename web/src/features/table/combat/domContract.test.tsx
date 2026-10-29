// Goal: the real table and the real DOM provider agree about where things are: cards are found by `data-object-id` and measured by their face, a seat by its panel as a whole (never by a card inside it), a seat that is only a tile by the tile, positions are relative to the overlay, and lines whose ends are not drawn stand in the tile of their controller and are drawn once; jsdom has no layout, so each element's box is given by a table of where the page would put it.
import { afterEach, describe, expect, it, vi } from 'vitest'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { forecastScene } from '../dev/combatScene'
import { buildFixture } from '../dev/fixture'
import { GameTable } from '../GameTable'
import type { Rect } from './geometry'

/** Where the overlay starts on the page; every box the overlay reports is relative to it. */
let origin = { x: 0, y: 0 }

const cardBox = (id: number): Rect => ({
  x: 30 + (id % 20) * 60,
  y: 200 + Math.floor(id / 20) * 100,
  width: 50,
  height: 70,
})
/** The face of a card sits a little inside the button that holds it, as a turned card's box does. */
const faceBox = (id: number): Rect => {
  const box = cardBox(id)
  return { x: box.x + 3, y: box.y + 3, width: box.width - 6, height: box.height - 6 }
}
const panelBox = (seat: number): Rect => ({ x: 0, y: 3000 + 400 * seat, width: 1200, height: 300 })
const tileBox = (seat: number): Rect => ({ x: 100 + 170 * seat, y: 20, width: 160, height: 70 })

/** The page coordinates of an element, as a table says them; null for everything that has no box. */
function boxOf(element: Element): Rect | null {
  if (element.getAttribute('data-slot') === 'combat-overlay') return { ...origin, width: 1400, height: 5000 }
  const seat = element.getAttribute('data-seat')
  if (seat !== null) return panelBox(Number(seat))
  const tile = element.getAttribute('data-tile-seat')
  if (tile !== null) return tileBox(Number(tile))
  const button = element.closest('button[data-object-id]')
  if (button === null) return null
  const id = Number(button.getAttribute('data-object-id'))
  return element === button
    ? cardBox(id)
    : element.getAttribute('data-slot') === 'card-face'
      ? faceBox(id)
      : null
}

function layOut(): void {
  vi.spyOn(Element.prototype, 'getBoundingClientRect').mockImplementation(function (this: Element) {
    const box = boxOf(this) ?? { x: 0, y: 0, width: 0, height: 0 }
    return {
      x: box.x,
      y: box.y,
      left: box.x,
      top: box.y,
      right: box.x + box.width,
      bottom: box.y + box.height,
      width: box.width,
      height: box.height,
      toJSON: () => ({}),
    }
  })
}

afterEach(() => {
  vi.restoreAllMocks()
  origin = { x: 0, y: 0 }
})

const distanceTo = (rect: Rect, point: { x: number; y: number }): number =>
  Math.hypot(
    Math.max(rect.x - point.x, 0, point.x - (rect.x + rect.width)),
    Math.max(rect.y - point.y, 0, point.y - (rect.y + rect.height)),
  )

const point = (path: string, at = 0): { x: number; y: number } => {
  const numbers = (path.match(/-?\d+(\.\d+)?/g) ?? []).map(Number)
  return { x: numbers[at] ?? Number.NaN, y: numbers[at + 1] ?? Number.NaN }
}

const shifted = (rect: Rect, by: { x: number; y: number }): Rect => ({
  ...rect,
  x: rect.x - by.x,
  y: rect.y - by.y,
})

/** The scene at `seats` seats on the real page provider, and what each drawn line is made of. */
function scene(seats: number, options: { mode?: 'grid' | 'focus'; focus?: number } = {}) {
  layOut()
  const table = renderTable({
    fixture: forecastScene(buildFixture({ seats })),
    options,
    ui: <GameTable />,
  })
  const view = must(table.store.getState().view)
  const groups = (): Element[] => [...table.container.querySelectorAll('[data-layer="lines"] g[data-line]')]
  const line = (key: string): Element =>
    must(groups().find((group) => group.getAttribute('data-line') === key))
  const path = (key: string, part: '.combat-line' | '.combat-head'): string =>
    must(line(key).querySelector(part)?.getAttribute('d'))
  return { table, view, groups, line, path }
}

describe('the real page provider over the real table', () => {
  it('draws every line of a combat whose cards are all on screen', () => {
    const { groups, view } = scene(2)
    const blockers = view.combat.attackers.flatMap((attacker) => attacker.blockers)
    expect(groups()).toHaveLength(view.combat.attackers.length + blockers.length)
  })

  it('starts a line at the edge of the face of its card, not of the button that holds it', () => {
    const { path, view } = scene(2)
    const [first] = view.combat.attackers
    const id = must(first).object
    const start = point(path(`attack:${id}`, '.combat-line'))
    expect(distanceTo(faceBox(id), start)).toBeGreaterThan(0)
    expect(distanceTo(faceBox(id), start)).toBeLessThanOrEqual(3.01)
    expect(distanceTo(cardBox(id), start)).toBeLessThan(distanceTo(faceBox(id), start) + 3.01)
  })

  it('ends an attack line at the edge of the seat panel as a whole, and not of a card inside the panel', () => {
    const { path, view } = scene(2)
    const attacker = must(view.combat.attackers[0])
    const tip = point(path(`attack:${attacker.object}`, '.combat-head'))
    expect(distanceTo(panelBox(0), tip)).toBeGreaterThan(0)
    expect(distanceTo(panelBox(0), tip)).toBeLessThanOrEqual(3.01)
    // No card of the viewer (its commanders wait in the hand dock) is where the line ends.
    const inside = [...document.querySelectorAll('[data-seat="0"] button[data-object-id]')]
    for (const card of inside) {
      expect(distanceTo(faceBox(Number(card.getAttribute('data-object-id'))), tip)).toBeGreaterThan(3.01)
    }
  })

  it('ends a block line at the face of the attacker it blocks, and starts it at the face of the blocker', () => {
    const { path, view } = scene(2)
    const attacker = must(view.combat.attackers[0])
    const blocker = must(attacker.blockers[0])
    const tip = point(path(`block:${blocker}`, '.combat-head'))
    const start = point(path(`block:${blocker}`, '.combat-line'))
    expect(distanceTo(faceBox(attacker.object), tip)).toBeLessThanOrEqual(3.01)
    expect(distanceTo(faceBox(blocker), start)).toBeLessThanOrEqual(3.01)
  })

  it('measures from the overlay: moving it on the page moves every coordinate by the same amount', () => {
    origin = { x: 50, y: 80 }
    const { path, view } = scene(2)
    const id = must(view.combat.attackers[0]).object
    const start = point(path(`attack:${id}`, '.combat-line'))
    const by = { x: 50, y: 80 }
    expect(distanceTo(shifted(faceBox(id), by), start)).toBeLessThanOrEqual(3.01)
    expect(
      distanceTo(shifted(panelBox(0), by), point(path(`attack:${id}`, '.combat-head'))),
    ).toBeLessThanOrEqual(3.01)
  })
})

describe('a table in focus mode, where only one opponent is drawn in full', () => {
  // Six seats: Bo (seat 1) attacks Alice, but the board on show is Dee's (seat 3), so Bo is a tile only.
  const focused = () => scene(6, { mode: 'focus', focus: 3 })

  it('has the tile of the attacking seat and none of its cards', () => {
    const { table, view } = focused()
    expect(table.container.querySelector('[data-tile-seat="1"]')).not.toBeNull()
    expect(table.container.querySelector('[data-seat="1"]')).toBeNull()
    const id = must(view.combat.attackers[0]).object
    expect(table.container.querySelector(`button[data-object-id="${id}"]`)).toBeNull()
  })

  it('stands the tile in for the attackers: their attack lines start at the tile, and lie on each other once', () => {
    const { groups, path, view } = focused()
    const attacks = groups().filter((group) => group.getAttribute('data-kind') === 'attack')
    expect(attacks).toHaveLength(1)
    const first = must(view.combat.attackers[0]).object
    const start = point(path(`attack:${first}`, '.combat-line'))
    expect(distanceTo(tileBox(1), start)).toBeGreaterThan(0)
    expect(distanceTo(tileBox(1), start)).toBeLessThanOrEqual(3.01)
  })

  it('draws the blocks from the cards of the viewer to the tile that stands for the attacker', () => {
    const { groups, path, view } = focused()
    const blocks = groups().filter((group) => group.getAttribute('data-kind') === 'block')
    const blockers = view.combat.attackers.flatMap((attacker) => attacker.blockers)
    expect(blocks).toHaveLength(blockers.length)
    const [blocker] = blockers
    const tip = point(path(`block:${blocker}`, '.combat-head'))
    expect(distanceTo(tileBox(1), tip)).toBeLessThanOrEqual(3.01)
    expect(
      distanceTo(faceBox(must(blocker)), point(path(`block:${blocker}`, '.combat-line'))),
    ).toBeLessThanOrEqual(3.01)
  })

  it('ends the attack at the viewer panel, which is always drawn', () => {
    const { path, view } = focused()
    const id = must(view.combat.attackers[0]).object
    expect(distanceTo(panelBox(0), point(path(`attack:${id}`, '.combat-head')))).toBeLessThanOrEqual(3.01)
  })
})

describe('a table whose boxes are not laid out', () => {
  it('draws no line, and no error, when nothing has a size', () => {
    const table = renderTable({ fixture: forecastScene(buildFixture({ seats: 2 })), ui: <GameTable /> })
    expect(table.container.querySelectorAll('[data-layer="lines"] g')).toHaveLength(0)
    expect(table.container.querySelector('[data-slot="combat-overlay"]')).not.toBeNull()
  })
})
