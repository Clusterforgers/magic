// Goal: the create page offers who may find the game with the consequence beside each choice and lists the game by default, sends the choice with only the creator's seat held, opens the new game's lobby on that seat with the join code kept for it in this tab, and keeps working when the browser stores nothing or the server sends no code.
import { screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { loadInvites } from '@/lib/invites'
import { chooseSeatLinks, openCreateForm } from '@/test-utils/createForm'
import { jsonResponse } from '@/test-utils/fakeFetch'
import { MADE_CODE, MADE_GAME, tokenOf } from '@/test-utils/fakeServer'

afterEach(() => {
  vi.restoreAllMocks()
  window.sessionStorage.clear()
})

const listed = () => screen.getByRole('radio', { name: 'Listed' })
const inviteOnly = () => screen.getByRole('radio', { name: 'Invite only' })
const create = () => screen.getByRole('button', { name: 'Create game' })

describe('the visibility choice', () => {
  it('offers Listed and Invite only, with Listed chosen', async () => {
    await openCreateForm()
    expect(screen.getByRole('group', { name: 'Visibility' })).toBeInTheDocument()
    expect(listed()).toBeChecked()
    expect(inviteOnly()).not.toBeChecked()
  })

  it('says beside each choice what it means', async () => {
    await openCreateForm()
    expect(listed()).toHaveAccessibleDescription('Listed in the game browser')
    expect(inviteOnly()).toHaveAccessibleDescription('Only people with your link or code')
    expect(screen.getByText('Listed in the game browser')).toBeVisible()
    expect(screen.getByText('Only people with your link or code')).toBeVisible()
  })

  it('moves the choice with a press on the choice or on its words', async () => {
    const { user } = await openCreateForm()
    await user.click(inviteOnly())
    expect(inviteOnly()).toBeChecked()
    expect(listed()).not.toBeChecked()
    await user.click(screen.getByText('Listed'))
    expect(listed()).toBeChecked()
  })

  it.each([
    ['Listed', 'public'],
    ['Invite only', 'invite'],
  ])('sends %s as %s with only the creator’s seat held', async (choice, visibility) => {
    const { server, user } = await openCreateForm()
    await user.click(screen.getByRole('radio', { name: choice }))
    await user.click(create())
    await waitFor(() => expect(server.creates).toHaveLength(1))
    expect(server.creates[0]).toEqual({
      format: 'normal',
      seats: [
        { name: 'Player 1', deck: 'mono-green' },
        { name: 'Player 2', deck: 'mono-red' },
      ],
      visibility,
      held: 1,
    })
  })
})

describe('Advanced', () => {
  it('starts closed and holds the format and the link for every seat', async () => {
    const { user } = await openCreateForm()
    const details = screen.getByText('Advanced').closest('details')
    expect(details).not.toHaveAttribute('open')
    await user.click(screen.getByText('Advanced'))
    expect(details).toHaveAttribute('open')
    expect(screen.getByRole('radio', { name: 'Normal' })).toBeChecked()
    expect(screen.getByRole('checkbox', { name: 'Give every seat its own link' })).not.toBeChecked()
    expect(
      screen.getByRole('checkbox', { name: 'Give every seat its own link' }),
    ).toHaveAccessibleDescription(/The game is invite only/)
  })

  it('takes the visibility choice away while every seat gets a link, and brings it back as it was', async () => {
    const { user } = await openCreateForm()
    await user.click(inviteOnly())
    await chooseSeatLinks(user)
    expect(screen.queryByRole('group', { name: 'Visibility' })).toBeNull()
    await user.click(screen.getByRole('checkbox', { name: 'Give every seat its own link' }))
    expect(inviteOnly()).toBeChecked()
  })
})

describe('a game that is made', () => {
  it('opens its lobby on the creator’s seat, with the token only in the fragment', async () => {
    const { user, router } = await openCreateForm()
    await user.click(create())
    expect(await screen.findByTestId('game-stub')).toHaveTextContent(MADE_GAME)
    expect(router.state.location.pathname).toBe(`/game/${MADE_GAME}`)
    expect(router.state.location.href).toBe(`/game/${MADE_GAME}#seat=0&token=${tokenOf(0)}`)
    expect(screen.queryByRole('heading', { name: 'Game created' })).toBeNull()
  })

  it('keeps the code and the creator’s seat for that game in this tab, for the lobby to show', async () => {
    const { user } = await openCreateForm()
    await user.clear(screen.getByLabelText('Your name'))
    await user.type(screen.getByLabelText('Your name'), 'Ana')
    await user.click(create())
    await screen.findByTestId('game-stub')
    expect(loadInvites(MADE_GAME)).toEqual({
      game: MADE_GAME,
      code: MADE_CODE,
      seats: [{ seat: 0, name: 'Ana', token: tokenOf(0) }],
    })
    expect(loadInvites('another game')).toBeNull()
  })

  it('keeps no code for a game with a link for every seat, which has no open seat to take by one', async () => {
    const { user } = await openCreateForm()
    await chooseSeatLinks(user)
    await user.click(create())
    await screen.findByRole('heading', { name: 'Game created' })
    expect(loadInvites(MADE_GAME)?.code).toBeUndefined()
    expect(loadInvites(MADE_GAME)?.seats).toHaveLength(2)
  })

  it('opens the lobby when the server sends no code, keeping none', async () => {
    const { user } = await openCreateForm({
      create: () => jsonResponse(201, { game: MADE_GAME, seats: [{ seat: 0, token: tokenOf(0) }] }),
    })
    await user.click(create())
    await screen.findByTestId('game-stub')
    expect(loadInvites(MADE_GAME)).toEqual({
      game: MADE_GAME,
      seats: [{ seat: 0, name: 'Player 1', token: tokenOf(0) }],
    })
  })

  it('still opens the lobby when the browser will not store anything', async () => {
    vi.spyOn(window, 'sessionStorage', 'get').mockImplementation(() => {
      throw new DOMException('blocked', 'SecurityError')
    })
    const { user } = await openCreateForm()
    await user.click(create())
    expect(await screen.findByTestId('game-stub')).toHaveTextContent(MADE_GAME)
  })

  it('still opens the lobby when the store is full', async () => {
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new DOMException('quota', 'QuotaExceededError')
    })
    const { user } = await openCreateForm()
    await user.click(create())
    expect(await screen.findByTestId('game-stub')).toHaveTextContent(MADE_GAME)
  })

  it('shows the links page instead when the answer holds no token for the creator', async () => {
    const { user } = await openCreateForm({ create: () => jsonResponse(201, { game: MADE_GAME, seats: [] }) })
    await user.click(create())
    expect(await screen.findByRole('heading', { name: 'Game created' })).toBeInTheDocument()
  })
})
