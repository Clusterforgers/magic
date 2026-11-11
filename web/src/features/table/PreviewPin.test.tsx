// Goal: on the table a click pins a card's preview, and the pin stays while the pointer crosses other cards until its Close button or a second click on the same card takes it away.
import { screen } from '@testing-library/react'
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

describe('a pinned table preview', () => {
  it('stays on its card while another card is hovered and left', async () => {
    const { user, first, second } = mount()
    await user.hover(second)
    const other = named()
    await user.click(first)
    const pinned = named()
    expect(pinned).not.toBe(other)
    await user.hover(second)
    expect(preview()).toHaveAttribute('data-pinned', 'true')
    expect(named()).toBe(pinned)
    await user.unhover(second)
    expect(named()).toBe(pinned)
  })

  it.each([
    ['its Close button', 'close'],
    ['a second click on the card', 'again'],
  ] as const)('closes with %s', async (_, how) => {
    const { user, first, second } = mount()
    await user.click(first)
    await user.hover(second)
    if (how === 'close') await user.click(screen.getByRole('button', { name: 'Close preview' }))
    else await user.click(first)
    expect(preview()?.getAttribute('data-pinned') ?? null).toBeNull()
  })
})
