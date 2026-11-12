// Goal: an invite shows what it signs up for; a player signs up alone, starts a team or takes the open place of one; a started tournament and an unknown invite say so.
import { fireEvent, screen, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { AuthProvider } from '@/features/auth/AuthProvider'
import { createAuthStore } from '@/features/auth/store'
import { TournamentError } from '@/net/tournaments'
import type { TournamentsApi } from '@/net/tournaments'
import type { TournamentInvite } from '@/protocol/generated'
import { startAccounts } from '@/test-utils/accountsServer'
import { renderInRouter } from '@/test-utils/router'
import { JoinTournamentPage } from './JoinTournamentPage'

const ADA = { username: 'Ada_L', password: 'correct horse battery' }

function invite(change: Partial<TournamentInvite> = {}): TournamentInvite {
  return {
    id: 't1',
    name: 'Pairs Night',
    format: 'commander_2v2',
    structure: 'swiss',
    status: 'signup',
    organiser_name: 'Org',
    entrants: [
      { entrant: 0, name: 'Red', players: ['Ann'], dropped: false, mine: false },
      { entrant: 1, name: 'Blue', players: ['Bob', 'Cyd'], dropped: false, mine: false },
    ],
    joined: false,
    ...change,
  }
}

async function open(answer: Promise<TournamentInvite>) {
  const join = vi.fn(() => Promise.resolve({ id: 't1' }))
  const client = { invite: vi.fn(() => answer), join } as unknown as TournamentsApi
  const server = startAccounts({ users: { [ADA.username]: ADA.password }, signedIn: ADA.username })
  const store = createAuthStore({ api: server.api })
  await renderInRouter(
    <AuthProvider store={store}>
      <JoinTournamentPage code="ABCDEFGH" client={client} />
    </AuthProvider>,
  )
  return join
}

describe('the invite page', () => {
  it('starts a named team', async () => {
    const join = await open(Promise.resolve(invite()))
    expect(await screen.findByRole('heading', { name: 'Pairs Night' })).toBeInTheDocument()
    fireEvent.change(screen.getByLabelText('Team name (optional)'), { target: { value: ' Green ' } })
    fireEvent.click(screen.getByRole('button', { name: 'Sign up' }))
    await waitFor(() => expect(join).toHaveBeenCalledWith('ABCDEFGH', { team_name: 'Green' }))
  })

  it('offers only teams with an open place', async () => {
    const join = await open(Promise.resolve(invite()))
    await screen.findByRole('heading', { name: 'Pairs Night' })
    expect(screen.queryByLabelText(/Join Blue/)).toBeNull()
    fireEvent.click(screen.getByLabelText('Join Red (Ann)'))
    fireEvent.click(screen.getByRole('button', { name: 'Sign up' }))
    await waitFor(() => expect(join).toHaveBeenCalledWith('ABCDEFGH', { team: 0 }))
  })

  it('signs up a player alone in 1v1', async () => {
    const join = await open(Promise.resolve(invite({ format: 'commander_1v1', entrants: [] })))
    fireEvent.click(await screen.findByRole('button', { name: 'Sign up' }))
    await waitFor(() => expect(join).toHaveBeenCalledWith('ABCDEFGH', { team: undefined }))
  })

  it('says when sign-up is closed', async () => {
    await open(Promise.resolve(invite({ status: 'running' })))
    expect(await screen.findByText(/sign-up is closed/)).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Sign up' })).toBeNull()
  })

  it('says when an invite opens nothing', async () => {
    await open(Promise.reject(new TournamentError('not_found', 404, 'no such tournament')))
    expect(await screen.findByText(/This invite opens no tournament/)).toBeInTheDocument()
  })
})
