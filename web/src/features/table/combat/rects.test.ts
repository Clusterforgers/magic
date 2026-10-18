// Goal: anchors are found in the page by the data attributes cards and seats carry (a card by its face, a seat by its panel, else its tile, a card that is not drawn by its controller's), measured relative to the overlay, hidden ones skipped; lines get a curve only when both ends are on screen with room between them and identical drawings are made once; and the scheduler runs its work once per frame however often it is asked.
import { afterEach, describe, expect, it, vi } from 'vitest'
import { fakeFrames } from '@/test-utils/frames'
import type { Rect } from './geometry'
import { anchorKey } from './plan'
import type { Anchor, LineKind, LineSpec } from './plan'
import { createScheduler, domRects, sameRects, selectorsOf, shapesOf } from './rects'
import type { RectMap } from './rects'

const card = (id: number, seat?: number): Anchor =>
  seat === undefined ? { kind: 'card', id } : { kind: 'card', id, seat }
const seat = (n: number): Anchor => ({ kind: 'seat', seat: n })

describe('selectorsOf', () => {
  it.each<[string, Anchor, string[]]>([
    ['a card', card(12), ['button[data-object-id="12"]']],
    [
      'a card with a controller',
      card(12, 3),
      ['button[data-object-id="12"]', '[data-seat="3"]', '[data-tile-seat="3"]'],
    ],
    ['a seat', seat(2), ['[data-seat="2"]', '[data-tile-seat="2"]']],
    ['seat 0', seat(0), ['[data-seat="0"]', '[data-tile-seat="0"]']],
  ])('looks for %s', (_name, anchor, expected) => {
    expect(selectorsOf(anchor)).toEqual(expected)
  })
})

/** An element whose box is `rect`, in page coordinates. */
function placed<T extends Element>(element: T, rect: Rect | null): T {
  const box = rect ?? { x: 0, y: 0, width: 0, height: 0 }
  element.getBoundingClientRect = () =>
    ({
      x: box.x,
      y: box.y,
      left: box.x,
      top: box.y,
      right: box.x + box.width,
      bottom: box.y + box.height,
      width: box.width,
      height: box.height,
      toJSON: () => ({}),
    }) as DOMRect
  return element
}

function child<K extends keyof HTMLElementTagNameMap>(
  parent: Element,
  tag: K,
  attributes: Record<string, string>,
  rect: Rect | null,
): HTMLElementTagNameMap[K] {
  const element = document.createElement(tag)
  for (const [name, value] of Object.entries(attributes)) element.setAttribute(name, value)
  parent.append(element)
  return placed(element, rect)
}

/** A table-shaped page: an overlay at (100, 50) among cards, a panel and a tile, all in page coordinates. */
function page() {
  const table = document.createElement('div')
  document.body.append(table)
  const origin = placed(document.createElementNS('http://www.w3.org/2000/svg', 'svg'), {
    x: 100,
    y: 50,
    width: 800,
    height: 600,
  })
  table.append(origin)
  return { table, origin }
}

afterEach(() => {
  document.body.replaceChildren()
})

describe('domRects', () => {
  it('measures a card by its face, relative to the overlay', () => {
    const { table, origin } = page()
    const button = child(
      table,
      'button',
      { 'data-object-id': '1' },
      { x: 120, y: 80, width: 134, height: 134 },
    )
    child(button, 'span', { 'data-slot': 'card-face' }, { x: 130, y: 90, width: 134, height: 96 })
    const found = domRects.measure([card(1)], origin)
    expect(found.get('card:1')).toEqual({ x: 30, y: 40, width: 134, height: 96 })
  })

  it('measures a card without a face by the card itself', () => {
    const { table, origin } = page()
    child(table, 'button', { 'data-object-id': '2' }, { x: 300, y: 250, width: 60, height: 84 })
    expect(domRects.measure([card(2)], origin).get('card:2')).toEqual({
      x: 200,
      y: 200,
      width: 60,
      height: 84,
    })
  })

  it('measures a seat by its panel, and by its tile when it has none', () => {
    const { table, origin } = page()
    child(table, 'section', { 'data-seat': '0' }, { x: 100, y: 500, width: 700, height: 200 })
    child(table, 'button', { 'data-tile-seat': '0' }, { x: 100, y: 60, width: 150, height: 70 })
    child(table, 'button', { 'data-tile-seat': '3' }, { x: 300, y: 60, width: 150, height: 70 })
    const found = domRects.measure([seat(0), seat(3)], origin)
    expect(found.get('seat:0')).toEqual({ x: 0, y: 450, width: 700, height: 200 })
    expect(found.get('seat:3')).toEqual({ x: 200, y: 10, width: 150, height: 70 })
  })

  it('measures a panel as a whole, not the first card it holds', () => {
    const { table, origin } = page()
    const panel = child(table, 'section', { 'data-seat': '0' }, { x: 100, y: 500, width: 700, height: 200 })
    const graveyard = child(
      panel,
      'button',
      { 'data-object-id': '4' },
      { x: 300, y: 510, width: 60, height: 84 },
    )
    child(graveyard, 'span', { 'data-slot': 'card-face' }, { x: 300, y: 510, width: 60, height: 84 })
    const found = domRects.measure([seat(0), card(4)], origin)
    expect(found.get('seat:0')).toEqual({ x: 0, y: 450, width: 700, height: 200 })
    expect(found.get('card:4')).toEqual({ x: 200, y: 460, width: 60, height: 84 })
  })

  it('stands in the controller of a card that is not drawn', () => {
    const { table, origin } = page()
    child(table, 'button', { 'data-tile-seat': '4' }, { x: 500, y: 60, width: 150, height: 70 })
    const found = domRects.measure([card(7, 4), card(8)], origin)
    expect(found.get('card:7')).toEqual({ x: 400, y: 10, width: 150, height: 70 })
    expect(found.has('card:8')).toBe(false)
  })

  it('skips an element that has no size and goes on to the next place to look', () => {
    const { table, origin } = page()
    child(table, 'button', { 'data-object-id': '5' }, null)
    child(table, 'section', { 'data-seat': '2' }, { x: 100, y: 100, width: 300, height: 100 })
    const found = domRects.measure([card(5, 2), card(6)], origin)
    expect(found.get('card:5')).toEqual({ x: 0, y: 50, width: 300, height: 100 })
    expect(found.has('card:6')).toBe(false)
  })

  it('only looks inside the table around the overlay', () => {
    const { origin } = page()
    const stray = document.createElement('button')
    stray.setAttribute('data-object-id', '9')
    document.body.append(placed(stray, { x: 0, y: 0, width: 50, height: 50 }))
    expect(domRects.measure([card(9)], origin).size).toBe(0)
  })

  it('measures nothing for no anchors', () => {
    const { origin } = page()
    expect(domRects.measure([], origin).size).toBe(0)
  })
})

describe('sameRects', () => {
  const a: RectMap = new Map([['card:1', { x: 1, y: 2, width: 3, height: 4 }]])

  it.each<[string, RectMap, boolean]>([
    ['the same boxes', new Map([['card:1', { x: 1, y: 2, width: 3, height: 4 }]]), true],
    ['another x', new Map([['card:1', { x: 9, y: 2, width: 3, height: 4 }]]), false],
    ['another y', new Map([['card:1', { x: 1, y: 9, width: 3, height: 4 }]]), false],
    ['another width', new Map([['card:1', { x: 1, y: 2, width: 9, height: 4 }]]), false],
    ['another height', new Map([['card:1', { x: 1, y: 2, width: 3, height: 9 }]]), false],
    ['another anchor', new Map([['card:2', { x: 1, y: 2, width: 3, height: 4 }]]), false],
    ['one more box', new Map([...a, ['seat:0', { x: 0, y: 0, width: 1, height: 1 }]]), false],
    ['no boxes', new Map(), false],
  ])('%s', (_name, other, expected) => {
    expect(sameRects(a, other)).toBe(expected)
  })
})

let counter = 0
const spec = (
  from: Anchor,
  to: Anchor,
  kind: LineKind = 'attack',
  over: Partial<LineSpec> = {},
): LineSpec => {
  counter += 1
  return { key: `line:${counter}`, kind, pending: false, from, to, ...over }
}
const boxes = (entries: [Anchor, Rect][]): RectMap => new Map(entries.map(([a, r]) => [anchorKey(a), r]))
const CARD: Rect = { x: 100, y: 0, width: 96, height: 134 }
const PANEL: Rect = { x: 0, y: 400, width: 800, height: 200 }

describe('shapesOf', () => {
  it('draws a curve for a line whose two ends are on screen', () => {
    const lines = [spec(card(1), seat(0))]
    const drawn = shapesOf(
      lines,
      boxes([
        [card(1), CARD],
        [seat(0), PANEL],
      ]),
    )
    expect(drawn).toHaveLength(1)
    expect(drawn[0]?.line).toBe(lines[0])
    expect(drawn[0]?.shape.d).toMatch(/^M /)
  })

  it.each<[string, RectMap]>([
    ['the start is not on screen', boxes([[seat(0), PANEL]])],
    ['the end is not on screen', boxes([[card(1), CARD]])],
    ['neither is', boxes([])],
    [
      'the boxes overlap',
      boxes([
        [card(1), CARD],
        [seat(0), { ...CARD, x: 120 }],
      ]),
    ],
    [
      'a box has no size',
      boxes([
        [card(1), { x: 0, y: 0, width: 0, height: 0 }],
        [seat(0), PANEL],
      ]),
    ],
  ])('draws nothing when %s', (_name, rects) => {
    expect(shapesOf([spec(card(1), seat(0))], rects)).toEqual([])
  })

  it('keeps the order of the lines and skips only the ones it cannot draw', () => {
    const lines = [spec(card(1), seat(0)), spec(card(2), seat(0)), spec(card(3), seat(0))]
    const rects = boxes([
      [card(1), CARD],
      [card(3), { ...CARD, x: 300 }],
      [seat(0), PANEL],
    ])
    expect(shapesOf(lines, rects).map((drawn) => drawn.line)).toEqual([lines[0], lines[2]])
  })

  it('draws lines that would lie exactly on each other once: cards standing in as one tile', () => {
    const tile: Rect = { x: 300, y: 20, width: 150, height: 70 }
    const lines = [spec(card(1, 4), seat(0)), spec(card(2, 4), seat(0)), spec(card(3, 4), seat(0))]
    const rects = boxes([
      [card(1, 4), tile],
      [card(2, 4), tile],
      [card(3, 4), tile],
      [seat(0), PANEL],
    ])
    expect(shapesOf(lines, rects)).toHaveLength(1)
  })

  it('does not merge lines that differ in kind, in being unsent or in outcome', () => {
    const rects = boxes([
      [card(1), CARD],
      [seat(0), PANEL],
      [card(2), { ...CARD, y: 200 }],
    ])
    const same = (over: Partial<LineSpec>, kind: LineKind = 'attack'): LineSpec =>
      spec(card(1), seat(0), kind, over)
    const lines = [
      same({}),
      same({ pending: true }),
      same({ outcome: 'both-die' }, 'block'),
      same({ outcome: 'attacker-dies' }, 'block'),
      same({}, 'block'),
    ]
    expect(shapesOf(lines, rects)).toHaveLength(5)
  })

  it('draws blocks with a bigger head than attacks over the same distance', () => {
    const rects = boxes([
      [card(1), CARD],
      [seat(0), PANEL],
    ])
    const wing = (kind: LineKind): number => {
      const [shape] = shapesOf([spec(card(1), seat(0), kind)], rects)
      const [tipX, tipY, wingX, wingY] = (shape?.shape.head.match(/-?\d+(\.\d+)?/g) ?? []).map(Number)
      return Math.hypot((tipX ?? 0) - (wingX ?? 0), (tipY ?? 0) - (wingY ?? 0))
    }
    expect(wing('block')).toBeGreaterThan(wing('attack'))
  })
})

describe('createScheduler', () => {
  it('asks for one frame however often it is asked, and runs the work once in it', () => {
    const { frames, waiting, flush } = fakeFrames()
    const work = vi.fn()
    const scheduler = createScheduler(work, frames)
    for (let i = 0; i < 25; i += 1) scheduler.schedule()
    expect(waiting()).toBe(1)
    expect(work).not.toHaveBeenCalled()
    flush()
    expect(work).toHaveBeenCalledTimes(1)
    expect(waiting()).toBe(0)
  })

  it('takes a new request after the frame has run', () => {
    const { frames, waiting, flush } = fakeFrames()
    const work = vi.fn()
    const scheduler = createScheduler(work, frames)
    scheduler.schedule()
    flush()
    scheduler.schedule()
    scheduler.schedule()
    expect(waiting()).toBe(1)
    flush()
    expect(work).toHaveBeenCalledTimes(2)
  })

  it('lets the work ask for the next frame', () => {
    const { frames, waiting, flush } = fakeFrames()
    const scheduler = createScheduler(() => scheduler.schedule(), frames)
    scheduler.schedule()
    flush()
    expect(waiting()).toBe(1)
  })

  it('forgets a waiting request when stopped, and stopping with none waiting is harmless', () => {
    const { frames, waiting, flush } = fakeFrames()
    const work = vi.fn()
    const scheduler = createScheduler(work, frames)
    scheduler.stop()
    scheduler.schedule()
    scheduler.stop()
    expect(waiting()).toBe(0)
    flush()
    expect(work).not.toHaveBeenCalled()
    scheduler.schedule()
    flush()
    expect(work).toHaveBeenCalledTimes(1)
  })
})
