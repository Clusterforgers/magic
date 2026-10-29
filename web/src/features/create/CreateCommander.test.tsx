// Goal: the form starts on Commander, follows the format in its seats, deck and rules, and sends the format and only the house rules that were filled in.
import { screen, waitFor } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { openCreateForm, typeSeats } from '@/test-utils/createForm'
import { MADE_GAME } from '@/test-utils/fakeServer'

const seats = () => screen.getByLabelText('Seats')
const create = () => screen.getByRole('button', { name: 'Create game' })
const rule = (label: string) => screen.getByLabelText(label)

describe('the form as first shown', () => {
  it('is a Commander game of four seats with no deck to pick, and says where the deck is chosen', async () => {
    await openCreateForm({}, 'default')
    expect(screen.getByRole('radio', { name: /^Commander/ })).toBeChecked()
    expect(screen.getByRole('radio', { name: /^Normal/ })).not.toBeChecked()
    expect(seats()).toHaveValue(4)
    expect(screen.queryByLabelText('Your deck')).toBeNull()
    expect(document.querySelector('[data-slot="deck-later"]')).toHaveTextContent(
      /choose your deck in the lobby/i,
    )
  })

  it('has a number field for the seats with no most, so a table of 24 is asked for by typing it', async () => {
    const { user } = await openCreateForm({}, 'default')
    expect(seats()).toHaveAttribute('type', 'number')
    expect(seats()).not.toHaveAttribute('max')
    await typeSeats(user, 24)
    expect(seats()).toHaveValue(24)
  })

  it('keeps a link for every seat to the Normal format', async () => {
    const { user } = await openCreateForm({}, 'default')
    await user.click(screen.getByText('Advanced'))
    expect(screen.getByRole('checkbox', { name: 'Give every seat its own link' })).toBeDisabled()
    await user.click(screen.getByRole('radio', { name: /^Normal/ }))
    expect(screen.getByRole('checkbox', { name: 'Give every seat its own link' })).toBeEnabled()
  })
})

describe('following the format', () => {
  it('goes to two seats and a deck to pick in Normal, and back to a pod of four in Commander', async () => {
    const { user } = await openCreateForm({}, 'default')
    await user.click(screen.getByRole('radio', { name: /^Normal/ }))
    expect(seats()).toHaveValue(2)
    expect(screen.getByLabelText('Your deck')).toBeInTheDocument()
    await user.click(screen.getByRole('radio', { name: /^Commander/ }))
    expect(seats()).toHaveValue(4)
    expect(screen.queryByLabelText('Your deck')).toBeNull()
  })

  it('keeps a chosen count in either format', async () => {
    const { user } = await openCreateForm({}, 'normal')
    await typeSeats(user, 9)
    await user.click(screen.getByRole('radio', { name: /^Commander/ }))
    expect(seats()).toHaveValue(9)
    await typeSeats(user, 40)
    await user.click(screen.getByRole('radio', { name: /^Normal/ }))
    expect(seats()).toHaveValue(40)
  })

  it('shows the Commander rules only in Commander', async () => {
    const { user } = await openCreateForm({}, 'default')
    expect(screen.queryByLabelText('Commander tax')).not.toBeNull()
    await user.click(screen.getByRole('radio', { name: /^Normal/ }))
    expect(screen.queryByLabelText('Commander tax')).toBeNull()
  })
})

describe('creating the game', () => {
  it('asks for a Commander game with the creator’s seat alone, no deck and no rules', async () => {
    const { server, user } = await openCreateForm({}, 'default')
    await user.click(create())
    await waitFor(() => expect(server.creates).toHaveLength(1))
    expect(server.creates[0]).toEqual({
      format: 'commander',
      seats: [{ name: 'Player 1' }, { name: 'Player 2' }, { name: 'Player 3' }, { name: 'Player 4' }],
      visibility: 'public',
      held: 1,
    })
  })

  it('sends the rules that were filled in and nothing else', async () => {
    const { server, user, router } = await openCreateForm({}, 'default')
    await user.click(screen.getByText('House rules'))
    await user.type(rule('Starting life'), '30')
    await user.type(rule('Hand limit'), 'unlimited')
    await user.selectOptions(rule('Mulligan style'), 'vancouver')
    await user.type(rule('Commander damage'), 'off')
    await user.click(create())
    await waitFor(() => expect(server.creates).toHaveLength(1))
    expect(server.creates[0]).toMatchObject({
      format: 'commander',
      house_rules: {
        starting_life: 30,
        max_hand_size: 'unlimited',
        mulligan_style: 'vancouver',
        commander_damage: 'off',
      },
    })
    await waitFor(() => expect(router.state.location.href).toContain(`/game/${MADE_GAME}`))
  })

  it('leaves the Commander rules out of a Normal game', async () => {
    const { server, user } = await openCreateForm({}, 'default')
    await user.click(screen.getByText('House rules'))
    await user.type(rule('Commander tax'), '0')
    await user.click(screen.getByRole('radio', { name: /^Normal/ }))
    await user.click(create())
    await waitFor(() => expect(server.creates).toHaveLength(1))
    expect(server.creates[0]).not.toHaveProperty('house_rules')
    expect(server.creates[0]).toMatchObject({ format: 'normal' })
  })

  it('shows the format’s own value as each empty field’s placeholder and counts what differs', async () => {
    const { user } = await openCreateForm({}, 'default')
    await user.click(screen.getByText('House rules'))
    expect(rule('Starting life')).toHaveAttribute('placeholder', '40')
    expect(rule('Free mulligans')).toHaveAttribute('placeholder', '1')
    expect(rule('Commander tax')).toHaveAttribute('placeholder', '2')
    const summary = document.querySelector('[data-slot="house-rules-summary"]')
    expect(summary).toHaveTextContent('Commander rules')
    await user.type(rule('Starting life'), '30')
    await user.type(rule('Free mulligans'), '1')
    expect(summary).toHaveTextContent('1 rule differs')
    await user.type(rule('Opening hand'), '6')
    expect(summary).toHaveTextContent('2 rules differ')
  })

  it('refuses a rule out of range beside its field and sends nothing', async () => {
    const { server, user } = await openCreateForm({}, 'default')
    await user.click(screen.getByText('House rules'))
    await user.type(rule('Starting life'), '0')
    await user.click(create())
    expect(server.creates).toHaveLength(0)
    expect(rule('Starting life')).toHaveAttribute('aria-invalid', 'true')
    expect(rule('Starting life')).toHaveAccessibleDescription(/whole number from 1 to 1000/)
    expect(rule('Starting life')).toHaveFocus()
    expect(screen.getByRole('alert')).toHaveTextContent('Fix the house rules marked below')
  })
})
