// Goal: the tournament page shows the organiser the invite and the step that fits (start, next round, finish), lets them set a result and drop an entrant, gives a player a link into their own game and a way out, lists standings with the tiebreakers, and shows why an action failed.
import { fireEvent, screen, waitFor, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { AuthProvider } from '@/features/auth/AuthProvider'
import { createAuthStore } from '@/features/auth/store'
import { TournamentError } from '@/net/tournaments'
import type { TournamentsApi } from '@/net/tournaments'
import type { TournamentView } from '@/protocol/generated'
import { startAccounts } from '@/test-utils/accountsServer'
import { renderInRouter } from '@/test-utils/router'
import { TournamentPage } from './TournamentPage'
import { TournamentsPage } from './TournamentsPage'

const ADA = { username: 'Ada_L', password: 'correct horse battery' }

function tournament(change: Partial<TournamentView> = {}): TournamentView {
  return {
    id: 't1',
    name: 'Friday Cup',
    format: 'commander_1v1',
    structure: 'swiss',
    status: 'running',
    round: 1,
    rounds: 2,
    organiser_name: 'Ada_L',
    organiser: true,
    invite: 'ABCDEFGH',
    playable: true,
    entrants: [
      { entrant: 0, name: 'Ada_L', players: ['Ada_L'], dropped: false, mine: true },
      { entrant: 1, name: 'Bob', players: ['Bob'], dropped: false, mine: false },
      { entrant: 2, name: 'Cyd', players: ['Cyd'], dropped: false, mine: false },
    ],
    rounds_played: [
      {
        round: 1,
        matches: [
          { table: 1, a: 0, b: 1, game: 'g1', outcome: null, corrected: false },
          { table: 2, a: 2, b: null, game: null, outcome: null, corrected: false },
        ],
      },
    ],
    standings: [
      {
        rank: 1,
        entrant: 2,
        points: 3,
        wins: 1,
        losses: 0,
        draws: 0,
        opponents_match_win: 0,
        game_win: 1,
        opponents_game_win: 0,
      },
    ],
    ...change,
  }
}

function fakeClient(view: TournamentView): TournamentsApi & Record<string, ReturnType<typeof vi.fn>> {
  return {
    list: vi.fn(() => Promise.resolve({ tournaments: [] })),
    create: vi.fn(() => Promise.resolve({ id: 't2', invite: 'ABCDEFGH' })),
    get: vi.fn(() => Promise.resolve(view)),
    invite: vi.fn(),
    join: vi.fn(),
    act: vi.fn(() => Promise.resolve(view)),
    leave: vi.fn(() => Promise.resolve()),
    dropEntrant: vi.fn(() => Promise.resolve(view)),
    setOutcome: vi.fn(() => Promise.resolve(view)),
    remove: vi.fn(() => Promise.resolve()),
  } as unknown as TournamentsApi & Record<string, ReturnType<typeof vi.fn>>
}

async function open(view: TournamentView, client = fakeClient(view)) {
  const server = startAccounts({ users: { [ADA.username]: ADA.password }, signedIn: ADA.username })
  const store = createAuthStore({ api: server.api })
  await renderInRouter(
    <AuthProvider store={store}>
      <TournamentPage id="t1" client={client} />
    </AuthProvider>,
  )
  await screen.findByRole('heading', { level: 1, name: 'Friday Cup' })
  return client
}

describe('the tournament page', () => {
  it('shows the round, the standings with tiebreakers and the player’s game', async () => {
    await open(tournament())
    expect(screen.getByRole('status')).toHaveTextContent('Round 1 of 2')
    const standings = screen.getByRole('table')
    expect(within(standings).getByRole('row', { name: /Cyd/ })).toHaveTextContent('1Cyd31-0-00.0%100.0%0.0%')
    const play = screen.getByRole('link', { name: 'Play your game' })
    expect(play).toHaveAttribute('href', '/game/g1')
    expect(screen.getByText('Cyd has a bye')).toBeInTheDocument()
  })

  it('lets the organiser set a result and drop an entrant', async () => {
    const client = await open(tournament())
    fireEvent.change(screen.getByRole('combobox', { name: 'Result of table 1' }), { target: { value: 'b' } })
    await waitFor(() => expect(client.setOutcome).toHaveBeenCalledWith('t1', 1, 1, 'b'))
    vi.spyOn(window, 'confirm').mockReturnValue(true)
    fireEvent.click(screen.getByRole('button', { name: 'Drop Bob' }))
    await waitFor(() => expect(client.dropEntrant).toHaveBeenCalledWith('t1', 1))
  })

  it('waits for every result before the next round', async () => {
    const client = await open(tournament())
    const next = screen.getByRole('button', { name: 'Pair next round' })
    expect(next).toHaveAttribute('aria-disabled', 'true')
    fireEvent.click(next)
    expect(client.act).not.toHaveBeenCalled()
  })

  it('finishes after the last round once it is decided', async () => {
    const decided = tournament({
      round: 2,
      rounds_played: [
        { round: 2, matches: [{ table: 1, a: 0, b: 1, game: 'g2', outcome: 'a', corrected: false }] },
      ],
    })
    const client = await open(decided)
    fireEvent.click(screen.getByRole('button', { name: 'Finish tournament' }))
    await waitFor(() => expect(client.act).toHaveBeenCalledWith('t1', 'next'))
  })

  it('shows the invite in sign-up and says why a start failed', async () => {
    const signup = tournament({ status: 'signup', round: 0, rounds: null, rounds_played: [], standings: [] })
    const client = fakeClient(signup)
    client.act = vi.fn(() =>
      Promise.reject(new TournamentError('not_enough_entrants', 409, 'a team is missing a player')),
    )
    await open(signup, client)
    expect(screen.getByText(/\/tournaments\/join\/ABCDEFGH$/)).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Start round 1' }))
    expect(await screen.findByText('a team is missing a player')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Leave' })).toBeInTheDocument()
  })

  it('tells a player of a team tournament that it cannot start yet', async () => {
    await open(
      tournament({ status: 'signup', round: 0, playable: false, format: 'commander_2v2', rounds_played: [] }),
    )
    expect(screen.getByText(/Team games are not available/)).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Start round 1' })).toBeDisabled()
  })

  it('gives a player without organiser rights no controls', async () => {
    await open(tournament({ organiser: false, invite: null }))
    expect(screen.queryByRole('heading', { name: 'Organiser' })).toBeNull()
    expect(screen.queryByRole('combobox')).toBeNull()
    expect(screen.getByRole('button', { name: 'Drop out' })).toBeInTheDocument()
  })
})

describe('the tournaments page', () => {
  it('creates a Swiss tournament with the rounds chosen', async () => {
    const client = fakeClient(tournament())
    const server = startAccounts({ users: { [ADA.username]: ADA.password }, signedIn: ADA.username })
    const store = createAuthStore({ api: server.api })
    await renderInRouter(
      <AuthProvider store={store}>
        <TournamentsPage client={client} />
      </AuthProvider>,
    )
    expect(await screen.findByText('No tournaments yet')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Create tournament' }))
    expect(await screen.findByText('Give the tournament a name.')).toBeInTheDocument()
    fireEvent.change(screen.getByLabelText('Name'), { target: { value: ' Friday ' } })
    fireEvent.change(screen.getByLabelText('Rounds'), { target: { value: '4' } })
    fireEvent.click(screen.getByRole('button', { name: 'Create tournament' }))
    await waitFor(() =>
      expect(client.create).toHaveBeenCalledWith({
        name: 'Friday',
        format: 'commander_1v1',
        structure: 'swiss',
        rounds: 4,
      }),
    )
  })
})
