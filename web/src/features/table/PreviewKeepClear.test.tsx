// Goal: the table's card preview keeps its full size and moves off the elements marked `data-keep-clear` (the decision's home, its sentence and answers, the hand dock, the controls) to the side they leave free; each mark carries its rank, the answers measured button by button with the primary one marked, and the strip is marked only while a decision is owed.
import { screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { DEV_PROMPTS } from '@/features/table/dev/prompts'
import { buildFixture } from '@/features/table/dev/fixture'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { keepClearBoxes } from './keepClear'
import { anchorOf } from './preview'
import { previewFit } from './previewFit'

const KEPT = '[data-keep-clear]'

/** A marked element that reads as a box in the window, as jsdom measures nothing. */
function mark(left: number, top: number, right: number, bottom: number): HTMLElement {
  const element = document.createElement('div')
  element.setAttribute('data-keep-clear', '')
  element.getBoundingClientRect = () =>
    ({
      left,
      top,
      right,
      bottom,
      x: left,
      y: top,
      width: right - left,
      height: bottom - top,
      toJSON: () => ({}),
    }) satisfies DOMRect
  document.body.append(element)
  return element
}

afterEach(() => {
  for (const element of document.querySelectorAll('body > [data-keep-clear]')) element.remove()
})

const firstCard = (container: HTMLElement) =>
  must(container.querySelector<HTMLElement>('[data-battlefield-seat="1"] button[data-object-id]'))

describe('the preview beside a strip', () => {
  it('docks on the right of a card at the left without a strip in the way', async () => {
    const { user, container } = renderTable({ seats: 2 })
    await user.hover(firstCard(container))
    expect(screen.getByTestId('card-preview')).toHaveAttribute('data-dock', 'right')
  })

  it('docks on the left at its full width when the strip stands where the right panel would', async () => {
    // The window is 1024 px wide: the right panel is its last 396 px, where the strip stands; the left one is clear.
    mark(620, 100, 1000, 500)
    const { user, container } = renderTable({ seats: 2 })
    const card = firstCard(container)
    await user.hover(card)
    const preview = screen.getByTestId('card-preview')
    expect(preview).toHaveAttribute('data-dock', 'left')
    const full = previewFit(anchorOf(card), { width: window.innerWidth, height: window.innerHeight })
    expect(preview.style.width).toBe(`${full.width}px`)
    expect(preview).toHaveAttribute('data-picture', String(full.picture))
  })
})

describe('the measured boxes', () => {
  it('measures each answer button on its own and marks the primary one', () => {
    const answers = mark(600, 600, 1000, 640)
    answers.setAttribute('data-keep-clear', 'answers')
    answers.setAttribute('data-slot', 'prompt-actions')
    for (const [left, right] of [
      [900, 990],
      [800, 890],
    ] as const) {
      const button = document.createElement('button')
      button.getBoundingClientRect = () => new DOMRect(left, 600, right - left, 40)
      answers.append(button)
    }
    const text = mark(200, 600, 500, 620)
    text.setAttribute('data-keep-clear', 'text')
    const kinds = keepClearBoxes().map((box) => [box.left, box.kind])
    expect(kinds).toEqual([
      [900, 'primary'],
      [800, 'answer'],
      [200, 'text'],
    ])
  })

  it('takes the pass controls primary button as the primary answer', () => {
    const controls = mark(1000, 800, 1200, 860)
    controls.setAttribute('data-keep-clear', 'answers')
    const stops = document.createElement('button')
    stops.getBoundingClientRect = () => new DOMRect(1000, 800, 60, 40)
    const primary = document.createElement('button')
    primary.setAttribute('data-slot', 'primary-action')
    primary.getBoundingClientRect = () => new DOMRect(1080, 800, 100, 40)
    controls.append(stops, primary)
    expect(keepClearBoxes().map((box) => box.kind)).toEqual(['answer', 'primary'])
  })
})

describe('the home mark', () => {
  const sheet = () => must(document.querySelector('[data-slot="decision-line"]'))

  it('is on the home while the seat owes a decision', () => {
    const fixture = buildFixture({ seats: 2 })
    const pending = must(DEV_PROMPTS['priority'])(fixture)
    renderTable({ fixture, state: (state) => ({ ...state, pending }) })
    expect(sheet()).toHaveAttribute('data-keep-clear')
  })

  it.each([
    'priority',
    'attack',
    'block',
    'mulligan',
    'cards',
    'targets',
    'mode',
    'x',
    'option',
    'yesno',
    'discard',
    'divide',
  ])(
    'marks the home soft, the sentence of a %s prompt text and its buttons answers, so a preview covers them last',
    (prompt) => {
      const fixture = buildFixture({ seats: 2 })
      const pending = must(DEV_PROMPTS[prompt])(fixture)
      renderTable({ fixture, state: (state) => ({ ...state, pending }) })
      // A floating opening hand holds its sentence and buttons in the panel over the table, not in a home.
      const home = document.querySelector('[data-slot="decision-line"]')
      if (prompt !== 'mulligan') expect(home).toHaveAttribute('data-keep-clear', 'soft')
      const sentences = [...document.querySelectorAll('[data-slot="prompt-text"]')]
      const answers = [...document.querySelectorAll('[data-slot="prompt-actions"]')]
      expect(sentences.length + answers.length).toBeGreaterThan(0)
      for (const element of sentences) expect(element).toHaveAttribute('data-keep-clear', 'text')
      for (const element of answers) expect(element).toHaveAttribute('data-keep-clear', 'answers')
    },
  )

  it('is nowhere while nothing is owed', () => {
    renderTable({ seats: 2, state: (state) => ({ ...state, pending: null }) })
    expect(document.querySelector('[data-slot="decision-line"]')).toBeNull()
  })

  it('is on the hand dock and the controls', () => {
    renderTable({ seats: 2 })
    expect(document.querySelector('[data-region="hand"]')).toHaveAttribute('data-keep-clear')
    expect(document.querySelector('[data-region="controls"]')).toHaveAttribute('data-keep-clear', 'answers')
    expect(document.querySelectorAll(KEPT).length).toBeGreaterThanOrEqual(2)
  })
})
