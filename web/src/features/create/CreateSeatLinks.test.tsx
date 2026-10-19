// Goal: with a link for every seat switched on under Advanced the form is the earlier one, a name and a deck for each of 2 to 12 seats added and removed with what was typed kept, and it asks for an invite-only game that holds every seat; switching it off gives the simple form back with what was typed.
import { screen, waitFor, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { MAX_SEATS, MIN_SEATS } from '@/lib/limits'
import { must } from '@/test-utils/batches'
import { chooseSeatLinks, openCreateForm } from '@/test-utils/createForm'
import { DECKS } from '@/test-utils/decks'

const nameField = (seat: number) => screen.getByLabelText(`Name for seat ${seat}`)
const deckField = (seat: number) => screen.getByLabelText(`Deck for seat ${seat}`)
const button = (name: string | RegExp) => screen.getByRole('button', { name })
const seatRow = (seat: number) => within(must(deckField(seat).closest('li')))

const seatCounts = Array.from({ length: MAX_SEATS - MIN_SEATS + 1 }, (_, i) => MIN_SEATS + i)

async function openLinks() {
  const page = await openCreateForm()
  await chooseSeatLinks(page.user)
  return page
}

describe('switching it on', () => {
  it('shows a name and a deck for each of the two seats, on different decks, and no visibility choice', async () => {
    await openLinks()
    expect(nameField(1)).toHaveValue('Player 1')
    expect(nameField(2)).toHaveValue('Player 2')
    expect(deckField(1)).toHaveValue('mono-green')
    expect(deckField(2)).toHaveValue('mono-red')
    expect(screen.getByText('2 of 12 seats')).toBeInTheDocument()
    expect(screen.queryByRole('group', { name: 'Visibility' })).toBeNull()
    expect(screen.queryByLabelText('Your name')).toBeNull()
    expect(screen.getByRole('checkbox', { name: 'Give every seat its own link' })).toBeChecked()
  })

  it('previews the deck of every seat', async () => {
    const { user } = await openLinks()
    await user.selectOptions(deckField(2), 'mono-white')
    expect(seatRow(1).getByText('Green, 60 cards')).toBeInTheDocument()
    expect(seatRow(2).getByText('White, 60 cards')).toBeInTheDocument()
  })

  it('gives the simple form back, with what was typed, when it is switched off', async () => {
    const { user } = await openLinks()
    await user.clear(nameField(1))
    await user.type(nameField(1), 'Ana')
    await user.click(button('Add seat'))
    await user.click(screen.getByRole('checkbox', { name: 'Give every seat its own link' }))
    expect(screen.getByLabelText('Your name')).toHaveValue('Ana')
    expect(screen.getByLabelText('Seats')).toHaveValue('3')
    expect(screen.getByRole('group', { name: 'Visibility' })).toBeInTheDocument()
  })
})

describe('the number of seats', () => {
  it.each(seatCounts)('sends exactly the documented request for %i seats, every seat held', async (count) => {
    const { server, user } = await openLinks()
    for (let seat = MIN_SEATS; seat < count; seat += 1) await user.click(button('Add seat'))
    expect(screen.getByText(`${count} of 12 seats`)).toBeInTheDocument()
    await user.click(button('Create game'))
    await waitFor(() => expect(server.creates).toHaveLength(1))
    expect(server.creates[0]).toEqual({
      format: 'normal',
      seats: Array.from({ length: count }, (_, i) => ({
        name: `Player ${i + 1}`,
        deck: DECKS[i % DECKS.length]?.id,
      })),
      visibility: 'invite',
    })
    expect(await screen.findByRole('heading', { name: 'Game created' })).toBeInTheDocument()
  })

  it('stops offering more seats at 12 and fewer than 2', async () => {
    const { user } = await openLinks()
    expect(button('Remove seat 1')).toBeDisabled()
    expect(button('Remove seat 2')).toBeDisabled()
    for (let seat = MIN_SEATS; seat < MAX_SEATS; seat += 1) await user.click(button('Add seat'))
    expect(button('Add seat')).toBeDisabled()
    expect(screen.getAllByRole('heading', { name: /^Seat \d+$/ })).toHaveLength(12)
    expect(button('Remove seat 1')).toBeEnabled()
  })

  it('keeps what was typed for the seats that stay when one in the middle is removed', async () => {
    const { server, user } = await openLinks()
    await user.click(button('Add seat'))
    await user.clear(nameField(1))
    await user.type(nameField(1), 'Ana')
    await user.clear(nameField(3))
    await user.type(nameField(3), 'Cy')
    await user.selectOptions(deckField(3), 'mono-red')
    await user.click(button('Remove seat 2'))
    expect(nameField(1)).toHaveValue('Ana')
    expect(nameField(2)).toHaveValue('Cy')
    expect(deckField(2)).toHaveValue('mono-red')
    await user.click(button('Create game'))
    await waitFor(() => expect(server.creates).toHaveLength(1))
    expect(server.creates[0]).toEqual({
      format: 'normal',
      seats: [
        { name: 'Ana', deck: 'mono-green' },
        { name: 'Cy', deck: 'mono-red' },
      ],
      visibility: 'invite',
    })
  })
})

describe('checking the seats before asking', () => {
  it('names the seat that is wrong, sends nothing and puts focus on the first wrong field', async () => {
    const { server, user } = await openLinks()
    await user.click(button('Add seat'))
    await user.clear(nameField(2))
    await user.clear(nameField(3))
    await user.click(button('Create game'))
    expect(server.creates).toHaveLength(0)
    expect(nameField(2)).toHaveFocus()
    expect(nameField(2)).toHaveAccessibleDescription(/enter a name for this seat/i)
    expect(screen.getByRole('alert')).toHaveTextContent('2 seats need fixing before the game can be created.')
  })

  it('says one seat needs fixing when only one does', async () => {
    const { user } = await openLinks()
    await user.clear(nameField(2))
    await user.click(button('Create game'))
    expect(screen.getByRole('alert')).toHaveTextContent(
      'One seat needs fixing before the game can be created.',
    )
  })

  it('does not judge the names of the open seats once the simple form is back', async () => {
    const { server, user } = await openLinks()
    await user.clear(nameField(2))
    await user.click(screen.getByRole('checkbox', { name: 'Give every seat its own link' }))
    await user.click(button('Create game'))
    await waitFor(() => expect(server.creates).toHaveLength(1))
    expect(server.creates[0]).toMatchObject({ seats: [{ name: 'Player 1' }, { name: 'Player 2' }], held: 1 })
  })
})
