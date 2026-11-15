// Goal: a pinned table preview closes on a press outside its panel and outside the card it shows (mouse or touch) and on Escape, lets that press reach its target, switches to another card that is clicked, and a hovered preview ignores presses.
import { fireEvent, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'

function mount() {
  const app = renderTable({ seats: 2 })
  const cards = [
    ...app.container.querySelectorAll<HTMLElement>('[data-battlefield-seat="1"] button[data-object-id]'),
  ]
  expect(cards.length).toBeGreaterThanOrEqual(2)
  return { ...app, first: must(cards[0]), second: must(cards[1]) }
}

const preview = () => screen.queryByTestId('card-preview')
const named = () => preview()?.querySelector('p.font-semibold')?.textContent

describe('dismissing a pinned preview', () => {
  it.each([
    ['mouse', 'mouse'],
    ['touch', 'touch'],
  ] as const)('closes on a %s press on the table background', async (_, pointerType) => {
    const { user, first } = mount()
    await user.click(first)
    expect(preview()).toHaveAttribute('data-pinned', 'true')
    fireEvent.pointerDown(document.body, { pointerType })
    expect(preview()).toBeNull()
  })

  it('stays on a press inside the panel', async () => {
    const { user, first } = mount()
    await user.click(first)
    const panel = must(preview())
    await user.click(must(panel.querySelector('p.font-semibold')))
    fireEvent.pointerDown(panel)
    expect(preview()).toHaveAttribute('data-pinned', 'true')
  })

  it('closes on a press on a control and the press still reaches it', async () => {
    const { user, first } = mount()
    let pressed = 0
    const control = document.createElement('button')
    control.addEventListener('pointerdown', () => (pressed += 1))
    document.body.append(control)
    await user.click(first)
    await user.click(control)
    expect(preview()).toBeNull()
    expect(pressed).toBe(1)
    control.remove()
  })

  it('switches to another card that is clicked', async () => {
    const { user, first, second } = mount()
    await user.click(first)
    const before = named()
    await user.click(second)
    expect(preview()).toHaveAttribute('data-pinned', 'true')
    expect(named()).not.toBe(before)
  })

  it('closes with a second click on its card', async () => {
    const { user, first } = mount()
    await user.click(first)
    await user.click(first)
    expect(preview()).toBeNull()
  })

  it('closes on Escape', async () => {
    const { user, first } = mount()
    await user.click(first)
    await user.keyboard('{Escape}')
    expect(preview()).toBeNull()
  })

  it('leaves a hovered preview alone', async () => {
    const { user, first } = mount()
    await user.hover(first)
    expect(preview()).not.toBeNull()
    expect(preview()?.getAttribute('data-pinned') ?? null).toBeNull()
    fireEvent.pointerDown(document.body)
    expect(preview()).not.toBeNull()
  })
})
