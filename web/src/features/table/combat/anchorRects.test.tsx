// Goal: the anchors of the overlay are measured when they first appear and again, once per frame, whenever the table could have moved (the table or the window resized, a scroll inside the table, cards entering or leaving, a card turning, a transition ending), however many of those happen within a frame; never for a render that changes nothing; and not at all once the overlay is gone.
import { act, render, screen } from '@testing-library/react'
import { useRef } from 'react'
import type { ReactElement } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { fakeFrames } from '@/test-utils/frames'
import type { FakeFrames } from '@/test-utils/frames'
import { anchorKey } from './plan'
import type { Anchor } from './plan'
import { useAnchorRects } from './rects'
import type { RectMap, RectProvider } from './rects'

const card = (id: number): Anchor => ({ kind: 'card', id })
const seat = (n: number): Anchor => ({ kind: 'seat', seat: n })

/** A provider that gives every anchor a box that moves each time the test changes `shift`. */
function provider() {
  const state = { shift: 0 }
  const measure = vi.fn(
    (anchors: readonly Anchor[], _origin: Element): RectMap =>
      new Map(
        anchors.map((anchor, index) => [
          anchorKey(anchor),
          { x: state.shift, y: index, width: 10, height: 10 },
        ]),
      ),
  )
  const given: RectProvider = { measure }
  return { given, measure, state }
}

const renders = vi.fn()

function Probe({
  anchors,
  given,
  frames,
}: {
  anchors: readonly Anchor[]
  given: RectProvider
  frames: FakeFrames['frames']
}) {
  const origin = useRef<SVGSVGElement>(null)
  const rects = useAnchorRects(origin, anchors, given, frames)
  renders()
  // The boxes are printed inside the svg, as the overlay draws its lines there: that is not a change of the table.
  return (
    <div data-testid="table">
      <svg ref={origin}>
        <desc data-testid="shown">{[...rects].map(([key, rect]) => `${key}@${rect.x}`).join(' ')}</desc>
      </svg>
      <button data-testid="card" />
    </div>
  )
}

function mount(anchors: readonly Anchor[] = [card(1), seat(0)]) {
  const fake = fakeFrames()
  const p = provider()
  renders.mockClear()
  const element = (next: readonly Anchor[]): ReactElement => (
    <Probe anchors={next} given={p.given} frames={fake.frames} />
  )
  const view = render(element(anchors))
  return { ...fake, ...p, view, again: (next: readonly Anchor[]) => view.rerender(element(next)) }
}

const table = (): HTMLElement => screen.getByTestId('table')
const shown = (): string => screen.getByTestId('shown').textContent ?? ''

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('measuring the anchors', () => {
  it('measures once as the overlay appears, before anything is painted, and shows the boxes', () => {
    const { measure } = mount()
    expect(measure).toHaveBeenCalledTimes(1)
    expect(measure.mock.calls[0]?.[0].map(anchorKey)).toEqual(['card:1', 'seat:0'])
    expect(measure.mock.calls[0]?.[1]).toBe(table().querySelector('svg'))
    expect(shown()).toBe('card:1@0 seat:0@0')
  })

  it('does not measure again for a render with the same anchors, even in a new array', () => {
    const { measure, again, waiting } = mount()
    again([card(1), seat(0)])
    again([card(1), seat(0)])
    expect(measure).toHaveBeenCalledTimes(1)
    expect(waiting()).toBe(0)
  })

  it('measures again, at once, when the anchors differ', () => {
    const { measure, again } = mount()
    again([card(1), card(2), seat(0)])
    expect(measure).toHaveBeenCalledTimes(2)
    expect(measure.mock.calls[1]?.[0].map(anchorKey)).toEqual(['card:1', 'card:2', 'seat:0'])
    expect(shown()).toBe('card:1@0 card:2@0 seat:0@0')
  })

  it('draws again only when a box moved: a measurement that finds the same boxes changes nothing', () => {
    const { flush, measure, state } = mount()
    const before = renders.mock.calls.length
    act(() => {
      window.dispatchEvent(new Event('resize'))
      flush()
    })
    expect(measure).toHaveBeenCalledTimes(2)
    expect(renders.mock.calls.length).toBe(before)
    state.shift = 25
    act(() => {
      window.dispatchEvent(new Event('resize'))
      flush()
    })
    expect(renders.mock.calls.length).toBe(before + 1)
    expect(shown()).toBe('card:1@25 seat:0@25')
  })
})

describe('what makes it measure again', () => {
  it.each<[string, () => void | Promise<void>]>([
    ['the window resizing', () => void window.dispatchEvent(new Event('resize'))],
    [
      'a scroll in a row of the table, which does not bubble',
      () => void screen.getByTestId('card').dispatchEvent(new Event('scroll', { bubbles: false })),
    ],
    [
      'a transition ending in the table',
      () => void screen.getByTestId('card').dispatchEvent(new Event('transitionend', { bubbles: true })),
    ],
    ['a card entering the table', () => void table().append(document.createElement('button'))],
    ['a card leaving the table', () => void screen.getByTestId('card').remove()],
    ['a card turning (tapped)', () => void screen.getByTestId('card').setAttribute('data-tapped', 'true')],
    ['a card changing size', () => void screen.getByTestId('card').setAttribute('data-size', 'compact')],
  ])('%s', async (_name, change) => {
    const { measure, waiting, flush } = mount()
    await act(async () => {
      await change()
    })
    expect(waiting()).toBe(1)
    expect(measure).toHaveBeenCalledTimes(1)
    act(() => flush())
    expect(measure).toHaveBeenCalledTimes(2)
  })

  it('settles after it appears: no frame is waiting once the first boxes are drawn', async () => {
    const { waiting } = mount()
    await act(async () => undefined)
    expect(waiting()).toBe(0)
  })

  it('ignores its own drawing: lines added inside the overlay move no anchor', async () => {
    const { measure, waiting } = mount()
    await act(async () => {
      table().querySelector('svg')?.append(document.createElementNS('http://www.w3.org/2000/svg', 'path'))
    })
    expect(waiting()).toBe(0)
    expect(measure).toHaveBeenCalledTimes(1)
  })

  it('ignores what cannot move a box: other attributes of a card, and a change outside the table', async () => {
    const { measure, waiting } = mount()
    await act(async () => {
      screen.getByTestId('card').setAttribute('data-highlight', 'legal')
      document.body.append(document.createElement('div'))
    })
    expect(waiting()).toBe(0)
    expect(measure).toHaveBeenCalledTimes(1)
  })

  it('asks for one frame, and measures once, however many things happen before it', async () => {
    const { measure, waiting, flush, asked } = mount()
    await act(async () => {
      for (let i = 0; i < 10; i += 1) window.dispatchEvent(new Event('resize'))
      screen.getByTestId('card').dispatchEvent(new Event('transitionend', { bubbles: true }))
      table().append(document.createElement('span'), document.createElement('span'))
    })
    expect(asked()).toBe(1)
    expect(waiting()).toBe(1)
    act(() => flush())
    expect(measure).toHaveBeenCalledTimes(2)
  })

  it('takes the next change in a new frame, once the first has run', async () => {
    const { measure, flush, asked } = mount()
    await act(async () => void window.dispatchEvent(new Event('resize')))
    act(() => flush())
    await act(async () => void window.dispatchEvent(new Event('resize')))
    expect(asked()).toBe(2)
    act(() => flush())
    expect(measure).toHaveBeenCalledTimes(3)
  })

  it('also listens to the size of the table when the browser has a ResizeObserver', () => {
    const observed: Element[] = []
    let notify: () => void = () => undefined
    class Observer {
      constructor(callback: () => void) {
        notify = callback
      }
      observe(element: Element): void {
        observed.push(element)
      }
      disconnect(): void {
        observed.length = 0
      }
    }
    vi.stubGlobal('ResizeObserver', Observer)
    const { measure, waiting, flush, view } = mount()
    expect(observed).toEqual([table()])
    act(() => notify())
    expect(waiting()).toBe(1)
    act(() => flush())
    expect(measure).toHaveBeenCalledTimes(2)
    view.unmount()
    expect(observed).toEqual([])
  })
})

describe('when the overlay goes', () => {
  it('stops listening and drops a frame that is waiting', async () => {
    const { measure, waiting, view } = mount()
    await act(async () => void window.dispatchEvent(new Event('resize')))
    expect(waiting()).toBe(1)
    view.unmount()
    expect(waiting()).toBe(0)
    window.dispatchEvent(new Event('resize'))
    await act(async () => undefined)
    expect(waiting()).toBe(0)
    expect(measure).toHaveBeenCalledTimes(1)
  })

  it('is not asked to measure by a change that comes after it, such as fonts arriving late', async () => {
    let arrive: () => void = () => undefined
    const fonts = { ready: new Promise<void>((resolve) => (arrive = resolve)) }
    Object.defineProperty(document, 'fonts', { value: fonts, configurable: true })
    try {
      const { measure, waiting, view } = mount()
      view.unmount()
      arrive()
      await act(async () => undefined)
      expect(waiting()).toBe(0)
      expect(measure).toHaveBeenCalledTimes(1)
    } finally {
      Reflect.deleteProperty(document, 'fonts')
    }
  })

  it('measures when fonts arrive while it is there', async () => {
    let arrive: () => void = () => undefined
    const fonts = { ready: new Promise<void>((resolve) => (arrive = resolve)) }
    Object.defineProperty(document, 'fonts', { value: fonts, configurable: true })
    try {
      const { measure, flush } = mount()
      arrive()
      await act(async () => undefined)
      act(() => flush())
      expect(measure).toHaveBeenCalledTimes(2)
    } finally {
      Reflect.deleteProperty(document, 'fonts')
    }
  })
})
