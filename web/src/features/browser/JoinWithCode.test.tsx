// Goal: the join code field takes a code as typed (any case, spaces and hyphens), hands the handler the code as the server reads it and the button that asked, refuses text that cannot be a code with a reason on the field, shows why the last join failed under the field until the code changes, and, while a request is out, spins, locks the code and sends nothing more, even on Enter.
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { JoinWithCode } from './JoinWithCode'

const field = () => screen.getByRole('textbox', { name: 'Join with a code' })
const button = () => screen.getByRole('button', { name: 'Join' })

describe('with a handler', () => {
  it.each([
    ['k7m2-9qxa', 'K7M29QXA'],
    [' K7M2 9QXA ', 'K7M29QXA'],
    ['abcd2345', 'ABCD2345'],
  ])('sends "%s" as %s and names the button that asked', async (typed, code) => {
    const onJoin = vi.fn()
    render(<JoinWithCode onJoin={onJoin} />)
    const user = userEvent.setup()
    await user.type(field(), typed)
    await user.click(button())
    expect(onJoin).toHaveBeenCalledExactlyOnceWith(code, button())
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    expect(button()).toBeEnabled()
  })

  it('sends on Enter in the field', async () => {
    const onJoin = vi.fn()
    render(<JoinWithCode onJoin={onJoin} />)
    await userEvent.setup().type(field(), 'K7M29QXA{Enter}')
    expect(onJoin).toHaveBeenCalledExactlyOnceWith('K7M29QXA', button())
  })

  it.each([
    ['', 'Type the code the host gave you.'],
    ['K7M29QX', 'A code is 8 letters and digits, like K7M2-9QXA.'],
    ['K7M29QXAB', 'A code is 8 letters and digits, like K7M2-9QXA.'],
    ['K7M2OQXA', 'A code is 8 letters and digits, like K7M2-9QXA.'],
  ])('refuses "%s" with a reason on the field, and sends nothing', async (typed, reason) => {
    const onJoin = vi.fn()
    const onEdit = vi.fn()
    render(<JoinWithCode onJoin={onJoin} onEdit={onEdit} />)
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
    expect(onEdit).toHaveBeenCalled()
  })
})

describe('a failure of the last join', () => {
  const WORDS = 'No open game with that code.'

  it('is said under the field, which it describes, with the typed code kept and the button free', async () => {
    render(<JoinWithCode onJoin={vi.fn()} failure={WORDS} />)
    expect(screen.getByRole('alert')).toHaveTextContent(WORDS)
    expect(field()).toHaveAccessibleDescription(WORDS)
    // The code was not malformed, so the field is not marked invalid; the words say what is wrong.
    expect(field()).not.toBeInvalid()
    expect(button()).toBeEnabled()
  })

  it('leaves with the first edit of the code, which the page is told about', async () => {
    const onEdit = vi.fn()
    const { rerender } = render(<JoinWithCode onJoin={vi.fn()} failure={WORDS} onEdit={onEdit} />)
    await userEvent.setup().type(field(), 'K')
    expect(onEdit).toHaveBeenCalledTimes(1)
    rerender(<JoinWithCode onJoin={vi.fn()} failure={null} onEdit={onEdit} />)
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    expect(field()).not.toHaveAttribute('aria-describedby')
  })

  it('gives way to a reason the player can fix, which comes first', async () => {
    const onJoin = vi.fn()
    render(<JoinWithCode onJoin={onJoin} failure={WORDS} />)
    await userEvent.setup().type(field(), 'K7M2{Enter}')
    expect(screen.getAllByRole('alert')).toHaveLength(1)
    expect(screen.getByRole('alert')).toHaveTextContent('A code is 8 letters and digits')
    expect(onJoin).not.toHaveBeenCalled()
  })
})

describe('while a request is out', () => {
  it('spins, locks the code and sends nothing more, even on Enter', async () => {
    const onJoin = vi.fn()
    render(<JoinWithCode onJoin={onJoin} pending busy />)
    expect(button()).toHaveAttribute('aria-busy', 'true')
    expect(field()).toHaveAttribute('readonly')
    expect(screen.getByRole('form', { name: 'Join with a code' })).toHaveAttribute('aria-busy', 'true')
    const user = userEvent.setup()
    await user.click(button())
    await user.type(field(), '{Enter}')
    expect(onJoin).not.toHaveBeenCalled()
  })

  it('waits, disabled but still reachable, while a row is joined, and does not spin', async () => {
    const onJoin = vi.fn()
    render(<JoinWithCode onJoin={onJoin} busy />)
    expect(button()).toHaveAttribute('aria-disabled', 'true')
    expect(button()).not.toHaveAttribute('aria-busy')
    button().focus()
    expect(button()).toHaveFocus()
    expect(field()).not.toHaveAttribute('readonly')
    await userEvent.setup().type(field(), 'K7M29QXA{Enter}')
    expect(onJoin).not.toHaveBeenCalled()
    expect(field()).toHaveValue('K7M29QXA')
  })
})
