// Goal: the spotlight stands for a cast by another seat or one that passes through the stack inside the batch, and for the viewer's own cast that stays on the stack, for 0.6 of the time and never as a card back; it comes after the first leg (at once under reduced motion, where it is still), lasts `--fx-spotlight-ms` (0.6 of it for the viewer's own), is stretched by the slow-down, shows a back for a card the viewer may not identify, is 1.6 times a hand card (shrunk to the room above the strip, kept inside the window), and holds the second leg of the same card back until it is over.
import { describe, expect, it } from 'vitest'
import { card } from '@/test-utils/cards'
import { MOVE_MS } from './moves'
import type { Move } from './moves'
import type { Box, Spot } from './places'
import {
  CAPTION_PX,
  OWN_SHARE,
  planSpotlight,
  SPOTLIGHT_FLOOR_PX,
  SPOTLIGHT_GAP_PX,
  SPOTLIGHT_MIN_PX,
  spotlightBoxes,
  spotlightMs,
  spotlightKeyframes,
  spotlightSize,
} from './spotlight'
import type { SpotlightInput } from './spotlight'
import { planFlights } from './travel'

const ME = 0
const FOE = 1
const WINDOW = { width: 1440, height: 900 }
const STRIP: Box = { x: 200, y: 520, width: 800, height: 120 }
const HAND = { width: 90, height: 126 }

const cast = (over: Partial<Move> = {}): Move => ({
  kind: 'cast',
  object: 5,
  look: card({ id: 5, zone: 'Stack', name: 'Lightning Bolt', controller: FOE, owner: FOE }),
  from: { seat: FOE, zone: 'hand', object: 5 },
  to: { stack: true, object: 5 },
  order: 0,
  targets: [],
  spotlight: true,
  ...over,
})

/** The viewer's own cast of the same spell: it stays on the stack, so it has no full spotlight. */
const mine = (over: Partial<Move> = {}): Move =>
  cast({
    look: card({ id: 5, zone: 'Stack', name: 'Lightning Bolt', controller: ME, owner: ME }),
    from: { seat: ME, zone: 'hand', object: 5 },
    spotlight: false,
    ...over,
  })

const plan = (moves: Move[], over: Partial<SpotlightInput> = {}) =>
  planSpotlight({
    moves,
    strip: STRIP,
    hand: HAND,
    window: WINDOW,
    viewer: ME,
    batch: 3,
    still: false,
    ...over,
  })

describe('who stands in the spotlight', () => {
  it.each<[string, Move[], string | null]>([
    ['a cast by another seat', [cast()], 'Lightning Bolt'],
    ['the viewer’s own cast that stays on the stack', [mine()], 'Lightning Bolt'],
    ['the viewer’s own cast that resolves in the same batch', [mine({ spotlight: true })], 'Lightning Bolt'],
    [
      'another seat’s cast, which wins over the viewer’s own in the same batch',
      [mine(), cast({ order: 1, look: card({ id: 7, name: 'Shock', controller: FOE }), object: 7 })],
      'Shock',
    ],
    [
      'the viewer’s own cast after a land',
      [mine({ kind: 'landed', spotlight: false }), mine({ order: 1 })],
      'Lightning Bolt',
    ],
    ['a cast with no card to show', [mine({ look: null })], null],
    [
      'a face-down card of the viewer’s own',
      [mine({ look: card({ id: 5, name: 'Lightning Bolt', controller: ME, face_down: true }) })],
      null,
    ],
    [
      'a nameless card of the viewer’s own',
      [mine({ look: card({ id: 5, name: '', controller: ME }) })],
      null,
    ],
    ['an ability of the viewer’s own', [mine({ kind: 'activated' })], null],
    [
      'the first of two casts',
      [cast(), cast({ object: 6, look: card({ id: 6, name: 'Shock', controller: FOE }), order: 1 })],
      'Lightning Bolt',
    ],
    ['a land', [cast({ kind: 'landed' })], null],
    ['an ability', [cast({ kind: 'triggered' })], null],
    ['no move at all', [], null],
  ])('%s', (_, moves, name) => {
    expect(plan(moves)?.look?.name ?? null).toBe(name)
  })

  it('needs a strip on screen', () => {
    expect(plan([cast()], { strip: null })).toBeNull()
  })

  it('names the caster and the targets of the cast', () => {
    const targets = [{ Object: { id: 9, counter: 0 } }]
    const shown = plan([cast({ targets })])
    expect(shown).toMatchObject({ caster: FOE, targets, object: 5, key: '3:spotlight' })
  })
})

describe('when it stands', () => {
  it.each<[string, Partial<SpotlightInput>, Partial<Move>, number]>([
    ['after the first leg has landed', {}, {}, MOVE_MS.travel],
    ['in step with its place in the batch', {}, { order: 2 }, 2 * MOVE_MS.stagger + MOVE_MS.travel],
    ['at once when nothing flies (reduced motion)', { still: true }, {}, 0],
    ['stretched by the slow-down', { slow: 5 }, {}, 5 * MOVE_MS.travel],
  ])('%s', (_, over, move, delay) => {
    expect(plan([cast(move)], over)?.delay).toBe(delay)
  })

  it('lasts the spotlight token, stretched by the slow-down', () => {
    expect(plan([cast()])?.duration).toBe(MOVE_MS.spotlight)
    expect(plan([cast()], { slow: 3 })?.duration).toBe(3 * MOVE_MS.spotlight)
  })

  it.each<[string, Move[], Partial<SpotlightInput>, boolean, number, number]>([
    ['another seat’s cast', [cast()], {}, false, MOVE_MS.spotlight, MOVE_MS.travel],
    ['the viewer’s own cast', [mine()], {}, true, 540, MOVE_MS.travel],
    ['the viewer’s own cast, stretched', [mine()], { slow: 5 }, true, 5 * 540, 5 * MOVE_MS.travel],
    ['the viewer’s own cast under reduced motion', [mine()], { still: true }, true, 540, 0],
    ['the viewer’s own cast that resolves in the batch', [mine({ spotlight: true })], {}, false, 900, 380],
  ])('%s: own %s, lasts %s ms after %s ms', (_, moves, over, own, duration, delay) => {
    expect(plan(moves, over)).toMatchObject({ own, duration, delay })
  })

  it('stands for 0.6 of the full time in the same 150 ms fades, so it holds for 240 ms', () => {
    expect(OWN_SHARE).toBe(0.6)
    expect(spotlightMs(true)).toBe(0.6 * MOVE_MS.spotlight)
    const frames = spotlightKeyframes({ still: false, own: true })
    const duration = spotlightMs(true)
    expect((frames[1]?.offset ?? 0) * duration).toBeCloseTo(150)
    expect((1 - (frames[2]?.offset ?? 0)) * duration).toBeCloseTo(150)
    const held = ((frames[2]?.offset ?? 0) - (frames[1]?.offset ?? 0)) * duration
    expect(held).toBeCloseTo(240)
    const still = spotlightKeyframes({ still: true, own: true })
    expect(still.map((frame) => frame.opacity)).toEqual([0, 1, 1, 0])
  })

  it('keeps a whole cast under 1.7 s: the leg, the spotlight and the leg of the resolution', () => {
    const resolved: Move = { ...cast(), kind: 'resolved', from: { stack: true, object: 5 }, order: 1 }
    const spot = (x: number): Spot => ({ box: { x, y: 300, width: 100, height: 140 }, card: false })
    const flights = planFlights({
      moves: [cast(), resolved],
      viewer: ME,
      starts: [spot(0), spot(400)],
      ends: [spot(400), spot(800)],
      batch: 1,
    })
    const [, second] = flights
    expect(second?.delay).toBe(MOVE_MS.travel + MOVE_MS.spotlight)
    expect((second?.delay ?? 0) + (second?.duration ?? 0)).toBeLessThan(1700)
  })

  it('holds the second leg only when the first stood in the spotlight', () => {
    const resolved: Move = {
      ...cast({ spotlight: false }),
      kind: 'resolved',
      from: { stack: true, object: 5 },
      order: 1,
    }
    const spot = (x: number): Spot => ({ box: { x, y: 300, width: 100, height: 140 }, card: false })
    const flights = planFlights({
      moves: [cast({ spotlight: false }), resolved],
      viewer: ME,
      starts: [spot(0), spot(400)],
      ends: [spot(400), spot(800)],
      batch: 1,
    })
    expect(flights[1]?.delay).toBe(MOVE_MS.travel)
  })
})

describe('what it shows', () => {
  it.each<[string, Partial<Move>, boolean]>([
    ['a card the viewer can identify', {}, false],
    ['a card the viewer cannot identify', { look: null }, true],
    [
      'a face-down card',
      { look: card({ id: 5, zone: 'Stack', name: 'Lightning Bolt', face_down: true }) },
      true,
    ],
    ['a card with no name', { look: card({ id: 5, zone: 'Stack', name: '', controller: FOE }) }, true],
  ])('%s: the back is %s', (_, over, back) => {
    expect(plan([cast(over)])?.faceDown).toBe(back)
  })

  it('is still under reduced motion and moves otherwise', () => {
    expect(plan([cast()], { still: true })?.still).toBe(true)
    expect(plan([cast()])?.still).toBe(false)
    const still = spotlightKeyframes({ still: true })
    expect(still.every((frame) => frame.transform === undefined)).toBe(true)
    expect(still.map((frame) => frame.opacity)).toEqual([0, 1, 1, 0])
    const moving = spotlightKeyframes({ still: false })
    expect(moving.map((frame) => frame.opacity)).toEqual([0, 1, 1, 0])
    expect(moving[0]?.transform).toMatch(/scale/)
    expect(moving[1]?.offset).toBeCloseTo(150 / MOVE_MS.spotlight)
    expect(moving[2]?.offset).toBeCloseTo(1 - 150 / MOVE_MS.spotlight)
  })
})

describe('where it stands', () => {
  it('is 1.6 times a hand card, not lower than the minimum nor higher than 40 % of the window', () => {
    expect(spotlightSize({ width: 100, height: 200 }, WINDOW).height).toBeCloseTo(320)
    expect(spotlightSize({ width: 40, height: 60 }, WINDOW).height).toBe(SPOTLIGHT_MIN_PX)
    expect(spotlightSize({ width: 200, height: 400 }, WINDOW).height).toBeCloseTo(360)
    expect(spotlightSize(null, WINDOW).height).toBeCloseTo(224)
  })

  it('shrinks to the room above the strip, down to the floor', () => {
    expect(spotlightSize(HAND, WINDOW, 140).height).toBe(140)
    expect(spotlightSize(HAND, WINDOW, 10).height).toBe(SPOTLIGHT_FLOOR_PX)
  })

  it('stands centred over the strip with its foot above it, the caption under the card', () => {
    const size = spotlightSize(HAND, WINDOW)
    const { card: picture, box } = spotlightBoxes(STRIP, size, WINDOW)
    expect(box.x + box.width / 2).toBeCloseTo(STRIP.x + STRIP.width / 2)
    expect(box.y + box.height).toBe(STRIP.y - SPOTLIGHT_GAP_PX)
    expect(picture.y).toBe(box.y)
    expect(picture.height + CAPTION_PX).toBe(box.height)
    expect(picture.x + picture.width / 2).toBeCloseTo(box.x + box.width / 2)
  })

  it.each<[string, Box]>([
    ['at the left edge', { x: 0, y: 520, width: 100, height: 120 }],
    ['at the right edge', { x: 1340, y: 520, width: 100, height: 120 }],
    ['under the top edge', { x: 400, y: 100, width: 600, height: 120 }],
  ])('stays inside the window %s', (_, strip) => {
    const { box } = spotlightBoxes(strip, spotlightSize(HAND, WINDOW), WINDOW)
    expect(box.x).toBeGreaterThanOrEqual(0)
    expect(box.y).toBeGreaterThanOrEqual(0)
    expect(box.x + box.width).toBeLessThanOrEqual(WINDOW.width)
    expect(box.y + box.height).toBeLessThanOrEqual(WINDOW.height)
  })

  it.each<[Move['kind'], string | null]>([
    ['cast', null],
    ['resolved', 'resolved'],
    ['countered', 'countered'],
    ['fizzled', 'fizzled'],
    ['moved', null],
  ])('a flying %s card carries the badge %s', (kind, mark) => {
    const spot = (x: number): Spot => ({ box: { x, y: 300, width: 100, height: 140 }, card: false })
    const [flight] = planFlights({
      moves: [{ ...cast({ spotlight: false }), kind }],
      viewer: ME,
      starts: [spot(0)],
      ends: [spot(400)],
      batch: 1,
    })
    expect(flight?.mark).toBe(mark)
  })

  it('never lies over the strip when the room above it allows', () => {
    const shown = plan([cast()], {
      strip: { x: 8, y: 210, width: 374, height: 120 },
      window: { width: 390, height: 720 },
    })
    expect(shown).not.toBeNull()
    expect((shown?.box.y ?? 0) + (shown?.box.height ?? 0)).toBeLessThanOrEqual(210 - SPOTLIGHT_GAP_PX)
  })
})
