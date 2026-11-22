// Goal: the Card support page shows a signed-in account the headline share, the states, the popularity buckets, the trend, the feed newest first and the searchable list of what is missing; it says when nothing is counted yet, keeps what it shows when a refresh fails with Try again, loads the missing cards again only for a new snapshot, and its client refuses an answer it cannot read.
import { act, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { createAuthStore } from '@/features/auth/store'
import { AuthContext } from '@/features/auth/context'
import { createCardSupportApi, readMissing, readOverview } from '@/net/cardSupport'
import type { CardSupportApi } from '@/net/cardSupport'
import type { CardSupport, MissingCards } from '@/protocol/generated'
import { startAccounts } from '@/test-utils/accountsServer'
import { CardSupportPage } from './CardSupportPage'
import { PAGE_ROWS } from './MissingList'

afterEach(() => {
  vi.useRealTimers()
})

const overview = (id: number, automatic = 947): CardSupport => ({
  snapshot: {
    id,
    taken_at: '2026-11-22T04:10:00Z',
    release: 'v0.2.131',
    cards: 1000,
    automatic,
    states: { verified: 800, read: 100, unverified: 47, waiting: 3, partial: 30, manual: 15, other: 5 },
    buckets: [
      { label: 'top 100', cards: 100, verified: 95, automatic: 99 },
      { label: 'all ranked', cards: 900, verified: 750, automatic: 860 },
    ],
    missing: 53,
  },
  history: [
    { taken_at: '2026-11-20T04:00:00Z', release: 'v0.2.130', cards: 1000, automatic: 900 },
    { taken_at: '2026-11-22T04:10:00Z', release: 'v0.2.131', cards: 1000, automatic },
  ],
  last_check: {
    started_at: '2026-11-22T04:00:00Z',
    finished_at: '2026-11-22T04:10:00Z',
    state: 'succeeded',
    scryfall_updated_at: null,
    mtgjson_build: '5.3.0+20261122',
  },
  feed: [
    {
      kind: 'now_automatic',
      at: '2026-11-22T04:10:00Z',
      release: 'v0.2.131',
      new_release: true,
      count: 170,
      names: [],
    },
    {
      kind: 'new_precons',
      at: '2026-11-22T04:00:00Z',
      code: 'STC',
      set_name: 'Star Trek Commander',
      decks: ['A', 'B', 'C', 'D'],
    },
    { kind: 'new_cards', at: '2026-11-22T04:00:00Z', count: 289, names: ['Kirk'] },
  ],
})

const missing = (id: number, size = 60): MissingCards => ({
  snapshot_id: id,
  cards: Array.from({ length: size }, (_, at) => ({
    oracle_id: `id-${at}`,
    name: at === 0 ? 'Æther Vial' : `Card ${at}`,
    state: at % 2 === 0 ? 'partial' : 'manual',
    blocker: at % 2 === 0 ? 'A triggered ability the game cannot read yet' : 'No rules yet',
    detail: '',
    blockers: 1,
    rank: at + 1,
  })),
})

function fakeApi(answers: { overview: Array<CardSupport | Error>; missing: MissingCards[] }) {
  const calls = { overview: 0, missing: 0 }
  const api: CardSupportApi = {
    overview: () => {
      const answer = answers.overview[Math.min(calls.overview, answers.overview.length - 1)]
      calls.overview += 1
      return answer instanceof Error ? Promise.reject(answer) : Promise.resolve(answer as CardSupport)
    },
    missing: () => {
      const answer = answers.missing[Math.min(calls.missing, answers.missing.length - 1)]
      calls.missing += 1
      return Promise.resolve(answer as MissingCards)
    },
  }
  return { api, calls }
}

const share = (): string | null | undefined => document.getElementById('support-share')?.textContent

async function open(api: CardSupportApi) {
  const accounts = startAccounts({ users: { ann: 'a long enough password' }, signedIn: 'ann' })
  const store = createAuthStore({ api: accounts.api })
  await store.getState().start()
  render(
    <AuthContext value={store}>
      <CardSupportPage api={api} />
    </AuthContext>,
  )
}

describe('the Card support page', () => {
  it('shows the share, states, buckets, trend, feed and missing cards', async () => {
    const { api } = fakeApi({ overview: [overview(7)], missing: [missing(7)] })
    await open(api)
    await waitFor(() => expect(share()).toBe('94.7%'))
    expect(document.title).toBe('Card support · Magic')
    expect(screen.getByRole('heading', { level: 1, name: 'Card support' })).toBeInTheDocument()
    const states = screen.getByRole('list', { name: 'Cards by state' })
    expect(within(states).getAllByRole('listitem')).toHaveLength(7)
    expect(within(states).getByText('800 · 80.0%')).toBeInTheDocument()
    expect(screen.getByText('99.0%')).toBeInTheDocument()
    expect(screen.getByRole('img', { name: /From 90\.0% on 20 Nov to 94\.7% on 22 Nov/ })).toBeInTheDocument()
    const feed = within(screen.getByRole('region', { name: 'What changed' }))
      .getAllByRole('listitem')
      .map((item) => item.querySelector('p')?.textContent)
    expect(feed).toEqual([
      'v0.2.131: 170 cards now automatic',
      'Star Trek Commander: 4 precons added',
      '289 new cards',
    ])
    const list = await screen.findByRole('region', { name: 'What we are missing' })
    expect(within(list).getAllByRole('listitem')).toHaveLength(PAGE_ROWS)
  })

  it('searches without accents, filters by state and pages the rows', async () => {
    const user = userEvent.setup()
    const { api } = fakeApi({ overview: [overview(7)], missing: [missing(7)] })
    await open(api)
    const list = await screen.findByRole('region', { name: 'What we are missing' })
    await user.click(within(list).getByRole('button', { name: 'Show 10 more' }))
    expect(within(list).getAllByRole('listitem')).toHaveLength(60)
    await user.type(within(list).getByRole('searchbox'), 'aether')
    expect(
      within(list)
        .getAllByRole('listitem')
        .map((row) => row.textContent),
    ).toEqual([expect.stringContaining('Æther Vial')])
    await user.clear(within(list).getByRole('searchbox'))
    await user.click(within(list).getByRole('button', { name: /^Manual/ }))
    expect(within(list).getByRole('button', { name: /^Manual/ })).toHaveAttribute('aria-pressed', 'true')
    expect(within(list).getByText('30 of 60 cards match.')).toBeInTheDocument()
  })

  it('says when nothing is counted yet', async () => {
    const empty: CardSupport = { snapshot: null, history: [], last_check: null, feed: [] }
    const { api, calls } = fakeApi({ overview: [empty], missing: [] })
    await open(api)
    expect(await screen.findByText('No numbers yet')).toBeInTheDocument()
    expect(calls.missing).toBe(0)
  })

  it('keeps the numbers when a refresh fails, and loads the missing cards again only for a new snapshot', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true })
    const { api, calls } = fakeApi({
      overview: [overview(7), new Error('offline'), overview(7), overview(8, 950)],
      missing: [missing(7), missing(8, 3)],
    })
    await open(api)
    await waitFor(() => expect(share()).toBe('94.7%'))
    await act(async () => {
      await vi.advanceTimersByTimeAsync(60_000)
    })
    expect(await screen.findByText('Could not refresh. offline')).toBeInTheDocument()
    expect(share()).toBe('94.7%')
    await act(async () => {
      screen.getByRole('button', { name: 'Try again' }).click()
      await vi.advanceTimersByTimeAsync(0)
    })
    await waitFor(() => expect(screen.queryByText(/Could not refresh/)).not.toBeInTheDocument())
    expect(calls.missing).toBe(1)
    await act(async () => {
      await vi.advanceTimersByTimeAsync(60_000)
    })
    await waitFor(() => expect(share()).toBe('95.0%'))
    await waitFor(() => expect(calls.missing).toBe(2))
    expect(screen.getByText('Card support was updated with new numbers.')).toBeInTheDocument()
  })
})

describe('the client', () => {
  it('reads the answers it knows and refuses the rest', () => {
    expect(readOverview(overview(1))).toEqual(overview(1))
    expect(readMissing(missing(1, 2))).toEqual(missing(1, 2))
    expect(readOverview({ ...overview(1), feed: [{ kind: 'unknown', at: 'x' }] })).toBeNull()
    expect(readOverview({ ...overview(1), snapshot: { id: 1 } })).toBeNull()
    expect(readMissing({ snapshot_id: null, cards: [{ name: 'x' }] })).toBeNull()
  })

  it('sends the session and turns a refusal into a DeckError', async () => {
    const fetch = vi.fn(
      async () =>
        new Response(JSON.stringify({ error: 'unauthenticated', message: 'Sign in.' }), { status: 401 }),
    )
    const api = createCardSupportApi({ fetch })
    await expect(api.overview()).rejects.toMatchObject({ code: 'unauthenticated', status: 401 })
    expect(fetch).toHaveBeenCalledWith('/api/card-support', { credentials: 'include' })
  })
})
