// Goal: the create-game page loads the decks, shows the seat count, the creator's own name and deck plainly, validates before it asks, and sends exactly the request the server documents for every seat count from 2 to 12, with only the creator's seat held.
import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { scaled } from '@/test-utils/loadFactor'
import { MAX_NAME_CHARS } from '@/lib/limits'
import { jsonResponse } from '@/test-utils/fakeFetch'
import { MADE_GAME, startServer, tokenOf } from '@/test-utils/fakeServer'
import type { ServerScript } from '@/test-utils/fakeServer'
import { openCreateForm, typeSeats } from '@/test-utils/createForm'
import { DECKS } from '@/test-utils/decks'
import { renderInRouter } from '@/test-utils/router'
import { CreateGamePage } from './CreateGamePage'

async function openPage(script: ServerScript = {}, strict = false) {
  const server = startServer(script)
  const user = userEvent.setup()
  const view = await renderInRouter(<CreateGamePage api={server.api} />, { strict })
  return { server, user, ...view }
}

const nameField = () => screen.getByLabelText('Your name')
const deckField = () => screen.getByLabelText('Your deck')
const seatCount = () => screen.getByLabelText('Seats')
const button = (name: string | RegExp) => screen.getByRole('button', { name })

const seatCounts = [2, 3, 4, 8, 12, 13, 24, 64]

describe('loading the decks', () => {
  it('says it is loading, then shows the form', async () => {
    await openPage()
    expect(screen.getByRole('status')).toHaveTextContent('Loading the decks')
    expect(await screen.findByRole('form', { name: 'Create a game' })).toBeInTheDocument()
    expect(screen.queryByText(/loading the decks/i)).toBeNull()
    expect(document.title).toBe('Create a game · Magic')
  })

  it('draws the skeleton of the form only once the wait has lasted 300 ms', async () => {
    await openPage({ decks: () => new Promise<Response>(() => undefined) })
    expect(screen.getByRole('status')).toHaveTextContent('Loading the decks')
    expect(document.querySelector('[data-slot="skeleton"]')).toBeNull()
    await waitFor(() => expect(document.querySelector('[data-slot="skeleton"]')).not.toBeNull(), {
      timeout: scaled(1500),
    })
    expect(screen.queryByRole('form')).toBeNull()
  })

  it('draws no skeleton at all when the decks come at once', async () => {
    await openCreateForm()
    expect(document.querySelector('[data-slot="skeleton"]')).toBeNull()
  })

  it('asks for the decks once, even when React mounts everything twice', async () => {
    const { server } = await openPage({}, true)
    await screen.findByRole('form', { name: 'Create a game' })
    expect(server.count('/api/demo-decks')).toBeLessThanOrEqual(2)
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
  it('asks for the number of seats in a number field with no most, defaulting to 2, and the creator’s own name and deck', async () => {
    await openCreateForm()
    expect(seatCount()).toHaveValue(2)
    expect(seatCount()).toHaveAttribute('type', 'number')
    expect(seatCount()).not.toHaveAttribute('max')
    expect(nameField()).toHaveValue('Player 1')
    expect(deckField()).toHaveValue('mono-green')
  })

  it('asks for nothing about the other seats: they are open for friends', async () => {
    await openCreateForm()
    expect(screen.queryByLabelText('Name for seat 2')).toBeNull()
    expect(screen.queryByLabelText('Deck for seat 2')).toBeNull()
    expect(screen.queryByRole('button', { name: 'Add seat' })).toBeNull()
  })

  it('describes every deck by name, colour and size in its menu', async () => {
    await openCreateForm()
    const options = within(deckField()).getAllByRole('option')
    expect(options.map((option) => option.textContent)).toEqual([
      'Mono Green Stompers (Green, 60 cards)',
      'Mono Red Giants (Red, 60 cards)',
      'Mono White Weenies (White, 60 cards)',
    ])
  })

  it('previews the chosen deck: colour, size, its biggest lines and the whole list on request', async () => {
    const { user } = await openCreateForm()
    const preview = within(screen.getByRole('region', { name: 'Players' }))
    expect(preview.getByText('Green, 60 cards')).toBeInTheDocument()
    expect(preview.getByText(/^Forest ×24, .* and 6 more$/)).toBeInTheDocument()
    expect(preview.getByText('24 × Forest')).not.toBeVisible()
    await user.click(preview.getByText('Full list of Mono Green Stompers'))
    expect(preview.getByText('24 × Forest')).toBeVisible()
    expect(preview.getAllByRole('listitem')).toHaveLength(10)
  })

  it('shows the preview of whatever deck is chosen now', async () => {
    const { user } = await openCreateForm()
    await user.selectOptions(deckField(), 'mono-white')
    const players = within(screen.getByRole('region', { name: 'Players' }))
    expect(players.getByText('White, 60 cards')).toBeInTheDocument()
    expect(players.getByText(/^Plains ×24, /)).toBeInTheDocument()
    expect(players.queryByText(/Mountain/)).toBeNull()
  })
})

describe('the number of seats', () => {
  it.each(seatCounts)('sends exactly the documented request for %i seats', async (count) => {
    const { server, user, router } = await openCreateForm()
    await typeSeats(user, count)
    await user.click(button('Create game'))
    await waitFor(() => expect(server.creates).toHaveLength(1))
    expect(server.creates[0]).toEqual({
      format: 'normal',
      // The creator's seat plays the deck the form shows; an open seat names none, its player chooses in the lobby.
      seats: Array.from({ length: count }, (_, i) =>
        i === 0 ? { name: 'Player 1', deck: DECKS[0]?.id } : { name: `Player ${i + 1}` },
      ),
      visibility: 'public',
      held: 1,
    })
    expect(await screen.findByTestId('game-stub')).toHaveTextContent(MADE_GAME)
    expect(router.state.location.href).toBe(`/game/${MADE_GAME}#seat=0&token=${tokenOf(0)}`)
  })

  it('keeps what was typed and chosen when the number changes, up and down', async () => {
    const { server, user } = await openCreateForm()
    await user.clear(nameField())
    await user.type(nameField(), 'Ana')
    await user.selectOptions(deckField(), 'mono-white')
    await typeSeats(user, 5)
    await typeSeats(user, 3)
    expect(seatCount()).toHaveValue(3)
    expect(nameField()).toHaveValue('Ana')
    expect(deckField()).toHaveValue('mono-white')
    await user.click(button('Create game'))
    await waitFor(() => expect(server.creates).toHaveLength(1))
    expect(server.creates[0]).toMatchObject({
      seats: [{ name: 'Ana', deck: 'mono-white' }, { name: 'Player 2' }, { name: 'Player 3' }],
    })
  })

  it('lets a number below the fewest pass on the way to a larger one, and says so', async () => {
    const { user } = await openCreateForm()
    await typeSeats(user, 20)
    expect(seatCount()).toHaveValue(20)
    await user.clear(seatCount())
    await user.type(seatCount(), '1')
    expect(seatCount()).toBeInvalid()
    expect(screen.getByText('A game needs at least 2 seats.')).toBeInTheDocument()
    await user.type(seatCount(), '5')
    expect(seatCount()).toHaveValue(15)
    expect(seatCount()).toBeValid()
    await user.clear(seatCount())
    await user.tab()
    expect(seatCount()).toHaveValue(15)
  })

  it('trims the name before sending it', async () => {
    const { server, user } = await openCreateForm()
    await user.clear(nameField())
    await user.type(nameField(), '   Ana  ')
    await user.click(button('Create game'))
    await waitFor(() => expect(server.creates).toHaveLength(1))
    expect(server.creates[0]).toMatchObject({ seats: [{ name: 'Ana' }, { name: 'Player 2' }] })
  })
})

describe('checking the form before asking', () => {
  async function submitWith(name: string) {
    const page = await openCreateForm()
    await page.user.clear(nameField())
    if (name !== '') await page.user.type(nameField(), name, { skipClick: true })
    await page.user.click(button('Create game'))
    return page
  }

  it.each([
    ['an empty name', '', /enter a name for this seat/i],
    ['a name that is too long', 'x'.repeat(MAX_NAME_CHARS + 1), /at most 32 characters \(this one has 33\)/i],
    ['a name with an invisible character', 'a\u200bb', /control or invisible/i],
  ])('refuses %s, says so beside the field and sends nothing', async (_what, name, message) => {
    const { server } = await submitWith(name)
    expect(server.creates).toHaveLength(0)
    expect(nameField()).toHaveAttribute('aria-invalid', 'true')
    expect(nameField()).toHaveAccessibleDescription(message)
    expect(screen.getByRole('alert')).toHaveTextContent(
      'Fix your name, deck or house rules before the game can be created.',
    )
    expect(nameField()).toHaveFocus()
  })

  it('shows nothing wrong before the first attempt, and lets the fix clear the message at once', async () => {
    const { server, user } = await openCreateForm()
    await user.clear(nameField())
    expect(nameField()).toHaveAttribute('aria-invalid', 'false')
    await user.click(button('Create game'))
    expect(nameField()).toHaveAttribute('aria-invalid', 'true')
    await user.type(nameField(), 'Bo')
    expect(nameField()).toHaveAttribute('aria-invalid', 'false')
    expect(screen.queryByRole('alert')).toBeNull()
    await user.click(button('Create game'))
    await waitFor(() => expect(server.creates).toHaveLength(1))
  })

  it('counts a name in characters, so 32 emoji are accepted', async () => {
    const emoji = '\u{1F600}'
    const { server, user } = await openCreateForm()
    await user.clear(nameField())
    await user.click(nameField())
    await user.paste(emoji.repeat(MAX_NAME_CHARS))
    await user.click(button('Create game'))
    await waitFor(() => expect(server.creates).toHaveLength(1))
  })
})
