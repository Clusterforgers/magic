// Goal: the table's card preview keeps off the elements marked `data-keep-clear` (the strip while it holds a decision, the hand dock, the controls): it docks to the side the strip leaves free, and the strip is marked only while a decision is owed.
import { screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { DEV_PROMPTS } from '@/features/table/dev/prompts'
import { buildFixture } from '@/features/table/dev/fixture'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'

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

  it('docks on the left when the strip stands where the right panel would', async () => {
    // The window is 1024 px wide: the right panel is the last 300 px; the strip reaches over them.
    mark(300, 100, 900, 500)
    const { user, container } = renderTable({ seats: 2 })
    await user.hover(firstCard(container))
    const preview = screen.getByTestId('card-preview')
    expect(preview).toHaveAttribute('data-dock', 'left')
    expect(Number(preview.style.width.replace('px', ''))).toBeLessThanOrEqual(300 - 12)
  })
})

describe('the strip mark', () => {
  const sheet = () => must(document.querySelector('[data-slot="shared-sheet"]'))

  it('is on the strip while the seat owes a decision', () => {
    const fixture = buildFixture({ seats: 2 })
    const pending = must(DEV_PROMPTS['priority'])(fixture)
    renderTable({ fixture, state: (state) => ({ ...state, pending }) })
    expect(sheet()).toHaveAttribute('data-keep-clear')
  })

  it.each(['priority', 'attack', 'mulligan'])(
    'marks the strip soft and the buttons of a %s prompt firm, so a readable preview may cover only the sentence',
    (prompt) => {
      const fixture = buildFixture({ seats: 2 })
      const pending = must(DEV_PROMPTS[prompt])(fixture)
      renderTable({ fixture, state: (state) => ({ ...state, pending }) })
      expect(sheet()).toHaveAttribute('data-keep-clear', 'soft')
      for (const actions of sheet().querySelectorAll('[data-slot="prompt-actions"]'))
        expect(actions).toHaveAttribute('data-keep-clear', '')
    },
  )

  it('is not on the strip while nothing is owed', () => {
    renderTable({ seats: 2, state: (state) => ({ ...state, pending: null }) })
    expect(sheet()).not.toHaveAttribute('data-keep-clear')
  })

  it('is on the hand dock and the controls', () => {
    renderTable({ seats: 2 })
    expect(document.querySelector('[data-region="hand"]')).toHaveAttribute('data-keep-clear')
    expect(document.querySelector('[data-region="controls"]')).toHaveAttribute('data-keep-clear')
    expect(document.querySelectorAll(KEPT).length).toBeGreaterThanOrEqual(2)
  })
})
