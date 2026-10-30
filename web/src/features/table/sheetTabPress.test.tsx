// Goal: a press on the tab that is already on show keeps the side sheet open (its tabs live inside it and Close shuts it), while in the rail the same press closes the panel it opened.
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { renderTable } from '@/test-utils/tableHarness'

const NAME = 'Commands, log, chat and stack'
const opener = () => screen.getByRole('button', { name: /^Commands, log, chat and stack/, hidden: true })

describe.each([
  ['a phone', { width: 390, height: 844 }],
  ['a tablet', { width: 800, height: 900 }],
])('%s: the sheet', (_name, viewport) => {
  it('stays open when the tab on show is pressed, also after another tab was chosen', async () => {
    const { user } = renderTable({ seats: 2, options: { viewport } })
    await user.click(opener())
    const sheet = screen.getByRole('dialog', { name: NAME })
    await user.click(within(sheet).getByRole('tab', { name: /^Commands/ }))
    expect(opener()).toHaveAttribute('aria-expanded', 'true')
    expect(within(sheet).getByRole('region', { name: 'Commands' })).toBeVisible()
    await user.click(within(sheet).getByRole('tab', { name: /^Log/ }))
    await user.click(within(sheet).getByRole('tab', { name: /^Log/ }))
    expect(opener()).toHaveAttribute('aria-expanded', 'true')
    expect(within(sheet).getByRole('log', { name: 'Game log' })).toBeInTheDocument()
  })
})

describe('a medium window: the rail', () => {
  it('closes the panel from the icon of the tab on show', async () => {
    const { user } = renderTable({ seats: 2, options: { viewport: { width: 1100, height: 800 } } })
    const commands = () => screen.getByRole('tab', { name: 'Commands' })
    await user.click(commands())
    expect(screen.getByRole('region', { name: 'Commands' })).toBeInTheDocument()
    await user.click(commands())
    expect(screen.queryByRole('region', { name: 'Commands' })).toBeNull()
  })
})
