// Goal: a created game shows one link per seat in the documented form with the token only in the fragment, copies each link and all of them, opens the host's own seat, and keeps working when the browser will not remember anything.
import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { loadInvites } from '@/lib/invites'
import { MADE_GAME, startServer, tokenOf } from '@/test-utils/fakeServer'
import { visibleOutsideShareLinks } from '@/test-utils/leaks'
import { renderInRouter } from '@/test-utils/router'
import { CreateGamePage } from './CreateGamePage'

async function createGame(seats = 3) {
  const server = startServer()
  const user = userEvent.setup()
  const view = await renderInRouter(<CreateGamePage api={server.api} />)
  await screen.findByRole('form', { name: 'Create a game' })
  for (let seat = 2; seat < seats; seat += 1)
    await user.click(screen.getByRole('button', { name: 'Add seat' }))
  await user.clear(screen.getByLabelText('Name for seat 1'))
  await user.type(screen.getByLabelText('Name for seat 1'), 'Ana')
  await user.click(screen.getByRole('button', { name: 'Create game' }))
  await screen.findByRole('heading', { name: 'Game created' })
  return { server, user, ...view }
}

const linkList = () => within(screen.getByRole('list', { name: 'Seat links' }))
const origin = window.location.origin

afterEach(() => {
  vi.restoreAllMocks()
  window.sessionStorage.clear()
})

describe('the links of a created game', () => {
  it('moves focus to the heading so a screen reader starts at the result', async () => {
    await createGame()
    expect(screen.getByRole('heading', { name: 'Game created' })).toHaveFocus()
  })

  it.each([2, 3, 7, 12])(
    'gives %i seats one link each, of the form /game/<id>#seat=<n>&token=<token>',
    async (count) => {
      await createGame(count)
      const links = linkList().getAllByRole('link')
      expect(links).toHaveLength(count)
      links.forEach((link, seat) => {
        expect(link).toHaveAttribute('href', `/game/${MADE_GAME}#seat=${seat}&token=${tokenOf(seat)}`)
        expect(link).toHaveTextContent(`${origin}/game/${MADE_GAME}#seat=${seat}&token=${tokenOf(seat)}`)
      })
    },
  )

  it('puts every token only after the # and nowhere else on the page', async () => {
    const { container } = await createGame(4)
    const links = linkList().getAllByRole('link')
    for (const link of links) {
      const url = new URL(link.getAttribute('href') ?? '', origin)
      expect(url.pathname + url.search).not.toMatch(/[0-9a-f]{64}/)
      expect(url.hash).toMatch(/^#seat=\d+&token=[0-9a-f]{64}$/)
    }
    const elsewhere = visibleOutsideShareLinks(container)
    for (let seat = 0; seat < 4; seat += 1) expect(elsewhere).not.toContain(tokenOf(seat))
  })

  it('labels each seat with its name and deck', async () => {
    await createGame(3)
    const rows = linkList().getAllByRole('listitem')
    expect(rows[0]).toHaveTextContent('Seat 1: Ana')
    expect(rows[0]).toHaveTextContent('Mono Green Stompers')
    expect(rows[2]).toHaveTextContent('Seat 3: Player 3')
    expect(rows[2]).toHaveTextContent('Mono White Weenies')
  })

  it('tells the host that a link is a key and to send each player only their own', async () => {
    await createGame()
    expect(screen.getByText(/anyone who has it can play that seat/i)).toBeInTheDocument()
    expect(screen.getByText(/send every player only their own link/i)).toBeInTheDocument()
  })
})

describe('copying', () => {
  it('copies one seat link, and only that one', async () => {
    const { user } = await createGame(3)
    await user.click(screen.getByRole('button', { name: 'Copy link for seat 2 (Player 2)' }))
    expect(await navigator.clipboard.readText()).toBe(
      `${origin}/game/${MADE_GAME}#seat=1&token=${tokenOf(1)}`,
    )
  })

  it('copies all links, one per line with the seat name', async () => {
    const { user } = await createGame(3)
    await user.click(screen.getByRole('button', { name: 'Copy all links' }))
    expect(await navigator.clipboard.readText()).toBe(
      [0, 1, 2]
        .map(
          (seat) =>
            `${['Ana', 'Player 2', 'Player 3'][seat]}: ${origin}/game/${MADE_GAME}#seat=${seat}&token=${tokenOf(seat)}`,
        )
        .join('\n'),
    )
  })

  it('has a copy button with a distinct name for every seat', async () => {
    await createGame(5)
    const names = screen
      .getAllByRole('button', { name: /^Copy link for seat/ })
      .map((b) => b.getAttribute('aria-label'))
    expect(new Set(names).size).toBe(5)
  })
})

describe('opening a seat', () => {
  it('takes the host to seat 1 with the token in the fragment', async () => {
    const { user, router } = await createGame(3)
    await user.click(screen.getByRole('button', { name: 'Open my seat (Ana)' }))
    await waitFor(() => expect(router.state.location.pathname).toBe(`/game/${MADE_GAME}`))
    expect(router.state.location.href).toBe(`/game/${MADE_GAME}#seat=0&token=${tokenOf(0)}`)
    expect(await screen.findByTestId('game-stub')).toHaveTextContent(MADE_GAME)
  })

  it('goes back to the form with the same seats for another game', async () => {
    const { user } = await createGame(3)
    await user.click(screen.getByRole('button', { name: 'Create another game' }))
    expect(screen.getByLabelText('Name for seat 1')).toHaveValue('Ana')
    expect(screen.getByLabelText('Name for seat 3')).toHaveValue('Player 3')
    expect(screen.queryByRole('link', { name: /token=/ })).toBeNull()
  })
})

describe('remembering the links in this tab', () => {
  it('keeps every seat link of the newest game in sessionStorage for the lobby', async () => {
    await createGame(3)
    expect(loadInvites(MADE_GAME)).toEqual({
      game: MADE_GAME,
      seats: [
        { seat: 0, name: 'Ana', token: tokenOf(0) },
        { seat: 1, name: 'Player 2', token: tokenOf(1) },
        { seat: 2, name: 'Player 3', token: tokenOf(2) },
      ],
    })
  })

  it('still shows every link when the browser will not store anything', async () => {
    vi.spyOn(window, 'sessionStorage', 'get').mockImplementation(() => {
      throw new DOMException('blocked', 'SecurityError')
    })
    await createGame(2)
    expect(linkList().getAllByRole('link')).toHaveLength(2)
    expect(screen.getByRole('button', { name: 'Open my seat (Ana)' })).toBeEnabled()
  })

  it('still shows every link when the store is full', async () => {
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new DOMException('quota', 'QuotaExceededError')
    })
    await createGame(2)
    expect(linkList().getAllByRole('link')).toHaveLength(2)
  })
})
