// Goal: the decks page sends a visitor who is not signed in to the sign-in form and back, lists the account's decks in the server's order with name, format, card count, age and the check state as an icon and a word, explains the state in one line, handles an empty list, a server without decks (a notice, not a crash), a failed load with Retry and an ended session, and offers Export as links to the three file kinds.
import { screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { decksApi } from '@/net/decks'
import type { DeckState } from '@/protocol/generated'
import { renderApp } from '@/test-utils/appHarness'
import { counts, deckError, deckIn, deckItem, summary } from './fakeDecks'
import { deckList, deckRows, renderDecks, requests, rowOf } from './renderDecks'

afterEach(() => {
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

const ADA = { username: 'Ada_L', password: 'correct horse battery' }
const users = { [ADA.username]: ADA.password }

describe('a visitor who is not signed in', () => {
  it('is sent to the sign-in form, which remembers where they were going, and no deck is asked for', async () => {
    const list = vi.spyOn(decksApi, 'list')
    const app = await renderApp('/decks', { accounts: { users } })
    expect(await screen.findByRole('form', { name: 'Sign in' })).toBeInTheDocument()
    expect(app.router.state.location.pathname).toBe('/login')
    expect(app.router.state.matches.at(-1)?.search).toEqual({ next: '/decks' })
    expect(list).not.toHaveBeenCalled()
  })

  it('comes back to the page after signing in', async () => {
    vi.spyOn(decksApi, 'list').mockResolvedValue([deckItem()])
    const app = await renderApp('/decks', { accounts: { users } })
    await screen.findByRole('form', { name: 'Sign in' })
    await app.user.type(screen.getByLabelText('Username'), ADA.username)
    await app.user.type(screen.getByLabelText('Password'), ADA.password)
    await app.user.click(screen.getByRole('button', { name: 'Sign in' }))
    expect(await screen.findByRole('list', { name: 'Your decks' })).toBeInTheDocument()
    expect(app.router.state.location.pathname).toBe('/decks')
  })
})

describe('before the server has said who is signed in', () => {
  it('says it is checking, and shows no list and no redirect', async () => {
    const app = await renderApp('/decks', {
      accounts: { users, override: { me: () => new Promise<Response>(() => undefined) } },
    })
    expect(screen.getByRole('status')).toHaveTextContent('Checking your sign-in…')
    expect(app.router.state.location.pathname).toBe('/decks')
    expect(screen.queryByRole('list')).toBeNull()
  })
})

describe('a server without accounts', () => {
  it('explains that there are no decks and points at creating a game', async () => {
    const list = vi.spyOn(decksApi, 'list')
    await renderApp('/decks', { accounts: { users, unavailable: true } })
    expect(await screen.findByText(/this server has no accounts, so it keeps no decks/i)).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'Create a game' })).toHaveAttribute('href', '/')
    expect(list).not.toHaveBeenCalled()
  })
})

describe('the page in the app', () => {
  it('is titled, linked from the main navigation between Precons and My games, and marked as the current page', async () => {
    vi.spyOn(decksApi, 'list').mockResolvedValue([deckItem()])
    const app = await renderApp('/decks', { accounts: { users, signedIn: ADA.username } })
    await screen.findByRole('list', { name: 'Your decks' })
    expect(document.title).toBe('Decks · Magic')
    const nav = screen.getByRole('navigation', { name: 'Main' })
    expect(
      within(nav)
        .getAllByRole('link')
        .map((link) => link.textContent),
    ).toEqual(['Play', 'Precons', 'Decks', 'My games', 'Create'])
    expect(within(nav).getByRole('link', { name: 'Decks' })).toHaveAttribute('aria-current', 'page')
    expect(within(nav).getByRole('link', { name: 'My games' })).not.toHaveAttribute('aria-current')
    expect(app.router.state.location.pathname).toBe('/decks')
  })

  it('goes home, not to the sign-in form, when the user signs out on it', async () => {
    vi.spyOn(decksApi, 'list').mockResolvedValue([deckItem()])
    const app = await renderApp('/decks', { accounts: { users, signedIn: ADA.username } })
    await screen.findByRole('list', { name: 'Your decks' })
    await app.user.click(screen.getByRole('button', { name: /signed in as/i }))
    await app.user.click(await screen.findByRole('menuitem', { name: 'Sign out' }))
    await waitFor(() => expect(app.store.getState().status).toBe('signed-out'))
    await waitFor(() => expect(app.router.state.location.pathname).toBe('/'))
  })
})

describe('the list', () => {
  it('asks the server once and shows the decks in its order with name, format, size and age', async () => {
    const { server } = await renderDecks({
      decks: [
        deckItem({ id: 'a', name: 'Atraxa Superfriends' }),
        deckItem({
          id: 'b',
          name: 'Burn',
          format: 'modern',
          updated_at: '2026-10-18T11:30:00Z',
          summary: summary({ format: 'modern', counts: counts({ total: 60, verified: 60 }) }),
        }),
        deckItem({ id: 'c', name: 'Old Pauper', format: 'pauper', updated_at: '2026-08-01T00:00:00Z' }),
      ],
    })
    await screen.findByRole('list', { name: 'Your decks' })
    expect(requests(server.calls)).toEqual(['GET /api/decks'])
    expect(deckRows()).toHaveLength(3)
    expect(deckRows()[0]).toHaveTextContent('Atraxa Superfriends')
    expect(deckRows()[0]).toHaveTextContent('Commander · 100 cards · Saved 1 day ago')
    expect(deckRows()[1]).toHaveTextContent('Burn')
    expect(deckRows()[1]).toHaveTextContent('Modern · 60 cards · Saved 30 minutes ago')
    expect(deckRows()[2]).toHaveTextContent('Pauper · 100 cards · Saved on 1 Aug 2026')
    expect(screen.getByText('3 decks')).toBeInTheDocument()
  })

  it('counts one deck as one deck and one card as one card', async () => {
    await renderDecks({
      decks: [deckItem({ summary: summary({ counts: counts({ total: 1, verified: 1 }) }) })],
    })
    await screen.findByRole('list', { name: 'Your decks' })
    expect(screen.getByText('1 deck')).toBeInTheDocument()
    expect(deckRows()[0]).toHaveTextContent('Commander · 1 card · Saved')
  })

  it('shows a format key it does not know as it is', async () => {
    await renderDecks({ decks: [deckItem({ format: 'brawl' })] })
    await screen.findByRole('list', { name: 'Your decks' })
    expect(deckRows()[0]).toHaveTextContent('brawl · 100 cards')
  })

  it('keeps a long name inside its row', async () => {
    const name = 'Atraxa Praetors Voice Superfriends '.repeat(3).trim()
    await renderDecks({ decks: [deckItem({ name })] })
    await screen.findByRole('list', { name: 'Your decks' })
    expect(
      within(deckRows()[0] as HTMLElement)
        .getByText(name)
        .closest('p'),
    ).toHaveClass('[overflow-wrap:anywhere]')
  })

  it('says it is loading while the first answer is awaited', async () => {
    await renderDecks({ override: { list: () => new Promise<Response>(() => undefined) } })
    expect(screen.getByText('Loading your decks…').closest('[role="status"]')).toHaveAttribute(
      'aria-busy',
      'true',
    )
    expect(screen.queryByRole('list', { name: 'Your decks' })).toBeNull()
  })
})

describe('the check state of a deck', () => {
  const WORDS: ReadonlyArray<readonly [DeckState, string]> = [
    ['verified', 'Verified'],
    ['read', 'Runs on reads'],
    ['manual_play', 'Played by hand'],
    ['blocked', 'Blocked'],
    ['unchecked', 'Unchecked'],
  ]

  it.each(WORDS)('shows %s as an icon and the word %s', async (state, word) => {
    await renderDecks({ decks: [deckIn(state, { name: 'Probe', changed: false })] })
    await screen.findByRole('list', { name: 'Your decks' })
    const badge = rowOf('Probe').querySelector('[data-deck-state]')
    expect(badge).toHaveAttribute('data-deck-state', state)
    expect(badge).toHaveTextContent(new RegExp(`^${word}$`))
    expect(badge?.querySelector('svg')).toHaveAttribute('aria-hidden', 'true')
  })

  it('says "Changed since the last check" beside the name of a deck whose stored check is stale', async () => {
    await renderDecks({ decks: [deckIn('unchecked', { name: 'Probe', changed: true })] })
    await screen.findByRole('list', { name: 'Your decks' })
    const marker = rowOf('Probe').querySelector('[data-marker]')
    expect(marker).toHaveAttribute('data-marker', 'changed')
    expect(marker).toHaveTextContent(/^Changed since the last check$/)
    expect(marker?.querySelector('svg')).toHaveAttribute('aria-hidden', 'true')
  })

  it('gives every state its own icon, so colour never carries the meaning alone', async () => {
    const decks = [
      ...WORDS.map(([state]) => deckIn(state, { id: state, name: state, changed: false })),
      deckIn('unchecked', { id: 'changed', name: 'changed', changed: true }),
    ]
    await renderDecks({ decks })
    await screen.findByRole('list', { name: 'Your decks' })
    const icons = decks.map((deck) =>
      rowOf(deck.name).querySelector('[data-marker] svg')?.getAttribute('class'),
    )
    expect(icons.every((icon) => (icon ?? '').includes('lucide-'))).toBe(true)
    expect(new Set(icons).size).toBe(decks.length)
  })

  it.each([
    [
      'a deck played by hand says how much is verified',
      deckIn('manual_play', {
        summary: summary({ state: 'manual_play', counts: counts({ verified: 96, manual: 3, partial: 1 }) }),
      }),
      '96 of 100 verified, 4 played by hand',
    ],
    [
      'a blocked deck says what blocks it',
      deckIn('blocked', {
        summary: summary({ state: 'blocked', issues: 2, counts: counts({ unplayable: 1 }) }),
      }),
      '2 legality issues, 1 unplayable card. Cannot be taken to a game until its issues are fixed.',
    ],
    [
      'a changed deck says it changed',
      deckIn('unchecked', { changed: true }),
      'Changed since the last check',
    ],
    ['a deck never checked says so', deckIn('unchecked', { changed: false }), 'Not checked yet'],
  ])('%s', async (_what, deck, note) => {
    await renderDecks({ decks: [{ ...deck, name: 'Probe' }] })
    await screen.findByRole('list', { name: 'Your decks' })
    expect(within(rowOf('Probe')).getByText(note)).toBeInTheDocument()
  })

  it('says how many cards wait for a read, with the address of the deck, and nothing for a deck with none', async () => {
    await renderDecks({
      decks: [
        deckIn('manual_play', {
          id: 'w1',
          name: 'Waiting',
          summary: summary({
            state: 'manual_play',
            counts: counts({ verified: 96, manual: 4 }),
            to_read: 3,
          }),
        }),
        deckIn('manual_play', {
          id: 'p1',
          name: 'Plain',
          summary: summary({ state: 'manual_play', counts: counts({ verified: 96, manual: 4 }) }),
        }),
      ],
    })
    await screen.findByRole('list', { name: 'Your decks' })
    expect(
      within(rowOf('Waiting')).getByText('3 cards to read').closest('[data-slot="to-read"]'),
    ).toHaveAttribute('data-read-path', '/read?deck=w1')
    expect(rowOf('Plain').querySelector('[data-slot="to-read"]')).toBeNull()
  })

  it('adds no line to a verified deck', async () => {
    await renderDecks({ decks: [deckIn('verified', { name: 'Probe' })] })
    await screen.findByRole('list', { name: 'Your decks' })
    expect(rowOf('Probe').querySelectorAll('p')).toHaveLength(2)
  })
})

describe('an empty list', () => {
  it('says there are no decks and offers to import or start one', async () => {
    await renderDecks({ decks: [] })
    expect(await screen.findByRole('heading', { name: 'You have no decks yet' })).toBeInTheDocument()
    expect(screen.queryByRole('list', { name: 'Your decks' })).toBeNull()
    const empty = document.querySelector<HTMLElement>('[data-slot="empty-state"]')
    expect(within(empty as HTMLElement).getByRole('button', { name: 'Import deck' })).toBeInTheDocument()
    expect(within(empty as HTMLElement).getByRole('button', { name: 'New deck' })).toBeInTheDocument()
    expect(screen.getByText('0 decks')).toBeInTheDocument()
  })
})

describe('a server without decks', () => {
  it('shows a notice, not an error, with no list and no way to add a deck', async () => {
    await renderDecks({ unavailable: true })
    expect(await screen.findByText('This server keeps no decks')).toBeInTheDocument()
    expect(screen.queryByRole('alert')).toBeNull()
    expect(screen.getByText('This server keeps no decks').closest('[data-slot="alert"]')).toHaveAttribute(
      'role',
      'status',
    )
    expect(screen.queryByRole('list')).toBeNull()
    expect(screen.queryByRole('button', { name: 'Import deck' })).toBeNull()
    expect(screen.queryByRole('button', { name: 'New deck' })).toBeNull()
    expect(screen.getByRole('link', { name: 'Create a game' })).toHaveAttribute('href', '/')
    expect(screen.getByRole('heading', { level: 1, name: 'Decks' })).toBeInTheDocument()
  })
})

describe('a list that cannot be loaded', () => {
  it('words the failure, keeps the page around it, and loads again on Retry', async () => {
    let attempts = 0
    const { user } = await renderDecks({
      decks: [deckItem()],
      override: {
        list: () => {
          attempts += 1
          return attempts === 1 ? deckError(500, 'internal', 'boom') : undefined
        },
      },
    })
    const alert = await screen.findByRole('alert')
    expect(alert).toHaveTextContent('Could not load your decks')
    expect(alert).toHaveTextContent('The server ran into a problem. Try again in a moment.')
    expect(screen.getByRole('heading', { level: 1, name: 'Decks' })).toBeInTheDocument()
    expect(screen.queryByRole('list', { name: 'Your decks' })).toBeNull()
    await user.click(within(alert).getByRole('button', { name: 'Retry' }))
    expect(await screen.findByRole('list', { name: 'Your decks' })).toBeInTheDocument()
    expect(attempts).toBe(2)
    expect(screen.queryByRole('alert')).toBeNull()
  })

  it.each([
    [
      'no answer at all',
      () => Promise.reject(new TypeError('Failed to fetch')),
      /Could not reach the server/,
    ],
    [
      'an HTML page from a proxy',
      () => new Response('<html>', { status: 502 }),
      /could not read \(status 502\)/,
    ],
    ['a server without a card database', () => deckError(503, 'cards_unavailable', 'x'), /no card database/],
    [
      'a refused origin',
      () => deckError(403, 'forbidden_origin', 'x'),
      /does not accept requests from this web address/,
    ],
  ])('words %s', async (_what, answer, words) => {
    await renderDecks({ override: { list: answer } })
    expect(await screen.findByRole('alert')).toHaveTextContent(words)
  })

  it('offers to sign in again, not to retry, when the session has ended', async () => {
    await renderDecks({ override: { list: () => deckError(401, 'unauthenticated', 'sign in') } })
    const alert = await screen.findByRole('alert')
    expect(alert).toHaveTextContent('Your session has ended')
    expect(within(alert).getByRole('link', { name: 'Sign in' }).getAttribute('href')).toMatch(
      /^\/login\?next=(%2F|\/)decks$/,
    )
    expect(within(alert).queryByRole('button', { name: 'Retry' })).toBeNull()
  })
})

describe('the export menu', () => {
  it('offers the three file kinds as downloads of the endpoint, named after the deck', async () => {
    const { user } = await renderDecks({
      decks: [deckItem({ id: 'a b', name: 'Atraxa' }), deckItem({ id: 'c', name: 'Burn' })],
    })
    await screen.findByRole('list', { name: 'Your decks' })
    await user.click(screen.getByRole('button', { name: 'Export Atraxa' }))
    const menu = await screen.findByRole('menu')
    const items = within(menu).getAllByRole('menuitem')
    expect(items.map((item) => item.textContent)).toEqual(['Plain text', 'MTG Arena', 'MTG Online'])
    expect(items.map((item) => item.getAttribute('href'))).toEqual([
      '/api/decks/a%20b/export?format=text',
      '/api/decks/a%20b/export?format=arena',
      '/api/decks/a%20b/export?format=mtgo',
    ])
    for (const item of items) {
      expect(item.tagName).toBe('A')
      expect(item).toHaveAttribute('download')
    }
  })

  it('closes on Escape and gives the focus back to its button', async () => {
    const { user } = await renderDecks({ decks: [deckItem({ name: 'Atraxa' })] })
    await screen.findByRole('list', { name: 'Your decks' })
    const button = screen.getByRole('button', { name: 'Export Atraxa' })
    await user.click(button)
    await screen.findByRole('menu')
    await user.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('menu')).toBeNull())
    expect(button).toHaveFocus()
  })

  it('is reachable by keyboard and opens with Enter', async () => {
    const { user } = await renderDecks({ decks: [deckItem({ name: 'Atraxa' })] })
    await screen.findByRole('list', { name: 'Your decks' })
    const button = screen.getByRole('button', { name: 'Export Atraxa' })
    button.focus()
    await user.keyboard('{Enter}')
    expect(await screen.findByRole('menu')).toBeInTheDocument()
    expect(deckList()).toBeInTheDocument()
  })

  it('names each button after its deck, so a list of many is not a list of "Export" and "Delete"', async () => {
    await renderDecks({ decks: [deckItem({ id: 'a', name: 'Atraxa' }), deckItem({ id: 'b', name: 'Burn' })] })
    await screen.findByRole('list', { name: 'Your decks' })
    for (const name of ['Atraxa', 'Burn']) {
      expect(screen.getByRole('button', { name: `Export ${name}` })).toBeInTheDocument()
      expect(screen.getByRole('button', { name: `Delete ${name}` })).toBeInTheDocument()
    }
  })
})
