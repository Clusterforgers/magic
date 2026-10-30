// Goal: the layer of flying cards starts one animation per move with the plan's keyframes and timing, shows a card back that turns up for a card cast from another seat's hand and a named chip for an ability, takes a flier off when it lands, ends every animation at once when a newer batch arrives or the table goes, draws nothing under reduced motion or without the Web Animations API, and is hidden from assistive technology.
import { act, cleanup, render } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { card } from '@/test-utils/cards'
import { fakeAnimate, place, travelFrames } from '@/test-utils/fxDom'
import { FxContext } from './hooks'
import { FxTravel } from './FxTravel'
import { MOVE_MS } from './moves'
import type { Move } from './moves'
import { createFx } from './store'
import type { FxMode } from './store'
import { EASE_OUT, keyframesOf } from './travel'

const ME = 0
const FOE = 1

let page: HTMLElement
let mode: FxMode
let animate: ReturnType<typeof fakeAnimate>

beforeEach(() => {
  page = document.createElement('main')
  document.body.replaceChildren(page)
  mode = 'full'
  animate = fakeAnimate()
})
afterEach(() => {
  cleanup()
  animate.restore()
})

function setup() {
  const frames = travelFrames()
  const fx = createFx({ mode: () => mode, travel: { root: page, frame: frames.scheduler } })
  const view = render(
    <FxContext value={fx}>
      <FxTravel />
    </FxContext>,
  )
  const launch = (moves: readonly Move[]): void =>
    act(() => {
      fx.travel.launch(moves, ME)
      frames.flush()
    })
  return { fx, view, launch }
}

const cast = (over: Partial<Move> = {}): Move => ({
  kind: 'cast',
  object: 5,
  // Another controller than the viewer keeps the viewer's own spotlight (`TableSpotlight.test.tsx`) out of these fliers.
  look: card({ id: 5, zone: 'Stack', name: 'Lightning Bolt', controller: FOE }),
  from: { seat: ME, zone: 'hand', object: 5 },
  to: { stack: true, object: 5 },
  order: 0,
  targets: [],
  spotlight: false,
  ...over,
})

/** The viewer's hand card 5 and the stack's area, where a cast starts and ends. */
function table() {
  place(page, { 'data-object-id': '5' }, { x: 100, y: 700, width: 100, height: 140 }, 'button')
  place(page, { 'data-fx-anchor': 'stack' }, { x: 0, y: 280, width: 900, height: 60 })
}
const fliers = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>('[data-fx-flier]')]

describe('a card that flies', () => {
  it('starts one animation with the keyframes and the timing of its flight', () => {
    table()
    const { fx, launch } = setup()
    launch([cast()])
    expect(fliers().map((flier) => flier.dataset.fxFlier)).toEqual(['cast'])
    expect(animate.started).toHaveLength(1)
    const [flight] = fx.travel.store.getState().flights
    const [started] = animate.started
    if (flight?.size == null) throw new Error('no flight')
    expect(started?.keyframes).toEqual(
      keyframesOf(flight, flight.size, { width: window.innerWidth, height: window.innerHeight }),
    )
    expect(started?.timing).toEqual({ duration: MOVE_MS.travel, delay: 0, easing: EASE_OUT, fill: 'both' })
    expect(started?.element).toBe(fliers()[0])
  })

  it('stands in a layer that assistive technology and the pointer do not see', () => {
    table()
    setup().launch([cast()])
    const layer = document.querySelector('[data-slot="fx-travel"]')
    expect(layer?.getAttribute('aria-hidden')).toBe('true')
    expect(layer?.querySelectorAll('button, a, input, [tabindex], [data-object-id]')).toHaveLength(0)
  })

  it("draws the card itself, face up, when it is the viewer's", () => {
    table()
    setup().launch([cast()])
    expect(fliers()[0]?.textContent).toContain('Lightning Bolt')
    expect(fliers()[0]?.querySelector('[data-slot="card-back"]')).toBeNull()
  })

  it("draws a card back for a card from another seat's hand and turns it up on its way in", () => {
    table()
    place(page, { 'data-seat': String(FOE) }, { x: 0, y: 0, width: 168, height: 300 })
    const { launch } = setup()
    launch([cast({ from: { seat: FOE, zone: 'hand', object: 5 } })])
    expect(fliers()[0]?.querySelector('[data-slot="card-back"]')).not.toBeNull()
    expect(fliers()[0]?.textContent).toContain('Lightning Bolt')
    expect(animate.started).toHaveLength(2)
    expect(animate.started[1]?.keyframes.map((frame) => frame.opacity)).toEqual([0, 0, 1, 1])
  })

  it('draws only a back for a card nobody identifies, and never turns it up', () => {
    const pod = place(page, { 'data-seat': String(FOE) }, { x: 0, y: 0, width: 168, height: 300 })
    place(pod, { 'data-zone': 'library' }, { x: 10, y: 80, width: 30, height: 20 }, 'span')
    place(pod, { 'data-zone': 'hand' }, { x: 60, y: 80, width: 30, height: 20 }, 'span')
    const { launch } = setup()
    launch([
      cast({
        kind: 'drawn',
        object: null,
        look: null,
        from: { seat: FOE, zone: 'library' },
        to: { seat: FOE, zone: 'hand' },
      }),
    ])
    expect(fliers()[0]?.querySelector('[data-slot="card-back"]')).not.toBeNull()
    expect(animate.started).toHaveLength(1)
  })

  it("draws an ability as a chip with its source's name, at the size the stylesheet gives it", () => {
    table()
    const { launch } = setup()
    launch([cast({ kind: 'activated', look: card({ id: 9, name: 'Gravedigger' }) })])
    const [flier] = fliers()
    expect(flier?.hasAttribute('data-chip')).toBe(true)
    expect(flier?.textContent).toBe('Gravedigger')
    expect(animate.started).toHaveLength(1)
  })

  it('staggers the fliers of a batch', () => {
    table()
    const { launch } = setup()
    launch([cast(), cast({ object: 6, order: 1, from: { seat: ME, zone: 'hand', object: 5 } })])
    expect(animate.started.map((one) => one.timing.delay)).toEqual([0, MOVE_MS.stagger])
  })
})

describe('the end of a flight', () => {
  it('takes the flier off when it lands, and the layer with the last one', () => {
    table()
    const { launch } = setup()
    launch([cast()])
    act(() => animate.started[0]?.finish())
    expect(fliers()).toEqual([])
    expect(document.querySelector('[data-slot="fx-travel"]')).toBeNull()
  })

  it('ends every animation at once when a newer batch arrives, and flies the new one', () => {
    table()
    const { launch } = setup()
    launch([cast()])
    launch([cast({ object: 6 })])
    expect(animate.started).toHaveLength(2)
    expect(animate.started[0]?.cancel).toHaveBeenCalledTimes(1)
    expect(animate.started[1]?.cancel).not.toHaveBeenCalled()
    expect(fliers()).toHaveLength(1)
  })

  it('ends the animation of a card with its reveal when it is cancelled', () => {
    table()
    place(page, { 'data-seat': String(FOE) }, { x: 0, y: 0, width: 168, height: 300 })
    const { launch } = setup()
    launch([cast({ from: { seat: FOE, zone: 'hand', object: 5 } })])
    launch([cast({ object: 6 })])
    expect(animate.started.slice(0, 2).map((one) => one.cancel.mock.calls.length)).toEqual([1, 1])
  })

  it('ends every animation when the table goes', () => {
    table()
    const { launch, view } = setup()
    launch([cast()])
    view.unmount()
    expect(animate.live()).toEqual([])
    expect(document.querySelector('[data-slot="fx-travel"]')).toBeNull()
  })

  it('ends every animation when the layer is cleared', () => {
    table()
    const { launch, fx } = setup()
    launch([cast()])
    act(() => fx.clear())
    expect(animate.live()).toEqual([])
    expect(fliers()).toEqual([])
  })
})

describe('where nothing flies', () => {
  it.each<FxMode>(['reduced', 'off'])('draws nothing and starts nothing in %s mode', (value) => {
    mode = value
    table()
    setup().launch([cast()])
    expect(fliers()).toEqual([])
    expect(animate.started).toEqual([])
  })

  it('drops a flight quietly when the browser has no Web Animations API', () => {
    animate.restore()
    table()
    const { launch, fx } = setup()
    launch([cast()])
    expect(fliers()).toEqual([])
    expect(fx.travel.store.getState().flights).toEqual([])
  })

  it('draws nothing while nothing flies', () => {
    setup()
    expect(document.querySelector('[data-slot="fx-travel"]')).toBeNull()
  })
})
