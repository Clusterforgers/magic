// Goal: an attachment on a permanent takes no box of its own but lies tucked behind its host, whoever controls it, in the order Auras, Equipment, Fortifications; the fit counts the height its name lines take above the host, and a lands lane with a tuck is not fanned.
import { describe, expect, it } from 'vitest'
import type { PlayState } from '@/play/store'
import { indexById } from '@/play/view'
import type { ObjectView } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { card } from '@/test-utils/cards'
import { battlefieldStacks, idsOfRows, laneCounts, layoutSeat, visibleIds } from './rowStacks'
import { seatCounts } from './boardSplit'
import { fixtureState } from './dev/devGame'
import { buildFixture } from './dev/fixture'
import { fitLanes, LANE_LIMITS, laneHeight, TUCK_MAX_SHARE, TUCK_MIN_PX, tuckOf } from './laneFit'

const base = fixtureState(buildFixture({ seats: 2 }))

const state = (objects: readonly ObjectView[]): PlayState => ({
  ...base,
  view: { ...must(base.view), objects: [...objects], battlefield: { count: objects.length, cards: objects.map((o) => o.id) } },
  objectsById: indexById(objects),
})

const creature = (id: number, attachments: number[] = [], controller = 1): ObjectView =>
  card({ id, name: `Bear ${id}`, controller, owner: controller, ...(attachments.length > 0 ? { attachments } : {}) })
const attachment = (id: number, subtype: string, to: number, controller = 1): ObjectView =>
  card({
    id,
    name: `${subtype} ${id}`,
    controller,
    owner: controller,
    types: subtype === 'Aura' ? ['Enchantment'] : ['Artifact'],
    subtypes: [subtype],
    power: null,
    toughness: null,
    attached_to: to,
  })

describe('tuckOf', () => {
  it.each([
    // [card height, attachments, px each shows]
    [210, 0, 0],
    [210, 1, 29],
    [210, 3, 29],
    [210, 5, 21],
    [140, 1, 20],
    [140, 3, 20],
    [140, 5, 14],
    [62, 1, 20],
    [62, 5, TUCK_MIN_PX],
  ])('a card %i px tall with %i attachments shows %i px of each', (height, depth, each) => {
    expect(tuckOf(height, depth)).toBe(each)
  })

  it.each([60, 100, 140, 210, 314])('never lets five or fewer attachments take more than the share of a %i px card', (height) => {
    for (let depth = 1; depth <= 5; depth += 1) {
      const each = tuckOf(height, depth)
      expect(each * depth <= height * TUCK_MAX_SHARE || each === TUCK_MIN_PX).toBe(true)
    }
  })
})

describe('the tuck of a seat', () => {
  it.each([
    // [label, objects, creatures drawn, tucked ids of the host 1]
    ['an Equipment', [creature(1, [2]), attachment(2, 'Equipment', 1)], 1, [2]],
    [
      'Equipment before Auras in id order are tucked Auras first',
      [creature(1, [2, 3, 4]), attachment(2, 'Equipment', 1), attachment(3, 'Aura', 1), attachment(4, 'Aura', 1)],
      1,
      [3, 4, 2],
    ],
    [
      'an Aura of another seat',
      [creature(1, [2]), attachment(2, 'Aura', 1, 0)],
      1,
      [2],
    ],
    ['an unattached Equipment', [creature(1), attachment(2, 'Equipment', 99)], 1, []],
  ])('%s', (_label, objects, drawn, tucked) => {
    const stacks = battlefieldStacks(state(objects), 1).plain
    expect(stacks.creatures).toHaveLength(drawn)
    expect(stacks.creatures[0]?.tucked ?? []).toEqual(tucked)
    expect(idsOfRows(stacks)).toEqual(expect.arrayContaining([1, ...tucked]))
  })

  it('takes a foreign attachment out of its controller\'s lanes', () => {
    const play = state([creature(1, [2]), attachment(2, 'Aura', 1, 0)])
    expect(idsOfRows(battlefieldStacks(play, 0).plain)).toEqual([])
    expect(seatCounts(play, 1)).toEqual({ lands: 0, creatures: 1, other: 0, tucked: { lands: 0, creatures: 1, other: 0 } })
  })

  it('keeps an unattached Equipment in the other lane, without a tuck', () => {
    const play = state([creature(1), attachment(2, 'Equipment', 99)])
    expect(seatCounts(play, 1)).toEqual({ lands: 0, creatures: 1, other: 1 })
  })

  it('lets the arrow keys reach the attachments after their host', () => {
    const play = state([creature(1, [2, 3]), attachment(2, 'Equipment', 1), attachment(3, 'Aura', 1), creature(4)])
    const layout = layoutSeat({ box: null, stacks: battlefieldStacks(play, 1), facing: 'near', opened: new Set(), marked: new Set() })
    expect(visibleIds(layout.rows.creatures)).toEqual([1, 3, 2, 4])
    expect(laneCounts(layout.rows).tucked).toEqual({ creatures: 2, other: 0, lands: 0 })
  })
})

describe('the fit of a lane with a tuck', () => {
  const box = { width: 1200, height: 500 }

  it.each([1, 2, 3, 5])('a host with %i attachments makes its line taller by their name lines', (depth) => {
    const plain = fitLanes(box, { lands: 4, creatures: 6, other: 2 }, 'near')
    const tucked = fitLanes(box, { lands: 4, creatures: 6, other: 2, tucked: { creatures: depth } }, 'near')
    expect(tucked.cardWidth).toBeLessThanOrEqual(plain.cardWidth)
    expect(tucked.height).toBeLessThanOrEqual(box.height)
    const lane = must(tucked.bands.flatMap((band) => band.lanes).find((place) => place.lane === 'creatures'))
    expect(lane.lift).toBe(depth * tuckOf(lane.height, depth))
    expect(lane.tall).toBe(lane.lines * (lane.height + lane.lift) + (lane.lines - 1) * LANE_LIMITS.rowGap)
  })

  it('needs more height for the same cards once a host carries attachments', () => {
    const counts = { lands: 6, creatures: 8, other: 3 }
    expect(laneHeight(1000, { ...counts, tucked: { creatures: 3 } }, 120)).toBeGreaterThan(laneHeight(1000, counts, 120))
  })

  it('does not fan a lands lane with a tuck', () => {
    const narrow = { width: 700, height: 400 }
    const fanned = fitLanes(narrow, { lands: 12, creatures: 2, other: 0 }, 'near')
    const tucked = fitLanes(narrow, { lands: 12, creatures: 2, other: 0, tucked: { lands: 1 } }, 'near')
    const pileOf = (fit: typeof fanned) => fit.bands.flatMap((band) => band.lanes).find((place) => place.lane === 'lands')?.pile
    expect(pileOf(fanned)).toBeGreaterThan(1)
    expect(pileOf(tucked)).toBe(1)
  })
})
