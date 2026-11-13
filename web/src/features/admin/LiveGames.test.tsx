// Goal: the admin's list of live games shows each lobby and running game (its seats, state, mode, when it was made and last active) newest first as the server sent it, and ends one only after a question that starts in its reason field, sends the trimmed optional reason once, drops the row and says so; a refusal stays in the dialog in plain words and the row stays; an empty or failed list says so with a way to try again.
import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { AccountError } from '@/net/accounts'
import type { AdminGamesApi } from '@/net/adminGames'
import type { AdminGame } from '@/protocol/generated'
import { gameTitle } from './gameWords'
import { LiveGames } from './LiveGames'

const RUNNING: AdminGame = {
  game: '11111111-1111-4111-8111-111111111111',
  format: 'commander',
  visibility: 'invite',
  started: true,
  tutorial: false,
  seats: [
    { name: 'Ann', connected: true },
    { name: 'Bot', connected: false },
  ],
  created_at: '2026-11-13T10:00:00Z',
  last_activity: '2026-11-13T11:00:00Z',
}

const LOBBY: AdminGame = {
  ...RUNNING,
  game: '22222222-2222-4222-8222-222222222222',
  format: 'normal',
  visibility: 'public',
  started: false,
  seats: [
    { name: 'Cy', connected: true },
    { name: '', connected: false },
  ],
}

function fakeApi(games: AdminGame[], end: AdminGamesApi['endGame'] = () => Promise.resolve()) {
  return {
    listGames: vi.fn(() => Promise.resolve(games)),
    endGame: vi.fn(end),
  }
}

async function rows() {
  const list = await screen.findByRole('list')
  return within(list).getAllByRole('listitem')
}

describe('the list of live games', () => {
  it('shows every game in the order the server sent, with its seats, state and mode', async () => {
    render(<LiveGames api={fakeApi([LOBBY, RUNNING])} />)
    const [lobby, running] = await rows()
    expect(lobby).toHaveTextContent('Cy vs open seat')
    expect(within(lobby!).getByText('Lobby')).toBeInTheDocument()
    expect(lobby).toHaveTextContent('Normal, public')
    expect(running).toHaveTextContent('Ann vs Bot')
    expect(within(running!).getByText('Running')).toBeInTheDocument()
    expect(running).toHaveTextContent('Commander, invite only')
    expect(running).toHaveTextContent(RUNNING.game)
    expect(within(running!).getByText(/^Created /)).toHaveAttribute('datetime', RUNNING.created_at)
    expect(within(running!).getByText(/^Last active /)).toHaveAttribute('datetime', RUNNING.last_activity)
    expect(screen.getByText('2 games in a lobby or running, newest first.')).toBeInTheDocument()
  })

  it('says when there is no game', async () => {
    render(<LiveGames api={fakeApi([])} />)
    expect(await screen.findByText('No game is in a lobby or running.')).toBeInTheDocument()
  })

  it('says why it could not load and loads again on request', async () => {
    const api = fakeApi([RUNNING])
    api.listGames.mockRejectedValueOnce(new AccountError('forbidden', 403, 'only an admin can do that'))
    render(<LiveGames api={api} />)
    expect(await screen.findByText('Could not load the games. only an admin can do that')).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'Try again' }))
    expect(await rows()).toHaveLength(1)
  })

  it('names a game by its seats', () => {
    expect(gameTitle(RUNNING)).toBe('Ann vs Bot')
    expect(gameTitle(LOBBY)).toBe('Cy vs open seat')
  })
})

describe('ending a game', () => {
  it('asks first, starting in the reason field, and keeping the game sends nothing', async () => {
    const api = fakeApi([RUNNING])
    render(<LiveGames api={api} />)
    await userEvent.click(await screen.findByRole('button', { name: 'End Ann vs Bot' }))
    const dialog = await screen.findByRole('alertdialog', { name: 'End Ann vs Bot?' })
    expect(dialog).toHaveTextContent('has no winner')
    await waitFor(() => expect(within(dialog).getByLabelText('Reason (optional)')).toHaveFocus())
    await userEvent.click(within(dialog).getByRole('button', { name: 'Keep game' }))
    await waitFor(() => expect(screen.queryByRole('alertdialog')).toBeNull())
    expect(api.endGame).not.toHaveBeenCalled()
    expect(await rows()).toHaveLength(1)
  })

  it('sends the reason once, drops the row and says the game was ended', async () => {
    const api = fakeApi([LOBBY, RUNNING])
    render(<LiveGames api={api} />)
    await userEvent.click(await screen.findByRole('button', { name: 'End Ann vs Bot' }))
    const dialog = await screen.findByRole('alertdialog')
    await userEvent.type(within(dialog).getByLabelText('Reason (optional)'), 'stuck on a loop')
    await userEvent.click(within(dialog).getByRole('button', { name: 'End game' }))
    await waitFor(() => expect(screen.queryByRole('alertdialog')).toBeNull())
    expect(api.endGame).toHaveBeenCalledTimes(1)
    expect(api.endGame).toHaveBeenCalledWith(RUNNING.game, 'stuck on a loop')
    expect(await rows()).toHaveLength(1)
    expect(screen.queryByText('Ann vs Bot')).toBeNull()
    expect(screen.getByText('Ann vs Bot was ended.')).toBeInTheDocument()
  })

  it('keeps the dialog and the row when the server refuses', async () => {
    const api = fakeApi([RUNNING], () =>
      Promise.reject(new AccountError('unknown', 503, 'the server is busy')),
    )
    render(<LiveGames api={api} />)
    await userEvent.click(await screen.findByRole('button', { name: 'End Ann vs Bot' }))
    const dialog = await screen.findByRole('alertdialog')
    await userEvent.click(within(dialog).getByRole('button', { name: 'End game' }))
    expect(await within(dialog).findByText('the server is busy')).toBeInTheDocument()
    await userEvent.click(within(dialog).getByRole('button', { name: 'Keep game' }))
    await waitFor(() => expect(screen.queryByRole('alertdialog')).toBeNull())
    expect(await rows()).toHaveLength(1)
  })

  it('caps the reason at the length the server takes', async () => {
    render(<LiveGames api={fakeApi([RUNNING])} />)
    await userEvent.click(await screen.findByRole('button', { name: 'End Ann vs Bot' }))
    expect(screen.getByLabelText('Reason (optional)')).toHaveAttribute('maxlength', '200')
  })
})
