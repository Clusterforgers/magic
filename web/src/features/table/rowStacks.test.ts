// Goal: a seat's battlefield draws exactly the boxes the fit counts: lands and tokens share a box at once, identical cards when the zone would fall under 64 px, a tapped permanent takes the one box an upright one takes, a stack opened by the player or held open by a prompt takes a box per member, a permanent in combat stays alone; 40 permanents a seat fit the zones of a duel from 1024x768 up without a scroll, and an opponent's cards are never larger than yours.
import { describe, expect, it } from 'vitest'
import type { PlayState } from '@/play/store'
import { indexById } from '@/play/view'
import type { ObjectView, PermanentView } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { card, forest } from '@/test-utils/cards'
import type { RowKind } from './cardModel'
import { fixtureState } from './dev/devGame'
import { buildFixture } from './dev/fixture'
import { FIT_LIMITS, fitRows, LAND_SCALE } from './fit'
import type { FitBox } from './fit'
import { GROUP_OTHERS_BELOW } from './group'
import { POD_COLUMN_PX, regionSizes, ZONE_GAP_PX } from './layout'
import type { Viewport } from './layout'
import {
  battlefieldStacks,
  boxesOf,
  fitGroups,
  idsOfRows,
  keyOf,
  layoutSeat,
  sameEntries,
  visibleIds,
} from './rowStacks'
import type { RowEntry, SeatLayout } from './rowStacks'

const NEAR: readonly RowKind[] = ['creatures', 'other', 'lands']
const FAR: readonly RowKind[] = ['lands', 'other', 'creatures']
const NOTHING = new Set<never>()
const BIG: FitBox = { width: 1200, height: 600 }

const base = fixtureState(buildFixture({ seats: 2 }))

const state = (permanents: readonly ObjectView[], attackers: readonly number[] = []): PlayState => {
  const view = must(base.view)
  const objects = [...permanents]
  return {
    ...base,
    view: {
      ...view,
      objects,
      battlefield: { count: objects.length, cards: objects.map((object) => object.id) },
      combat: {
        ...view.combat,
        attackers: attackers.map((object) => ({
          object,
          target: { Player: 0 },
          blocked: false,
          blockers: [],
        })),
      },
    },
    objectsById: indexById(objects),
  }
}

const tapped = (object: ObjectView, on = true): ObjectView => ({
  ...object,
  permanent: { tapped: on, damage: 0, summoning_sick: false } satisfies PermanentView,
})
const named = (id: number, name: string, over: Partial<ObjectView> = {}): ObjectView =>
  card({ id, name, controller: 1, owner: 1, ...over })
const land = (id: number, name = 'Forest', over: Partial<ObjectView> = {}): ObjectView =>
  forest({ id, name, controller: 1, owner: 1, ...over })
const goblin = (id: number, over: Partial<ObjectView> = {}): ObjectView =>
  card({
    id,
    name: 'Goblin',
    kind: 'Token',
    controller: 1,
    owner: 1,
    colors: ['Red'],
    mana_cost: null,
    ...over,
  })

const layout = (
  permanents: readonly ObjectView[],
  box: FitBox | null = BIG,
  order: readonly RowKind[] = NEAR,
  extra: { opened?: ReadonlySet<string>; marked?: ReadonlySet<number> } = {},
): SeatLayout =>
  layoutSeat({
    box,
    stacks: battlefieldStacks(state(permanents), 1),
    order,
    opened: extra.opened ?? NOTHING,
    marked: extra.marked ?? NOTHING,
  })

const counts = (rows: SeatLayout['rows']): Record<RowKind, number> => ({
  creatures: boxesOf(rows.creatures),
  other: boxesOf(rows.other),
  lands: boxesOf(rows.lands),
})

describe('the stacks of a seat', () => {
  it('merge lands and tokens at once and keep other cards apart', () => {
    const rows = battlefieldStacks(
      state([
        land(1),
        land(2),
        land(3, 'Island'),
        goblin(4),
        goblin(5),
        named(6, 'Serra Angel'),
        named(7, 'Serra Angel'),
      ]),
      1,
    )
    expect(rows.plain.lands.map((stack) => stack.ids)).toEqual([[1, 2], [3]])
    expect(rows.plain.creatures.map((stack) => stack.ids)).toEqual([[4, 5], [6], [7]])
    expect(rows.merged.creatures.map((stack) => stack.ids)).toEqual([
      [4, 5],
      [6, 7],
    ])
  })

  it('give every permanent a stack of its own when asked not to group', () => {
    const rows = battlefieldStacks(state([land(1), land(2), goblin(3)]), 1, false)
    expect(rows.plain.lands.map((stack) => stack.ids)).toEqual([[1], [2]])
    expect(rows.plain.creatures.map((stack) => stack.ids)).toEqual([[3]])
    expect(rows.merged).toEqual(rows.plain)
  })

  it('keep a permanent in combat alone, whatever it equals', () => {
    const rows = battlefieldStacks(state([goblin(1), goblin(2), goblin(3)], [2]), 1)
    expect(rows.plain.creatures.map((stack) => stack.ids)).toEqual([[1, 3], [2]])
  })

  it('are worked out once per view and seat', () => {
    const table = state([land(1), land(2)])
    expect(battlefieldStacks(table, 1)).toBe(battlefieldStacks(table, 1))
    expect(battlefieldStacks(table, 0)).toEqual({ plain: idle(), merged: idle() })
  })

  it('name a stack by a member and list every permanent once', () => {
    const rows = battlefieldStacks(state([land(1), goblin(2), land(3), goblin(4)]), 1).plain
    expect(idsOfRows(rows).toSorted()).toEqual([1, 2, 3, 4])
    expect(keyOf(rows, 3)).toBe(keyOf(rows, 1))
    expect(keyOf(rows, 2)).not.toBe(keyOf(rows, 1))
    expect(keyOf(rows, 99)).toBeUndefined()
  })
})

function idle() {
  return { creatures: [], other: [], lands: [] }
}

describe('a tapped permanent against the fit', () => {
  // Fourteen creatures, ten other permanents and sixteen lands, as the development table draws a seat.
  const seat = (tapEvery: number, shape: 'distinct' | 'alike'): ObjectView[] => {
    const make = (id: number, row: RowKind): ObjectView => {
      const name = shape === 'distinct' ? `${row} ${id}` : row
      const object =
        row === 'lands'
          ? land(id, name)
          : row === 'creatures'
            ? named(id, name)
            : named(id, name, { types: ['Artifact'], power: null, toughness: null })
      return tapEvery > 0 && id % tapEvery === 0 ? tapped(object) : object
    }
    return [
      ...Array.from({ length: 14 }, (_, i) => make(i + 1, 'creatures')),
      ...Array.from({ length: 10 }, (_, i) => make(i + 20, 'other')),
      ...Array.from({ length: 16 }, (_, i) => make(i + 40, 'lands')),
    ]
  }

  it.each([0, 1, 2, 3, 5])(
    'counts one box a permanent however many are tapped (every %i-th), as the rows draw them',
    (every) => {
      const drawn = layout(seat(every, 'distinct'), { width: 700, height: 300 })
      const boxes = counts(drawn.rows)
      expect(boxes).toEqual({ creatures: 14, other: 10, lands: 16 })
      const fit = fitRows({ width: 700, height: 300 }, [
        { stacks: 14 },
        { stacks: 10 },
        { stacks: 16, scale: LAND_SCALE },
      ])
      expect(drawn.fit).toEqual(fit)
      for (const row of NEAR) expect(visibleIds(drawn.rows[row])).toHaveLength(boxes[row])
    },
  )

  it.each([
    ['all untapped', 0, { creatures: 14, other: 10, lands: 1 }],
    ['all tapped', 1, { creatures: 14, other: 10, lands: 1 }],
    ['every second tapped', 2, { creatures: 14, other: 10, lands: 2 }],
  ])(
    'puts the lands of one kind in one box and in two when some are tapped: %s',
    (_name, every, expected) => {
      const permanents = seat(every, 'alike')
      const drawn = layout(permanents.filter((object) => object.id < 20 || object.id >= 40))
      expect(drawn.rows.lands).toHaveLength(expected.lands)
      expect(boxesOf(drawn.rows.lands)).toBe(expected.lands)
    },
  )

  it.each([0, 2])('leaves the fit equal when the same cards are tapped (every %i-th)', (every) => {
    const box = { width: 800, height: 237 }
    const before = layout(seat(0, 'distinct'), box).fit
    const after = layout(seat(every, 'distinct'), box).fit
    expect(after?.cardWidth).toBe(before?.cardWidth)
    expect(after?.lines).toBe(before?.lines)
  })
})

describe('identical cards that are no lands or tokens', () => {
  const bears = (count: number): ObjectView[] =>
    Array.from({ length: count }, (_, i) => named(i + 1, 'Grizzly Bears'))
  const pack = (count: number, fillers: number): ObjectView[] => [
    ...bears(count),
    ...Array.from({ length: fillers }, (_, i) => named(100 + i, `Filler ${i}`)),
  ]

  it.each([
    ['a roomy zone keeps them apart', pack(6, 0), { width: 1000, height: 400 }, false],
    ['a lone card has nobody to merge with', pack(1, 3), { width: 400, height: 200 }, false],
    ['a crowded zone merges them', pack(12, 12), { width: 420, height: 110 }, true],
    ['a very small zone merges them', pack(8, 0), { width: 300, height: 90 }, true],
  ])('%s', (_name, permanents, box, merged) => {
    const drawn = layout(permanents, box)
    expect(drawn.merged).toBe(merged)
    const plain = layout(permanents, null)
    expect(plain.merged).toBe(false)
    expect(plain.fit).toBeNull()
    if (merged) {
      expect(must(layout(permanents, box, NEAR).fit).cardWidth).toBeGreaterThanOrEqual(
        must(fitRows(box, fitGroups(plain.rows, NEAR)).cardWidth),
      )
      expect(drawn.rows.creatures.length).toBeLessThan(plain.rows.creatures.length)
    } else {
      expect(drawn.rows).toEqual(plain.rows)
    }
  })

  it(`merge exactly while the plain fit is under ${GROUP_OTHERS_BELOW} px`, () => {
    const permanents = pack(10, 10)
    for (let height = 60; height <= 400; height += 20) {
      const box = { width: 500, height }
      const plainWidth = fitRows(box, fitGroups(layout(permanents, null).rows, NEAR)).cardWidth
      expect(layout(permanents, box).merged, `${height}`).toBe(plainWidth < GROUP_OTHERS_BELOW)
    }
  })
})

describe('an open stack', () => {
  const lands = [land(1), land(2), land(3), land(4, 'Island'), land(5, 'Island')]
  const forests = must(battlefieldStacks(state(lands), 1).plain.lands[0])

  it('takes a box a member when the player opened it, and one when closed', () => {
    const closed = layout(lands)
    const open = layout(lands, BIG, NEAR, { opened: new Set([forests.key]) })
    expect(boxesOf(closed.rows.lands)).toBe(2)
    expect(boxesOf(open.rows.lands)).toBe(4)
    expect(visibleIds(closed.rows.lands)).toEqual([1, 4])
    expect(visibleIds(open.rows.lands)).toEqual([1, 2, 3, 4])
    expect(open.rows.lands.map((entry) => [entry.open, entry.held])).toEqual([
      [true, false],
      [false, false],
    ])
  })

  it('is held open, past the player, while a prompt marks a member', () => {
    const held = layout(lands, BIG, NEAR, { marked: new Set([3]) })
    expect(held.rows.lands.map((entry) => [entry.open, entry.held])).toEqual([
      [true, true],
      [false, false],
    ])
    expect(visibleIds(held.rows.lands)).toEqual([1, 2, 3, 4])
  })

  it('never makes the cards larger, and a lone permanent has nothing to open', () => {
    const box = { width: 330, height: 150 }
    const closed = must(layout(lands, box).fit).cardWidth
    const open = must(layout(lands, box, NEAR, { opened: new Set([forests.key]) }).fit).cardWidth
    expect(open).toBeLessThanOrEqual(closed)
    const alone = layout([land(9)], BIG, NEAR, { opened: new Set(['#9']), marked: new Set([9]) })
    expect(alone.rows.lands).toEqual([{ ids: [9], open: false, held: false }])
  })

  it('compares by ids and openness, not by key', () => {
    const entry = (ids: number[], open = false, held = false): RowEntry => ({ ids, open, held })
    expect(sameEntries([entry([1, 2])], [entry([1, 2])])).toBe(true)
    expect(sameEntries([entry([1, 2])], [entry([1, 2], true)])).toBe(false)
    expect(sameEntries([entry([1, 2], true)], [entry([1, 2], true, true)])).toBe(false)
    expect(sameEntries([entry([1, 2])], [entry([1, 3])])).toBe(false)
    expect(sameEntries([entry([1, 2])], [entry([1, 2]), entry([3])])).toBe(false)
  })
})

// ---- 40 permanents a seat in the real zones of the table ---------------------------------------------------------

const SIZES: readonly [string, Viewport][] = [
  ['1920x1080', { width: 1920, height: 1080 }],
  ['1440x900', { width: 1440, height: 900 }],
  ['1366x768', { width: 1366, height: 768 }],
  ['1280x720', { width: 1280, height: 720 }],
  ['1024x768', { width: 1024, height: 768 }],
]

// A board beside a 168 px pod column inside a region padded by 8 px, in a zone padded by 4 px (`SeatArea`, `Battlefield`).
function boards(viewport: Viewport): { own: FitBox; opponent: FitBox } {
  const regions = regionSizes(viewport)
  const width = regions.main.width - 2 * ZONE_GAP_PX - POD_COLUMN_PX - ZONE_GAP_PX - 8
  const zone = (region: number): FitBox => ({
    width: Math.floor(width),
    height: Math.floor(region - 2 * ZONE_GAP_PX - 8),
  })
  return { own: zone(regions.own), opponent: zone(regions.opponents) }
}

const duel = fixtureState(buildFixture({ seats: 2 }))
const laid = (seat: number, box: FitBox, order: readonly RowKind[]): SeatLayout =>
  layoutSeat({ box, stacks: battlefieldStacks(duel, seat), order, opened: NOTHING, marked: NOTHING })

describe('40 permanents a seat in a duel', () => {
  it.each(SIZES)('fit both boards at %s without a zone scrolling', (_name, viewport) => {
    const { own, opponent } = boards(viewport)
    const mine = must(laid(0, own, NEAR).fit)
    const theirs = must(laid(1, opponent, FAR).fit)
    expect(mine.scrolls, 'your board').toBe(false)
    expect(theirs.scrolls, "the opponent's board").toBe(false)
    expect(mine.hidden + theirs.hidden).toBe(0)
    expect(mine.cardWidth).toBeGreaterThanOrEqual(FIT_LIMITS.minWidth)
  })

  it.each(SIZES)('shrink the opponent before you at %s', (_name, viewport) => {
    const { own, opponent } = boards(viewport)
    const mine = must(laid(0, own, NEAR).fit)
    const theirs = must(laid(1, opponent, FAR).fit)
    expect(own.height).toBeGreaterThan(opponent.height)
    expect(theirs.cardWidth).toBeLessThanOrEqual(mine.cardWidth)
    // The same cards in the same box come out the same on either side of the table.
    const alike = must(laid(1, own, FAR).fit)
    expect(alike.cardWidth).toBe(mine.cardWidth)
  })

  it('shows the count of what is cut off and scrolls once the floor does not fit', () => {
    const tiny = { width: 300, height: 90 }
    const fit = must(laid(0, tiny, NEAR).fit)
    expect(fit.scrolls).toBe(true)
    expect(fit.cardWidth).toBe(FIT_LIMITS.minWidth)
    expect(fit.hidden).toBeGreaterThan(0)
  })
})
