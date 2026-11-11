// Goal: the review screen joins a deck's cards to read and its cards played by hand in one list with parts To read and To check, a progress bar and a count; J, K and the arrows move between cards and ? shows the keys; a card to check shows its Oracle text with line marks, C marks it checked (kept over a reload) and H plays it by hand in its only deck; the wording-only cards are approved in bulk from one offer; and `/read` forwards to `/review` with its parameters.
import { act, fireEvent, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { AuthContext } from '@/features/auth/context'
import { createAuthStore, offlineAccounts } from '@/features/auth/store'
import type { ByHandApi } from '@/features/decks/byHand'
import { fixtureCardsApi, fixtureDecksApi } from '@/features/decks/unvalidated/fixture'
import { fixtureView } from '@/features/precons/dev/fixture'
import { CardsApiContext } from '@/features/table/cardsContext'
import { cardsApi } from '@/net/cards'
import { renderApp } from '@/test-utils/appHarness'

import { renderInRouter } from '@/test-utils/router'
import { READ_CARDS, fixtureReadBulkApi, fixtureReadsApi } from './dev/fixture'
import { ReadPage } from './ReadPage'

const ADA = { username: 'Ada_L', password: 'correct horse battery' }

const [FIRST, SECOND] = Object.values(READ_CARDS) as [
  (typeof READ_CARDS)[string],
  (typeof READ_CARDS)[string],
]

function signedIn() {
  return createAuthStore({
    api: { ...offlineAccounts, myGames: async () => [] },
    initial: { status: 'signed-in', user: { id: 'ada', username: 'Ada_L', is_admin: false } },
  })
}

async function renderReview() {
  const decks = fixtureDecksApi()
  const update = vi.fn((id: string) => decks.get(id))
  const byHand: ByHandApi = { get: decks.get, update }
  const precons = {
    get: async (code: string) => fixtureView(code) ?? Promise.reject(new Error('no such precon')),
  }
  const page = (
    <AuthContext value={signedIn()}>
      <CardsApiContext value={{ ...cardsApi, get: fixtureCardsApi().get }}>
        <ReadPage
          target={{ deck: 'deck-izzet' }}
          api={fixtureReadsApi()}
          decks={byHand}
          checks={{ decks, precons }}
          bulk={fixtureReadBulkApi()}
        />
      </CardsApiContext>
    </AuthContext>
  )
  const view = await renderInRouter(page)
  await screen.findByRole('article')
  await screen.findByRole('region', { name: /^To check/ })
  return { update, view }
}

const list = () => screen.getByRole('navigation', { name: 'Cards to review' })
const current = () => within(list()).getByRole('button', { current: true })
const progress = () => screen.getByRole('progressbar', { name: 'Review progress' })
const press = (key: string) => fireEvent.keyDown(document.body, { key })

afterEach(() => {
  window.localStorage.clear()
})

describe('the review', () => {
  it('lists the cards to read and then the cards to check, with the progress and the count', async () => {
    await renderReview()
    const toRead = within(list()).getByRole('region', { name: /^To read/ })
    const toCheck = within(list()).getByRole('region', { name: /^To check/ })
    expect(toRead).toHaveTextContent('3 left')
    expect(
      within(toCheck)
        .getAllByRole('button')
        .map((row) => row.textContent),
    ).toEqual([
      expect.stringContaining('Delver of Secrets'),
      expect.stringContaining('Mana Drain'),
      expect.stringContaining('Rhystic Study'),
    ])
    expect(within(toCheck).getByRole('button', { name: /Rhystic Study/ })).toHaveTextContent('Played by hand')
    expect(progress()).toHaveAttribute('aria-valuenow', '0')
    expect(progress()).toHaveAttribute('aria-valuemax', '6')
    expect(screen.getByText('0 of 6 reviewed')).toBeInTheDocument()
    expect(document.querySelector('[data-slot="read-count"]')).toHaveTextContent('3 to read, 3 to check.')
    expect(document.querySelector('[data-slot="review-picture"]')).not.toBeNull()
  })

  it('moves with J, K and the arrows, wrapping round, and shows the keys with ?', async () => {
    await renderReview()
    press('j')
    await waitFor(() => expect(current()).toHaveTextContent(SECOND.name))
    press('ArrowLeft')
    await waitFor(() => expect(current()).toHaveTextContent(FIRST.name))
    press('k')
    await waitFor(() => expect(current()).toHaveTextContent('Rhystic Study'))
    expect(await screen.findByRole('heading', { level: 2, name: 'Rhystic Study' })).toBeVisible()
    press('ArrowRight')
    await waitFor(() => expect(current()).toHaveTextContent(FIRST.name))
    expect(document.querySelector('[data-slot="review-keys"]')).toBeNull()
    press('?')
    expect(await screen.findByText('Play by hand in the card’s only deck')).toBeVisible()
    expect(screen.getByRole('button', { name: /^Keys/ })).toHaveAttribute('aria-expanded', 'true')
  })

  it('checks a card with C and keeps it over a reload, and plays a card by hand in its deck with H', async () => {
    const { update, view } = await renderReview()
    fireEvent.click(within(list()).getByRole('button', { name: /Mana Drain/ }))
    const card = await screen.findByRole('article')
    expect(card).toHaveAttribute('data-slot', 'check-card')
    expect(await within(card).findByRole('group', { name: 'Lines of the card' })).toBeVisible()
    expect(
      within(card).getByRole('button', { name: 'Play Mana Drain by hand in Izzet Spells' }),
    ).toBeVisible()
    press('c')
    await waitFor(() => expect(screen.getByText('1 of 6 reviewed')).toBeInTheDocument())
    expect(within(list()).getByRole('button', { name: /Mana Drain/ })).toHaveAttribute('data-tone', 'checked')
    await waitFor(() => expect(current()).toHaveTextContent('Rhystic Study'))
    press('h')
    await waitFor(() => expect(update).toHaveBeenCalledWith('deck-izzet', expect.anything()))
    expect(await screen.findByText('Played by hand in Izzet Spells')).toBeVisible()

    view.unmount()
    await renderReview()
    expect(screen.getByText('1 of 6 reviewed')).toBeInTheDocument()
    expect(within(list()).getByRole('button', { name: /Mana Drain/ })).toHaveTextContent('Checked')
  })

  it('approves the wording-only cards in bulk from one offer, which then leaves', async () => {
    await renderReview()
    fireEvent.click(await screen.findByRole('button', { name: 'Approve 1 wording-only' }))
    const dialog = await screen.findByRole('dialog', { name: 'Approve wording-only reads' })
    expect(within(dialog).getByRole('checkbox', { name: FIRST.name })).toBeChecked()
    fireEvent.click(within(dialog).getByRole('button', { name: 'Approve 1' }))
    await waitFor(() =>
      expect(within(list()).getByRole('button', { name: new RegExp(FIRST.name) })).toHaveTextContent(
        'You said it reads right',
      ),
    )
    expect(screen.getByText('1 of 6 reviewed')).toBeInTheDocument()
    await waitFor(() => expect(screen.queryByRole('button', { name: /wording-only$/ })).toBeNull())
  })
})

describe('the old address', () => {
  it.each([
    ['/read?deck=d1', { deck: 'd1' }],
    ['/read?mine=true', { mine: true }],
    ['/read', {}],
  ])('%s forwards to the review with the same cards', async (url, search) => {
    const app = await renderApp(url, {
      accounts: { users: { [ADA.username]: ADA.password }, signedIn: ADA.username },
    })
    await act(async () => {})
    await waitFor(() => expect(app.router.state.location.pathname).toBe('/review'))
    expect(app.router.state.location.search).toEqual(search)
  })
})
