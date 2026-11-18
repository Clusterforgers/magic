// Goal: a card added in the builder flies from the pressed search row into its line of the deck list (or to the Deck switch while the list is out of view) and the line beats as it lands; a line removed or moved away slides out as a ghost while the list closes up at once; nothing moves under reduced motion and nothing waits for a flight.
import { screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { copies, deckLine, DURESS, renderBuilder, searchBox } from './renderBuilder'
import { flyWaiting, noteArrival, noteLeaving, TRAVEL_MS } from './deckTravel'

type Animate = (keyframes: Keyframe[], options: KeyframeAnimationOptions) => Animation
const calls: { element: Element; keyframes: Keyframe[]; options: KeyframeAnimationOptions }[] = []
const finishers: (() => void)[] = []

/** Every element has a box of `size` at its place in the list (by its order in the page), and animates on call. */
function stubLayout(): void {
  vi.spyOn(Element.prototype, 'getBoundingClientRect').mockImplementation(function (this: Element) {
    const at = [...document.querySelectorAll('*')].indexOf(this)
    return DOMRect.fromRect({ x: 10, y: at * 2, width: 200, height: 32 })
  })
  const animate: Animate = function (this: Element, keyframes, options) {
    calls.push({ element: this, keyframes, options })
    let finish: () => void = () => undefined
    const finished = new Promise<Animation>((resolve) => {
      finish = () => resolve({} as Animation)
    })
    finishers.push(finish)
    return { finished } as Animation
  }
  Element.prototype.animate = animate as Element['animate']
}

function reduced(on: boolean): void {
  vi.stubGlobal('matchMedia', (query: string) => ({
    matches: on && query.includes('reduce'),
    media: query,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
  }))
}

beforeEach(() => {
  calls.length = 0
  finishers.length = 0
  reduced(false)
  stubLayout()
})
afterEach(() => {
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
  // jsdom has no animate of its own.
  Reflect.deleteProperty(Element.prototype, 'animate')
  for (const left of document.querySelectorAll('[data-slot="deck-flier"], [data-slot="deck-ghost"]'))
    left.remove()
})

const flier = (): HTMLElement | null => document.querySelector('[data-slot="deck-flier"]')
const ghost = (): HTMLElement | null => document.querySelector('[data-slot="deck-ghost"]')

describe('the flight of a card added', () => {
  it('starts at the pressed row, dips, arcs and lands on the line, which beats as it lands', () => {
    document.body.innerHTML =
      '<ul><li data-card="c1">Bolt</li></ul><ul><li data-line="main:c1">Bolt</li></ul>'
    noteArrival('main', 'c1', 'Lightning Bolt', document.querySelector('li[data-card="c1"]'))
    flyWaiting()
    expect(flier()).toHaveTextContent('Lightning Bolt')
    expect(flier()).toHaveAttribute('aria-hidden', 'true')
    const [flight, beat] = calls
    expect(flight?.element).toBe(flier())
    expect(flight?.options.duration).toBe(TRAVEL_MS)
    expect(flight?.keyframes).toHaveLength(4)
    expect(String(flight?.keyframes[1]?.transform)).toContain('scale(0.94)')
    expect(beat?.element).toBe(document.querySelector('[data-line="main:c1"]'))
    expect(Number(beat?.options.delay)).toBeLessThan(TRAVEL_MS)
    // The flier goes once its flight ends.
    finishers[0]?.()
    return Promise.resolve().then(() => expect(flier()).toBeNull())
  })

  it('lands on the Deck switch while the list is out of view', () => {
    document.body.innerHTML =
      '<ul><li data-card="c1">Bolt</li></ul><button data-travel-target="deck">Deck</button><ul hidden><li data-line="main:c1">Bolt</li></ul>'
    vi.spyOn(Element.prototype, 'getBoundingClientRect').mockImplementation(function (this: Element) {
      return this.closest('[hidden]') === null
        ? DOMRect.fromRect({ x: 10, y: 10, width: 120, height: 32 })
        : DOMRect.fromRect({ x: 0, y: 0, width: 0, height: 0 })
    })
    noteArrival('main', 'c1', 'Lightning Bolt', document.querySelector('li[data-card="c1"]'))
    flyWaiting()
    expect(flier()).not.toBeNull()
    expect(calls.map((call) => call.element)).toContain(document.querySelector('[data-travel-target="deck"]'))
  })

  it('flies once: a second render sends nothing', () => {
    document.body.innerHTML =
      '<ul><li data-card="c1">Bolt</li></ul><ul><li data-line="main:c1">Bolt</li></ul>'
    noteArrival('main', 'c1', 'Lightning Bolt', document.querySelector('li[data-card="c1"]'))
    flyWaiting()
    flyWaiting()
    expect(document.querySelectorAll('[data-slot="deck-flier"]')).toHaveLength(1)
  })

  it('does not move under reduced motion', () => {
    reduced(true)
    document.body.innerHTML =
      '<ul><li data-card="c1">Bolt</li></ul><ul><li data-line="main:c1">Bolt</li></ul>'
    noteArrival('main', 'c1', 'Lightning Bolt', document.querySelector('li[data-card="c1"]'))
    flyWaiting()
    noteLeaving('main', 'c1')
    expect(flier()).toBeNull()
    expect(ghost()).toBeNull()
    expect(calls).toEqual([])
  })
})

describe('a line that leaves', () => {
  it('leaves a ghost at its place that slides out and goes, hidden from assistive technology', async () => {
    document.body.innerHTML = '<ul><li data-line="main:c1"><button>Remove</button> Bolt</li></ul>'
    noteLeaving('main', 'c1')
    const left = ghost()
    expect(left).not.toBeNull()
    expect(left).toHaveAttribute('aria-hidden', 'true')
    expect(left?.inert).toBe(true)
    expect(left).not.toHaveAttribute('data-line')
    expect(left?.style.position).toBe('fixed')
    expect(String(calls[0]?.keyframes[1]?.opacity)).toBe('0')
    finishers[0]?.()
    await Promise.resolve()
    expect(ghost()).toBeNull()
  })
})

describe('in the builder', () => {
  it('flies a card added from the search into the deck list, which holds it at once', async () => {
    const { user } = await renderBuilder()
    await user.type(searchBox(), 'chain')
    await user.click(await screen.findByRole('button', { name: 'Add Chain Lightning to main deck' }))
    expect(copies('Chain Lightning')).toBe(1)
    expect(flier()).toHaveTextContent('Chain Lightning')
    expect(calls.some((call) => call.element === deckLine('Chain Lightning'))).toBe(true)
  })

  it('lets a removed line slide out while the list drops it at once', async () => {
    const { user } = await renderBuilder()
    await user.click(screen.getByRole('button', { name: 'Remove Duress from the sideboard' }))
    expect(ghost()).toHaveTextContent('Duress')
    expect(document.querySelector(`[data-line="sideboard:${DURESS.oracle_id}"]`)).toBeNull()
  })
})
