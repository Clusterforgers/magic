// Goal: the create-game page loads the decks, shows the format and each deck plainly, validates before it asks, and sends exactly the request the server documents for every seat count from 2 to 12.
import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { MAX_NAME_CHARS, MAX_SEATS, MIN_SEATS } from '@/lib/limits'
import { must } from '@/test-utils/batches'
import { jsonResponse } from '@/test-utils/fakeFetch'
import { startServer } from '@/test-utils/fakeServer'
import type { ServerScript } from '@/test-utils/fakeServer'
import { DECKS } from '@/test-utils/decks'
import { renderInRouter } from '@/test-utils/router'
import { CreateGamePage } from './CreateGamePage'

async function openPage(script: ServerScript = {}, strict = false) {
  const server = startServer(script)
  const user = userEvent.setup()
  const view = await renderInRouter(<CreateGamePage api={server.api} />, { strict })
  return { server, user, ...view }
}

async function openForm(script: ServerScript = {}) {
  const page = await openPage(script)
  await screen.findByRole('heading', { name: 'Create a game' })
  return page
}

const nameField = (seat: number) => screen.getByLabelText(`Name for seat ${seat}`)
const deckField = (seat: number) => screen.getByLabelText(`Deck for seat ${seat}`)
const button = (name: string | RegExp) => screen.getByRole('button', { name })
const seatRow = (seat: number) => within(must(deckField(seat).closest('li')))

const seatCounts = Array.from({ length: MAX_SEATS - MIN_SEATS + 1 }, (_, i) => MIN_SEATS + i)

describe('loading the decks', () => {
  it('says it is loading, then shows the form', async () => {
    await openPage()
    expect(screen.getByRole('status')).toHaveTextContent('Loading the decks')
    expect(await screen.findByRole('form', { name: 'Create a game' })).toBeInTheDocument()
    expect(screen.queryByText(/loading the decks/i)).toBeNull()
    expect(document.title).toBe('Create a game · Magic')
  })

  it('asks for the decks once, even when React mounts everything twice', async () => {
    const { server } = await openPage({}, true)
    await screen.findByRole('form', { name: 'Create a game' })
    expect(server.count('/api/decks')).toBeLessThanOrEqual(2)
    expect(screen.getAllByRole('form')).toHaveLength(1)
  })

  it('says why the decks failed and tries again on request', async () => {
    let attempts = 0
    const { user } = await openPage({
      decks: () => {
        attempts += 1
        return attempts === 1
          ? jsonResponse(503, { error: 'unavailable', message: 'the server is shutting down' })
          : jsonResponse(200, DECKS)
      },
    })
    expect(await screen.findByRole('alert')).toHaveTextContent('Cannot load the decks')
    expect(screen.getByRole('alert')).toHaveTextContent(/shutting down or restarting/i)
    await user.click(button('Retry'))
    expect(await screen.findByRole('form', { name: 'Create a game' })).toBeInTheDocument()
    expect(attempts).toBe(2)
  })

  it('explains an empty deck list instead of showing a form that cannot work', async () => {
    await openPage({ decks: () => jsonResponse(200, []) })
    expect(await screen.findByRole('alert')).toHaveTextContent('No decks to play with')
    expect(screen.queryByRole('form')).toBeNull()
  })
})

describe('the form as first shown', () => {
  it('has two seats named Player 1 and Player 2 on different decks', async () => {
    await openForm()
    expect(nameField(1)).toHaveValue('Player 1')
    expect(nameField(2)).toHaveValue('Player 2')
    expect(deckField(1)).toHaveValue('mono-green')
    expect(deckField(2)).toHaveValue('mono-red')
    expect(screen.queryByLabelText('Name for seat 3')).toBeNull()
    expect(screen.getByText('2 of 12 seats')).toBeInTheDocument()
  })

  it('fixes the format to Normal and shows Commander as unavailable, coming soon', async () => {
    const { user } = await openForm()
    const normal = screen.getByRole('radio', { name: 'Normal' })
    const commander = screen.getByRole('radio', { name: /Commander/ })
    expect(normal).toBeChecked()
    expect(commander).toBeDisabled()
    expect(commander).toHaveAccessibleName(/coming soon/i)
    await user.click(commander)
    expect(normal).toBeChecked()
    expect(commander).not.toBeChecked()
  })

  it('describes every deck by name, colour and size in its menu', async () => {
    await openForm()
    const options = within(deckField(1)).getAllByRole('option')
    expect(options.map((option) => option.textContent)).toEqual([
      'Mono Green Stompers (Green, 60 cards)',
      'Mono Red Giants (Red, 60 cards)',
      'Mono White Weenies (White, 60 cards)',
    ])
  })

  it('previews the chosen deck: colour, size, its biggest lines and the whole list on request', async () => {
    const { user } = await openForm()
    const preview = seatRow(1)
    expect(preview.getByText('Green, 60 cards')).toBeInTheDocument()
    expect(preview.getByText(/^Forest ×24, .* and 6 more$/)).toBeInTheDocument()
    expect(preview.getByText('24 × Forest')).not.toBeVisible()
    await user.click(preview.getByText('Full list of Mono Green Stompers'))
    expect(preview.getByText('24 × Forest')).toBeVisible()
    expect(preview.getAllByRole('listitem')).toHaveLength(10)
  })

  it('shows the preview of whatever deck is chosen now', async () => {
    const { user } = await openForm()
    await user.selectOptions(deckField(2), 'mono-white')
    const seat = seatRow(2)
    expect(seat.getByText('White, 60 cards')).toBeInTheDocument()
    expect(seat.getByText(/^Plains ×24, /)).toBeInTheDocument()
    expect(seat.queryByText(/Mountain/)).toBeNull()
  })
})

describe('the number of seats', () => {
  it.each(seatCounts)('sends exactly the documented request for %i seats', async (count) => {
    const { server, user } = await openForm()
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
    })
    expect(await screen.findByRole('heading', { name: 'Game created' })).toBeInTheDocument()
  })

  it('stops offering more seats at 12', async () => {
    const { user } = await openForm()
    for (let seat = MIN_SEATS; seat < MAX_SEATS; seat += 1) await user.click(button('Add seat'))
    expect(button('Add seat')).toBeDisabled()
    expect(screen.getAllByRole('heading', { name: /^Seat \d+$/ })).toHaveLength(12)
  })

  it('stops offering to remove seats at 2', async () => {
    const { user } = await openForm()
    expect(button('Remove seat 1')).toBeDisabled()
    expect(button('Remove seat 2')).toBeDisabled()
    await user.click(button('Add seat'))
    expect(button('Remove seat 1')).toBeEnabled()
    await user.click(button('Remove seat 3'))
    expect(button('Remove seat 1')).toBeDisabled()
  })

  it('keeps what was typed for the seats that stay when one in the middle is removed', async () => {
    const { server, user } = await openForm()
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
    })
  })

  it('trims names before sending them', async () => {
    const { server, user } = await openForm()
    await user.clear(nameField(1))
    await user.type(nameField(1), '   Ana  ')
    await user.click(button('Create game'))
    await waitFor(() => expect(server.creates).toHaveLength(1))
    expect(server.creates[0]).toMatchObject({ seats: [{ name: 'Ana' }, { name: 'Player 2' }] })
  })
})

describe('checking the form before asking', () => {
  async function submitWith(name: string) {
    const page = await openForm()
    await page.user.clear(nameField(2))
    if (name !== '') await page.user.type(nameField(2), name, { skipClick: true })
    await page.user.click(button('Create game'))
    return page
  }

  it.each([
    ['an empty name', '', /enter a name for this seat/i],
    ['a name that is too long', 'x'.repeat(MAX_NAME_CHARS + 1), /at most 32 characters \(this one has 33\)/i],
    ['a name with an invisible character', 'a​b', /control or invisible/i],
  ])('refuses %s, says so beside the field and sends nothing', async (_what, name, message) => {
    const { server } = await submitWith(name)
    expect(server.creates).toHaveLength(0)
    expect(nameField(2)).toHaveAttribute('aria-invalid', 'true')
    expect(nameField(2)).toHaveAccessibleDescription(message)
    expect(nameField(1)).toHaveAttribute('aria-invalid', 'false')
    expect(screen.getByRole('alert')).toHaveTextContent(
      'One seat needs fixing before the game can be created.',
    )
  })

  it('puts keyboard focus on the first field that is wrong', async () => {
    const { user } = await openForm()
    await user.click(button('Add seat'))
    await user.clear(nameField(2))
    await user.clear(nameField(3))
    await user.click(button('Create game'))
    expect(nameField(2)).toHaveFocus()
    expect(screen.getByRole('alert')).toHaveTextContent('2 seats need fixing')
  })

  it('shows nothing wrong before the first attempt, and lets the fix clear the message at once', async () => {
    const { server, user } = await openForm()
    await user.clear(nameField(2))
    expect(nameField(2)).toHaveAttribute('aria-invalid', 'false')
    await user.click(button('Create game'))
    expect(nameField(2)).toHaveAttribute('aria-invalid', 'true')
    await user.type(nameField(2), 'Bo')
    expect(nameField(2)).toHaveAttribute('aria-invalid', 'false')
    expect(screen.queryByRole('alert')).toBeNull()
    await user.click(button('Create game'))
    await waitFor(() => expect(server.creates).toHaveLength(1))
  })

  it('counts a name in characters, so 32 emoji are accepted', async () => {
    const emoji = '\u{1F600}'
    const { server, user } = await openForm()
    await user.clear(nameField(1))
    await user.click(nameField(1))
    await user.paste(emoji.repeat(MAX_NAME_CHARS))
    await user.click(button('Create game'))
    await waitFor(() => expect(server.creates).toHaveLength(1))
  })
})
