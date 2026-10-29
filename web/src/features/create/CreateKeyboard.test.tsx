// Goal: the whole create-game flow can be done from the keyboard alone: every control is reached with Tab in reading order, Enter submits, seats of the link-per-seat form are added and removed with focus kept in view, and the result is reachable and operable.
import { screen, waitFor } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { chooseSeatLinks, openCreateForm } from '@/test-utils/createForm'
import { MADE_GAME, tokenOf } from '@/test-utils/fakeServer'

describe('reading order', () => {
  it('visits the seat count, the creator’s name and deck, then the visibility choice', async () => {
    const { user } = await openCreateForm()
    await user.tab()
    expect(document.activeElement).toBe(screen.getByLabelText('Seats'))
    await user.tab()
    expect(document.activeElement).toBe(screen.getByLabelText('Your name'))
    await user.tab()
    expect(document.activeElement).toBe(screen.getByLabelText('Your deck'))
    await user.tab()
    expect(document.activeElement).toHaveTextContent('Full list of Mono Green Stompers')
    await user.tab()
    expect(document.activeElement).toBe(screen.getByRole('radio', { name: 'Listed' }))
  })

  it('puts the choice, then Advanced, then the button after the fields in the page', async () => {
    await openCreateForm()
    const order = [
      screen.getByLabelText('Your deck'),
      screen.getByRole('radio', { name: 'Listed' }),
      screen.getByText('Advanced'),
      screen.getByRole('button', { name: 'Create game' }),
    ]
    for (let i = 1; i < order.length; i += 1) {
      expect(order[i - 1]?.compareDocumentPosition(order[i] as Element)).toBe(
        Node.DOCUMENT_POSITION_FOLLOWING,
      )
    }
  })

  it('never lets a positive tabindex reorder the form', async () => {
    await openCreateForm()
    expect(document.querySelectorAll('[tabindex]:not([tabindex="-1"]):not([tabindex="0"])')).toHaveLength(0)
  })

  it('gives every control on the form an accessible name', async () => {
    const { user } = await openCreateForm()
    await user.click(screen.getByText('Advanced'))
    for (const control of screen.getAllByRole('textbox')) expect(control).toHaveAccessibleName()
    for (const control of screen.getAllByRole('combobox')) expect(control).toHaveAccessibleName()
    for (const control of screen.getAllByRole('button')) expect(control).toHaveAccessibleName()
    for (const control of screen.getAllByRole('radio')) expect(control).toHaveAccessibleName()
    for (const control of screen.getAllByRole('checkbox')) expect(control).toHaveAccessibleName()
  })
})

describe('doing it with keys', () => {
  it('creates the game when Enter is pressed in the name field', async () => {
    const { server, user } = await openCreateForm()
    await user.clear(screen.getByLabelText('Your name'))
    await user.keyboard('Ana{Enter}')
    await waitFor(() => expect(server.creates).toHaveLength(1))
    expect(server.creates[0]).toMatchObject({ seats: [{ name: 'Ana' }, { name: 'Player 2' }] })
    expect(await screen.findByTestId('game-stub')).toBeInTheDocument()
  })

  it('creates the game when the Create game button is pressed with Space', async () => {
    const { server, user } = await openCreateForm()
    screen.getByRole('button', { name: 'Create game' }).focus()
    await user.keyboard(' ')
    await waitFor(() => expect(server.creates).toHaveLength(1))
  })

  it('chooses Invite only from the keyboard', async () => {
    const { server, user } = await openCreateForm()
    screen.getByRole('radio', { name: 'Listed' }).focus()
    await user.keyboard('{ArrowDown}')
    expect(screen.getByRole('radio', { name: 'Invite only' })).toBeChecked()
    await user.click(screen.getByRole('button', { name: 'Create game' }))
    await waitFor(() => expect(server.creates).toHaveLength(1))
    expect(server.creates[0]).toMatchObject({ visibility: 'invite' })
  })
})

describe('the form with a link for every seat', () => {
  it('adds a seat with Enter and lands in its name field', async () => {
    const { user } = await openCreateForm()
    await chooseSeatLinks(user)
    screen.getByRole('button', { name: 'Add seat' }).focus()
    await user.keyboard('{Enter}')
    expect(screen.getByLabelText('Name for seat 3')).toHaveFocus()
    expect(screen.getByText('3 seats')).toBeInTheDocument()
  })

  it('removes a seat with Space and keeps focus on the seat that took its place', async () => {
    const { user } = await openCreateForm()
    await chooseSeatLinks(user)
    await user.click(screen.getByRole('button', { name: 'Add seat' }))
    await user.click(screen.getByRole('button', { name: 'Add seat' }))
    screen.getByRole('button', { name: 'Remove seat 2' }).focus()
    await user.keyboard(' ')
    expect(screen.queryByLabelText('Name for seat 4')).toBeNull()
    expect(screen.getByLabelText('Name for seat 2')).toHaveFocus()
    expect(screen.getByLabelText('Name for seat 2')).toHaveValue('Player 3')
  })

  it('moves focus to the previous seat when the last one is removed', async () => {
    const { user } = await openCreateForm()
    await chooseSeatLinks(user)
    await user.click(screen.getByRole('button', { name: 'Add seat' }))
    screen.getByRole('button', { name: 'Remove seat 3' }).focus()
    await user.keyboard('{Enter}')
    expect(screen.getByLabelText('Name for seat 2')).toHaveFocus()
  })
})

describe('after the links are made', () => {
  it('starts at the result, reaches Open my seat with one Tab and follows it with Enter', async () => {
    const { user, router } = await openCreateForm()
    await chooseSeatLinks(user)
    screen.getByRole('button', { name: 'Create game' }).focus()
    await user.keyboard('{Enter}')
    const heading = await screen.findByRole('heading', { name: 'Game created' })
    expect(heading).toHaveFocus()
    await user.tab()
    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Open my seat (Player 1)' }))
    await user.keyboard('{Enter}')
    await waitFor(() =>
      expect(router.state.location.href).toBe(`/game/${MADE_GAME}#seat=0&token=${tokenOf(0)}`),
    )
  })

  it('reaches every copy button and every link in reading order', async () => {
    const { user } = await openCreateForm()
    await chooseSeatLinks(user)
    screen.getByRole('button', { name: 'Create game' }).focus()
    await user.keyboard('{Enter}')
    await screen.findByRole('heading', { name: 'Game created' })
    const order: string[] = []
    for (let step = 0; step < 7; step += 1) {
      await user.tab()
      const active = document.activeElement
      const name = active?.getAttribute('aria-label') ?? active?.textContent ?? '?'
      order.push(name.startsWith('http') ? 'a seat link' : name)
    }
    expect(order).toEqual([
      'Open my seat (Player 1)',
      'Copy all links',
      'Copy link for seat 1 (Player 1)',
      'a seat link',
      'Copy link for seat 2 (Player 2)',
      'a seat link',
      'Create another game',
    ])
  })
})
