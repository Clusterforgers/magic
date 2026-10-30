// Goal: the travel of a batch plays its moves in order, staggered and capped, hides what the viewer may not identify behind a card back, chains the legs of one card, skips a move whose place is not on screen, takes each picture's size from the card it starts or ends on, runs a straight path that ends on the destination's centre and fades there, measures a source when the batch arrives (before the table redraws) and a destination on the next frame, flies nothing under reduced motion or off, and gives way to a newer batch.
import { beforeEach, describe, expect, it } from 'vitest'
import { card } from '@/test-utils/cards'
import { place, travelFrames } from '@/test-utils/fxDom'
import type { TestBox } from '@/test-utils/fxDom'
import { MAX_MOVES, MOVE_MS } from './moves'
import type { FxPlace, Move } from './moves'
import type { Spot } from './places'
import type { FxMode } from './store'
import { createTravel, keyframesOf, MIN_SCALE, planFlights, REFERENCE_CARD } from './travel'
import type { Flight } from './travel'

const ME = 0
const FOE = 1

const spot = (x: number, y: number, width: number, height: number, isCard = false): Spot => ({
  box: { x, y, width, height },
  card: isCard,
})
const HAND = spot(100, 700, 100, 140, true)
const STACK = spot(400, 300, 120, 32)

function move(over: Partial<Move> = {}): Move {
  return {
    kind: 'cast',
    object: 5,
    look: card({ id: 5, zone: 'Stack' }),
    from: { seat: ME, zone: 'hand', object: 5 },
    to: { stack: true, object: 5 },
    order: 0,
    targets: [],
    spotlight: false,
    ...over,
  }
}

const sequence = (count: number): Move[] =>
  Array.from({ length: count }, (_, order) => move({ object: 10 + order, order }))

interface Run {
  moves?: readonly Move[]
  starts?: (Spot | null)[]
  ends?: (Spot | null)[]
  viewer?: number
  slow?: number
}

function plan({ moves = [move()], starts, ends, viewer = ME, slow }: Run = {}): Flight[] {
  return planFlights({
    moves,
    viewer,
    starts: starts ?? moves.map(() => HAND),
    ends: ends ?? moves.map(() => STACK),
    batch: 1,
    ...(slow === undefined ? {} : { slow }),
  })
}

describe('the plan of a batch', () => {
  it('starts each flight one stagger after the one before, for one travel time', () => {
    const flights = plan({ moves: sequence(4) })
    expect(flights.map((flight) => flight.delay)).toEqual([0, 1, 2, 3].map((n) => n * MOVE_MS.stagger))
    for (const flight of flights) expect(flight.duration).toBe(MOVE_MS.travel)
  })

  it('flies at most MAX_MOVES and finishes inside the design budget', () => {
    const flights = plan({ moves: sequence(MAX_MOVES + 4) })
    expect(flights).toHaveLength(MAX_MOVES)
    const last = flights.at(-1)
    expect((last?.delay ?? 0) + (last?.duration ?? 0)).toBeLessThan(1700)
  })

  it('stretches the delay and the duration by the dev slow-down', () => {
    const [first, second] = plan({ moves: sequence(2), slow: 5 })
    expect([first?.delay, first?.duration]).toEqual([0, MOVE_MS.travel * 5])
    expect(second?.delay).toBe(MOVE_MS.stagger * 5)
  })

  it.each([
    ['its source', [null, HAND], [STACK, STACK]],
    ['its destination', [HAND, HAND], [null, STACK]],
    ['both', [null, HAND], [null, STACK]],
  ])(
    'leaves a move out whose %s is not on screen, and keeps the others in their places',
    (_name, starts, ends) => {
      const flights = plan({ moves: sequence(2), starts, ends })
      expect(flights.map((flight) => flight.move.order)).toEqual([1])
      expect(flights[0]?.delay).toBe(MOVE_MS.stagger)
    },
  )

  it('leaves out a move that would not go anywhere, as when both ends are the same place', () => {
    expect(plan({ starts: [HAND], ends: [HAND] })).toEqual([])
  })

  it('plans nothing for nothing, and for a move the page shows neither end of', () => {
    expect(plan({ moves: [] })).toEqual([])
    expect(plan({ starts: [null], ends: [null] })).toEqual([])
  })

  it('starts the second leg of a card when its first has landed, not a stagger after it started', () => {
    const resolved = move({
      kind: 'resolved',
      order: 1,
      from: { stack: true, object: 5 },
      to: { seat: ME, zone: 'battlefield', object: 5 },
    })
    const [cast, leg] = plan({ moves: [move(), resolved] })
    expect(cast?.delay).toBe(0)
    expect(leg?.delay).toBe(MOVE_MS.travel)
  })

  it('names each flight by its batch and its place in it', () => {
    const keys = plan({ moves: sequence(3) }).map((flight) => flight.key)
    expect(keys).toEqual(['1:0', '1:1', '1:2'])
  })
})

describe('what a picture shows', () => {
  const at = (zone: 'hand' | 'library' | 'graveyard', seat: number): FxPlace => ({ seat, zone })
  it.each<[string, Partial<Move>, boolean, boolean]>([
    ["a card of the viewer's hand", { from: at('hand', ME) }, false, false],
    ["a card of another seat's hand, once it is known", { from: at('hand', FOE) }, true, true],
    [
      "a card from another seat's library, once it is known",
      { kind: 'drawn', from: at('library', FOE) },
      true,
      true,
    ],
    [
      'a card of another seat that nobody identifies',
      { kind: 'drawn', look: null, from: at('library', FOE) },
      true,
      false,
    ],
    ['a card the viewer draws', { kind: 'drawn', from: at('library', ME) }, false, false],
    ["a card of another seat's graveyard", { kind: 'moved', from: at('graveyard', FOE) }, false, false],
  ])('is %s: face down %s, turning up %s', (_name, over, faceDown, reveal) => {
    const [flight] = plan({ moves: [move(over)] })
    expect([flight?.faceDown, flight?.reveal]).toEqual([faceDown, reveal])
  })

  it.each<[string, Partial<Move>, Spot, Spot, { width: number; height: number } | null]>([
    ['the card it starts on', {}, HAND, STACK, { width: 100, height: 140 }],
    [
      'the card it ends on',
      {},
      spot(10, 10, 20, 20),
      spot(500, 500, 80, 112, true),
      { width: 80, height: 112 },
    ],
    ['the reference card between two chips', {}, spot(10, 10, 20, 20), STACK, REFERENCE_CARD],
    ['no picture at all for an activated ability', { kind: 'activated' }, HAND, STACK, null],
    ['no picture at all for a trigger', { kind: 'triggered' }, HAND, STACK, null],
  ])('has the size of %s', (_name, over, from, to, size) => {
    const [flight] = plan({ moves: [move(over)], starts: [from], ends: [to] })
    expect(flight?.size).toEqual(size)
  })
})

/** The corner and scale a keyframe's transform says, and so where its picture's middle is. */
function pose(frame: Keyframe, size: { width: number; height: number }) {
  const [, left, top, scale] =
    /translate\((-?[\d.]+)px, (-?[\d.]+)px\) scale\(([\d.]+)\)/.exec(String(frame.transform)) ?? []
  const s = Number(scale)
  return { x: Number(left) + (size.width * s) / 2, y: Number(top) + (size.height * s) / 2, scale: s }
}

describe('the path of a leg', () => {
  const SIZE = { width: 100, height: 140 }
  const frames = (from: Spot, to: Spot, over: Partial<Move> = {}) => {
    const [flight] = plan({ moves: [move(over)], starts: [from], ends: [to] })
    if (flight === undefined) throw new Error('no flight')
    return keyframesOf(flight, flight.size ?? SIZE)
  }

  it('runs from the middle of the source to the middle of the destination, in a straight line', () => {
    const steps = frames(HAND, STACK).map((frame) => pose(frame, SIZE))
    expect(steps[0]).toMatchObject({ x: 150, y: 770 })
    expect(steps.at(-1)).toMatchObject({ x: 460, y: 316 })
    expect(steps[1]).toMatchObject({ x: 305, y: 543 })
    // Every step lies on the line from the first to the last.
    for (const step of steps)
      expect((step.x - 150) / (460 - 150)).toBeCloseTo((step.y - 770) / (316 - 770), 6)
  })

  it('leaves at the offsets of its steps, whole until it lands and gone when it has', () => {
    const steps = frames(HAND, STACK)
    expect(steps.map((frame) => frame.offset)).toEqual([0, 0.5, 0.85, 1])
    expect(steps.map((frame) => frame.opacity)).toEqual([1, 1, 1, 0])
  })

  it('takes the size of the card it starts on and the size that fits the chip it lands on', () => {
    const steps = frames(HAND, STACK).map((frame) => pose(frame, SIZE))
    expect(steps[0]?.scale).toBe(1)
    expect(steps.at(-1)?.scale).toBeCloseTo(32 / 140, 6)
  })

  it('lands exactly on a card it flies to', () => {
    const steps = frames(STACK, spot(600, 500, 80, 112, true), { kind: 'resolved' }).map((frame) =>
      pose(frame, { width: 80, height: 112 }),
    )
    expect(steps.at(-1)?.scale).toBe(1)
  })

  it('is whole halfway, even from and to a chip, so it can be read', () => {
    const steps = frames(spot(10, 10, 24, 16), STACK).map((frame) => pose(frame, REFERENCE_CARD))
    expect(steps[1]?.scale).toBe(1)
    expect(steps[0]?.scale).toBeGreaterThanOrEqual(MIN_SCALE)
    expect(steps[0]?.scale).toBeLessThan(0.5)
  })

  it('stays inside the window it flies in, whatever edge its source is at', () => {
    const bounds = { width: 390, height: 844 }
    const edge = spot(-10, 600, 40, 56, true)
    const [flight] = plan({ moves: [move()], starts: [edge], ends: [spot(380, 840, 30, 30)] })
    if (flight === undefined) throw new Error('no flight')
    for (const frame of keyframesOf(flight, SIZE, bounds)) {
      const [, left, top, scale] =
        /translate\((-?[\d.]+)px, (-?[\d.]+)px\) scale\(([\d.]+)\)/.exec(String(frame.transform)) ?? []
      expect(Number(left)).toBeGreaterThanOrEqual(0)
      expect(Number(top)).toBeGreaterThanOrEqual(0)
      expect(Number(left) + SIZE.width * Number(scale)).toBeLessThanOrEqual(bounds.width)
      expect(Number(top) + SIZE.height * Number(scale)).toBeLessThanOrEqual(bounds.height)
    }
  })

  it('keeps an ability chip at its own size all the way', () => {
    const chip = { width: 144, height: 32 }
    const [flight] = plan({ moves: [move({ kind: 'activated' })], starts: [HAND], ends: [STACK] })
    if (flight === undefined) throw new Error('no flight')
    const steps = keyframesOf(flight, chip).map((frame) => pose(frame, chip))
    expect(steps.map((step) => step.scale)).toEqual([1, 1, 1, 1])
    expect(steps[0]).toMatchObject({ x: 150, y: 770 })
  })
})

describe('the travel layer', () => {
  let page: HTMLElement
  let mode: FxMode
  let frames: ReturnType<typeof travelFrames>
  beforeEach(() => {
    page = document.createElement('main')
    document.body.replaceChildren(page)
    mode = 'full'
    frames = travelFrames()
  })
  const travel = (slow?: number) =>
    createTravel({
      root: page,
      mode: () => mode,
      frame: frames.scheduler,
      ...(slow === undefined ? {} : { slow }),
    })
  const handBox: TestBox = { x: 100, y: 700, width: 100, height: 140 }
  const chipBox: TestBox = { x: 400, y: 300, width: 120, height: 32 }

  /** The hand card of object 5, and a stack area; the chip of 5 is drawn later, as React draws it. */
  function table() {
    const hand = place(page, { 'data-object-id': '5' }, handBox, 'button')
    const stack = place(page, { 'data-fx-anchor': 'stack' }, { x: 0, y: 280, width: 900, height: 60 })
    return { hand, stack }
  }
  const flights = (layer: ReturnType<typeof travel>) => layer.store.getState().flights

  it('measures a source now and a destination on the next frame', () => {
    const layer = travel()
    const { hand, stack } = table()
    layer.launch([move()], ME)
    expect(frames.waiting()).toBe(1)
    expect(flights(layer)).toEqual([])
    // React draws the batch: the card has left the hand and its chip is on the stack.
    hand.remove()
    place(stack, { 'data-object-id': '5' }, chipBox, 'button')
    frames.flush()
    expect(flights(layer)).toHaveLength(1)
    expect(flights(layer)[0]?.from).toEqual({ box: handBox, card: true })
    expect(flights(layer)[0]?.to).toEqual({ box: chipBox, card: false })
  })

  it('falls back to the anchor of the zone for a card it cannot find', () => {
    const layer = travel()
    place(page, { 'data-fx-anchor': '0:hand' }, { x: 0, y: 650, width: 900, height: 200 })
    place(page, { 'data-fx-anchor': 'stack' }, { x: 0, y: 280, width: 900, height: 60 })
    layer.launch([move()], ME)
    frames.flush()
    expect(flights(layer)[0]?.from.card).toBe(false)
    expect(flights(layer)[0]?.from.box.y).toBe(650)
  })

  it('leaves out a move without a source or a destination, and plays no frame when no source is on screen', () => {
    const layer = travel()
    layer.launch([move()], ME)
    expect(frames.asked()).toBe(0)
    table()
    place(page, { 'data-fx-anchor': 'stack' }, { x: 0, y: 280, width: 900, height: 60 })
    layer.launch([move(), move({ object: 6, order: 1, from: { seat: FOE, zone: 'graveyard' } })], ME)
    frames.flush()
    expect(flights(layer).map((flight) => flight.move.object)).toEqual([5])
  })

  it.each<FxMode>(['reduced', 'off'])('flies nothing and measures nothing while the mode is %s', (value) => {
    mode = value
    const layer = travel()
    const { hand } = table()
    let measured = 0
    const box = hand.getBoundingClientRect
    hand.getBoundingClientRect = () => {
      measured += 1
      return box.call(hand)
    }
    layer.launch([move()], ME)
    expect([frames.asked(), measured, flights(layer)]).toEqual([0, 0, []])
  })

  it('reads the mode at every batch', () => {
    const layer = travel()
    table()
    mode = 'reduced'
    layer.launch([move()], ME)
    mode = 'full'
    layer.launch([move()], ME)
    expect(frames.waiting()).toBe(1)
  })

  it('stops what flies and what waits when a newer batch with moves arrives', () => {
    const layer = travel()
    table()
    layer.launch([move()], ME)
    frames.flush()
    expect(flights(layer)).toHaveLength(1)
    layer.launch([move()], ME)
    expect(flights(layer)).toEqual([])
    expect(frames.waiting()).toBe(1)
    layer.launch([move({ object: 6, from: { seat: ME, zone: 'hand', object: 5 } })], ME)
    expect(frames.waiting()).toBe(1)
    frames.flush()
    expect(flights(layer).map((flight) => flight.key)).toEqual(['3:0'])
  })

  it('leaves what flies alone for a batch with no moves', () => {
    const layer = travel()
    table()
    layer.launch([move()], ME)
    frames.flush()
    layer.launch([], ME)
    expect(flights(layer)).toHaveLength(1)
  })

  it('takes a flight off when it has landed, and all of them when cleared', () => {
    const layer = travel()
    table()
    layer.launch(
      sequence(2).map((one) => ({ ...one, from: { seat: ME, zone: 'hand' as const, object: 5 } })),
      ME,
    )
    frames.flush()
    const [first, second] = flights(layer)
    expect(flights(layer)).toHaveLength(2)
    layer.done(first?.key ?? '')
    expect(flights(layer).map((flight) => flight.key)).toEqual([second?.key])
    layer.done('nothing')
    layer.launch([move()], ME)
    layer.clear()
    expect([flights(layer), frames.waiting()]).toEqual([[], 0])
  })

  it('stretches by the slow-down it is given', () => {
    const layer = travel(10)
    table()
    layer.launch([move()], ME)
    frames.flush()
    expect(flights(layer)[0]?.duration).toBe(MOVE_MS.travel * 10)
  })
})
