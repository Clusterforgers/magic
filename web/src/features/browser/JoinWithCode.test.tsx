// Goal: the join code field takes a code as typed (any case, spaces and hyphens), hands the handler the code as the server reads it, refuses text that cannot be a code with a reason on the field, and, with no handler, is disabled and says why and sends nothing even on Enter.
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { CODE_NOTE, JoinWithCode } from './JoinWithCode'

const field = () => screen.getByRole('textbox', { name: 'Join with a code' })
const button = () => screen.getByRole('button', { name: 'Join' })

describe('with a handler', () => {
  it.each([
    ['k7m2-9qxa', 'K7M29QXA'],
    [' K7M2 9QXA ', 'K7M29QXA'],
    ['abcd2345', 'ABCD2345'],
  ])('sends "%s" as %s', async (typed, code) => {
    const onJoin = vi.fn()
    render(<JoinWithCode onJoin={onJoin} />)
    const user = userEvent.setup()
    await user.type(field(), typed)
    await user.click(button())
    expect(onJoin).toHaveBeenCalledExactlyOnceWith(code)
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    expect(screen.queryByText(CODE_NOTE)).not.toBeInTheDocument()
    expect(button()).toBeEnabled()
  })

  it.each([
    ['', 'Type the code the host gave you.'],
    ['K7M29QX', 'A code is 8 letters and digits, like K7M2-9QXA.'],
    ['K7M29QXAB', 'A code is 8 letters and digits, like K7M2-9QXA.'],
    ['K7M2OQXA', 'A code is 8 letters and digits, like K7M2-9QXA.'],
  ])('refuses "%s" with a reason on the field, and sends nothing', async (typed, reason) => {
    const onJoin = vi.fn()
    render(<JoinWithCode onJoin={onJoin} />)
    const user = userEvent.setup()
    await user.click(field())
    if (typed !== '') await user.keyboard(typed)
    await user.keyboard('{Enter}')
    expect(screen.getByRole('alert')).toHaveTextContent(reason)
    expect(field()).toBeInvalid()
    expect(field()).toHaveAccessibleDescription(reason)
    expect(onJoin).not.toHaveBeenCalled()
    await user.type(field(), 'x')
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })
})

describe('without a handler', () => {
  it('disables the button and points it at the note', () => {
    render(<JoinWithCode />)
    expect(screen.getByText(CODE_NOTE)).toBeVisible()
    expect(button()).toHaveAttribute('aria-disabled', 'true')
    expect(button()).toHaveAccessibleDescription(CODE_NOTE)
  })

  it('does nothing when a valid code is typed and Enter is pressed, and shows no problem', async () => {
    render(<JoinWithCode />)
    await userEvent.setup().type(field(), 'K7M29QXA{Enter}')
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    expect(field()).toHaveValue('K7M29QXA')
  })
})
