// Goal: the builder page loads a deck by its id and says when it cannot, keeps edits in the browser until Save sends the whole deck and shows the state the server answers, keeps edits made while a save is in flight, guards unsaved changes against leaving the page and against navigation inside the app, sends a visitor who is not signed in to the sign-in form and back, switches between Search and Deck under the M breakpoint without losing the search, and fits the window with the two panes scrolling in their own regions.
import { screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { deckError, deckView, lineOf } from '@/features/decks/fakeDecks'
import { decksApi } from '@/net/decks'
import { renderApp } from '@/test-utils/appHarness'
import { jsonResponse } from '@/test-utils/fakeFetch'
import {
  BOLT,
  copies,
  deckPane,
  DURESS,
  MOUNTAIN,
  renderBuilder,
  searchBox,
  searchPane,
} from './renderBuilder'

afterEach(() => {
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

const pane = () => within(deckPane())
// The marker beside the name; the support counts show a Verified mark of their own.
const marker = () => deckPane().querySelector('[data-marker]')
const markedAs = (state: string) => waitFor(() => expect(marker()).toHaveAttribute('data-marker', state))
const ADA = { username: 'Ada_L', password: 'correct horse battery' }
const users = { [ADA.username]: ADA.password }

function hold() {
  let release: (response: Response) => void = () => undefined
  const promise = new Promise<Response>((resolve) => {
    release = resolve
  })
  return { promise, release: (response: Response) => release(response) }
}

describe('loading the deck', () => {
  it('asks for the deck by its id with the session and shows its name, format and saved state', async () => {
    const { decks } = await renderBuilder()
    expect(decks.calls[0]?.url).toBe('/api/decks/deck-1')
    expect(decks.calls[0]?.init?.credentials).toBe('include')
    expect(pane().getByRole('textbox', { name: 'Deck name' })).toHaveValue('Burn')
    expect(pane().getByRole('combobox', { name: 'Format' })).toHaveValue('modern')
    expect(pane().getByText('Played by hand', { selector: '[data-deck-state]' })).toBeInTheDocument()
    expect(screen.getByRole('heading', { level: 1, name: 'Edit Burn' })).toBeInTheDocument()
    await waitFor(() => expect(document.title).toBe('Edit Burn · Magic'))
  })

  it('says it is loading while the deck is on its way', async () => {
    await renderBuilder({
      wait: false,
      decks: { override: { get: () => new Promise<Response>(() => undefined) } },
    })
    expect(screen.getByText('Loading the deck…')).toBeInTheDocument()
    expect(screen.queryByRole('region', { name: 'Deck' })).toBeNull()
  })

  it('says a deck is not found, with the way back, when the server has no such deck for this account', async () => {
    await renderBuilder({
      wait: false,
      decks: { override: { get: () => deckError(404, 'not_found', 'no such deck') } },
    })
    expect(
      await screen.findByRole('heading', { level: 1, name: 'That deck was not found' }),
    ).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'Back to your decks' })).toHaveAttribute('href', '/decks')
  })

  it('says a server without a database keeps no decks', async () => {
    await renderBuilder({ wait: false, decks: { unavailable: true } })
    expect(await screen.findByText('This server keeps no decks')).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'Create a game' })).toHaveAttribute('href', '/')
  })

  it('words a failed load and loads again on Retry', async () => {
    let attempts = 0
    const { user, view } = await renderBuilder({
      wait: false,
      decks: { override: { get: () => (++attempts === 1 ? deckError(500, 'internal', 'boom') : undefined) } },
    })
    const alert = await screen.findByRole('alert')
    expect(alert).toHaveTextContent('Could not load the deck')
    await user.click(screen.getByRole('button', { name: 'Retry' }))
    expect(await screen.findByRole('region', { name: 'Deck' })).toBeInTheDocument()
    expect(pane().getByRole('textbox', { name: 'Deck name' })).toHaveValue(view.name)
  })

  it('offers sign-in, back to this deck, when the session has ended', async () => {
    await renderBuilder({
      wait: false,
      decks: { override: { get: () => deckError(401, 'unauthenticated', 'no') } },
    })
    const link = await screen.findByRole('link', { name: 'Sign in' })
    expect(link.getAttribute('href')).toContain('/login')
    expect(decodeURIComponent(link.getAttribute('href') ?? '')).toContain('next=/decks/deck-1')
  })
})

describe('Save', () => {
  it('does nothing while nothing changed, and keeps the focus it was given', async () => {
    const { user, decks } = await renderBuilder()
    const save = pane().getByRole('button', { name: 'Save' })
    expect(save).toHaveAttribute('aria-disabled', 'true')
    await user.click(save)
    expect(decks.updated).toEqual([])
  })

  it('sends the whole deck with PUT, then shows the state the server answered and no longer says unsaved', async () => {
    const { user, decks } = await renderBuilder({
      view: {
        cards: [
          lineOf('Mountain', 20, { card: MOUNTAIN, printing_id: 'printing-1' }),
          lineOf('Lightning Bolt', 4, { card: BOLT }),
          lineOf('Duress', 2, { zone: 'sideboard', card: DURESS }),
        ],
      },
    })
    await user.click(screen.getByRole('button', { name: 'Add one Lightning Bolt' }))
    await user.type(pane().getByRole('textbox', { name: 'Deck name' }), '  ')
    expect(pane().getByText('Unsaved changes')).toBeInTheDocument()
    await user.click(pane().getByRole('button', { name: 'Save' }))
    await markedAs('verified')
    expect(decks.updated).toEqual([
      {
        id: 'deck-1',
        body: {
          name: 'Burn',
          format: 'modern',
          notes: '',
          cards: [
            { zone: 'main', oracle_id: MOUNTAIN.oracle_id, printing_id: 'printing-1', count: 20 },
            { zone: 'main', oracle_id: BOLT.oracle_id, printing_id: null, count: 5 },
            { zone: 'sideboard', oracle_id: DURESS.oracle_id, printing_id: null, count: 2 },
          ],
        },
      },
    ])
    expect(decks.calls.at(-1)?.init?.method).toBe('PUT')
    expect(decks.calls.at(-1)?.init?.credentials).toBe('include')
    expect(pane().queryByText('Unsaved changes')).toBeNull()
    expect(pane().getByRole('button', { name: 'Save' })).toHaveAttribute('aria-disabled', 'true')
    expect(copies('Lightning Bolt')).toBe(5)
    expect(screen.getByText('Saved “Burn”: Verified.')).toBeInTheDocument()
  })

  it.each([
    ['blocked', 'Blocked'],
    ['manual_play', 'Played by hand'],
    ['unchecked', 'Unchecked'],
  ] as const)('shows %s as the state a save came out with', async (state, word) => {
    const { user } = await renderBuilder({
      view: { state: 'verified', verified: true },
      decks: { saveState: state },
    })
    await user.click(screen.getByRole('button', { name: 'Add one Lightning Bolt' }))
    await user.click(pane().getByRole('button', { name: 'Save' }))
    expect(await pane().findByText(word, { selector: '[data-deck-state]' })).toBeInTheDocument()
    expect(screen.getByText(`Saved “Burn”: ${word}.`)).toBeInTheDocument()
  })

  it('shows it is busy and sends once when pressed twice while the first save is in flight', async () => {
    const gate = hold()
    const { user, decks } = await renderBuilder({ decks: { override: { update: () => gate.promise } } })
    await user.click(screen.getByRole('button', { name: 'Add one Lightning Bolt' }))
    const save = pane().getByRole('button', { name: 'Save' })
    await user.click(save)
    await user.click(save)
    expect(save).toHaveAttribute('aria-busy', 'true')
    expect(decks.updated).toHaveLength(1)
    gate.release(
      jsonResponse(
        200,
        deckView({
          id: 'deck-1',
          name: 'Burn',
          format: 'modern',
          cards: [lineOf('Lightning Bolt', 5, { card: BOLT })],
        }),
      ),
    )
    await waitFor(() => expect(save).not.toHaveAttribute('aria-busy'))
  })

  it('keeps edits made while the save was in flight as unsaved', async () => {
    const gate = hold()
    const { user } = await renderBuilder({ decks: { override: { update: () => gate.promise } } })
    await user.click(screen.getByRole('button', { name: 'Add one Lightning Bolt' }))
    await user.click(pane().getByRole('button', { name: 'Save' }))
    await user.click(screen.getByRole('button', { name: 'Add one Lightning Bolt' }))
    gate.release(
      jsonResponse(
        200,
        deckView({
          id: 'deck-1',
          name: 'Burn',
          format: 'modern',
          state: 'verified',
          verified: true,
          cards: [
            lineOf('Mountain', 20, { card: MOUNTAIN }),
            lineOf('Lightning Bolt', 5, { card: BOLT }),
            lineOf('Duress', 2, { zone: 'sideboard', card: DURESS }),
          ],
        }),
      ),
    )
    await waitFor(() => expect(pane().getByRole('button', { name: 'Save' })).not.toHaveAttribute('aria-busy'))
    expect(copies('Lightning Bolt')).toBe(6)
    expect(pane().getByText('Unsaved changes')).toBeInTheDocument()
    expect(marker()).not.toHaveAttribute('data-marker', 'verified')
  })

  it('keeps the edits and the reason when the server refuses, and saves on the next press', async () => {
    let attempts = 0
    const { user, decks } = await renderBuilder({
      decks: {
        override: {
          update: () =>
            ++attempts === 1 ? deckError(400, 'invalid_deck', 'a deck has at most 250 cards') : undefined,
        },
      },
    })
    await user.click(screen.getByRole('button', { name: 'Add one Lightning Bolt' }))
    await user.click(pane().getByRole('button', { name: 'Save' }))
    const alert = await pane().findByRole('alert')
    expect(alert).toHaveTextContent('The server did not accept the deck. A deck has at most 250 cards.')
    expect(pane().getByText('Unsaved changes')).toBeInTheDocument()
    expect(copies('Lightning Bolt')).toBe(5)
    await user.click(pane().getByRole('button', { name: 'Save' }))
    await markedAs('verified')
    expect(pane().queryByRole('alert')).toBeNull()
    expect(decks.updated).toHaveLength(2)
  })

  it('offers sign-in when the session ended during the edit', async () => {
    const { user } = await renderBuilder({
      decks: { override: { update: () => deckError(401, 'unauthenticated', 'no session') } },
    })
    await user.click(screen.getByRole('button', { name: 'Add one Lightning Bolt' }))
    await user.click(pane().getByRole('button', { name: 'Save' }))
    expect(await pane().findByText(/Your session has ended/)).toBeInTheDocument()
    expect(pane().getByRole('link', { name: 'Sign in' }).getAttribute('href')).toContain('/login')
  })

  it('refuses a blank name in words, puts the focus on it and sends nothing', async () => {
    const { user, decks } = await renderBuilder()
    const name = pane().getByRole('textbox', { name: 'Deck name' })
    await user.clear(name)
    await user.click(pane().getByRole('button', { name: 'Save' }))
    expect(pane().getByText('Enter a name for the deck.')).toBeInTheDocument()
    expect(name).toHaveAttribute('aria-invalid', 'true')
    expect(name).toHaveFocus()
    expect(decks.updated).toEqual([])
    await user.type(name, 'B')
    expect(pane().queryByText('Enter a name for the deck.')).toBeNull()
  })
})

describe('leaving the page', () => {
  const leave = (): Event => {
    const event = new Event('beforeunload', { cancelable: true })
    window.dispatchEvent(event)
    return event
  }

  it('asks the browser to confirm only while there are unsaved changes', async () => {
    const { user } = await renderBuilder()
    expect(leave().defaultPrevented).toBe(false)
    await user.click(screen.getByRole('button', { name: 'Add one Lightning Bolt' }))
    expect(leave().defaultPrevented).toBe(true)
    await user.click(pane().getByRole('button', { name: 'Save' }))
    await markedAs('verified')
    expect(leave().defaultPrevented).toBe(false)
  })

  it('stops listening when the page is left', async () => {
    const { user, unmount } = await renderBuilder()
    await user.click(screen.getByRole('button', { name: 'Add one Lightning Bolt' }))
    unmount()
    expect(leave().defaultPrevented).toBe(false)
  })
})

describe('navigating inside the app', () => {
  async function openInApp() {
    const view = deckView({
      id: 'deck-1',
      name: 'Burn',
      format: 'modern',
      cards: [lineOf('Lightning Bolt', 4, { card: BOLT })],
    })
    vi.spyOn(decksApi, 'get').mockResolvedValue(view)
    vi.spyOn(decksApi, 'list').mockResolvedValue([])
    const app = await renderApp('/decks/deck-1', { accounts: { users, signedIn: ADA.username } })
    await screen.findByRole('region', { name: 'Deck' })
    return app
  }
  const decksLink = () => screen.getByRole('link', { name: 'Decks' })

  it('is marked as the Decks page in the navigation', async () => {
    await openInApp()
    expect(decksLink()).toHaveAttribute('aria-current', 'page')
  })

  it('lets a deck without changes go at once', async () => {
    const app = await openInApp()
    await app.user.click(decksLink())
    await waitFor(() => expect(app.router.state.location.pathname).toBe('/decks'))
    expect(screen.queryByRole('alertdialog')).toBeNull()
  })

  it('asks before leaving a deck with unsaved changes, and Keep editing stays with the changes', async () => {
    const app = await openInApp()
    await app.user.click(screen.getByRole('button', { name: 'Add one Lightning Bolt' }))
    await app.user.click(decksLink())
    const dialog = await screen.findByRole('alertdialog', { name: 'Leave without saving?' })
    expect(dialog).toHaveTextContent('“Burn” has changes that are not saved.')
    await waitFor(() => expect(within(dialog).getByRole('button', { name: 'Keep editing' })).toHaveFocus())
    await app.user.click(within(dialog).getByRole('button', { name: 'Keep editing' }))
    await waitFor(() => expect(screen.queryByRole('alertdialog')).toBeNull())
    expect(app.router.state.location.pathname).toBe('/decks/deck-1')
    expect(copies('Lightning Bolt')).toBe(5)
  })

  it('takes Escape as Keep editing and Discard changes as leaving', async () => {
    const app = await openInApp()
    await app.user.click(screen.getByRole('button', { name: 'Add one Lightning Bolt' }))
    await app.user.click(decksLink())
    await screen.findByRole('alertdialog')
    await app.user.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('alertdialog')).toBeNull())
    expect(app.router.state.location.pathname).toBe('/decks/deck-1')
    await app.user.click(decksLink())
    await app.user.click(await screen.findByRole('button', { name: 'Discard changes' }))
    await waitFor(() => expect(app.router.state.location.pathname).toBe('/decks'))
  })
})

describe('a visitor who is not signed in', () => {
  it('is sent to the sign-in form, which remembers this deck, and no deck is asked for', async () => {
    const get = vi.spyOn(decksApi, 'get')
    const app = await renderApp('/decks/deck-1', { accounts: { users } })
    expect(await screen.findByRole('form', { name: 'Sign in' })).toBeInTheDocument()
    expect(app.router.state.location.pathname).toBe('/login')
    expect(app.router.state.matches.at(-1)?.search).toEqual({ next: '/decks/deck-1' })
    expect(get).not.toHaveBeenCalled()
  })

  it('finds a server without accounts has no decks to edit', async () => {
    const get = vi.spyOn(decksApi, 'get')
    await renderApp('/decks/deck-1', { accounts: { users, unavailable: true } })
    expect(await screen.findByText(/this server has no accounts, so it keeps no decks/i)).toBeInTheDocument()
    expect(get).not.toHaveBeenCalled()
  })
})

describe('under the M breakpoint', () => {
  const cls = (element: HTMLElement) => element.className.split(/\s+/)

  it('shows the search first and swaps panes with the switch, both staying in the page', async () => {
    const { user } = await renderBuilder()
    const group = screen.getByRole('group', { name: 'Show' })
    expect(group).toHaveClass('lg:hidden')
    const search = within(group).getByRole('button', { name: 'Search' })
    const deck = within(group).getByRole('button', { name: 'Deck 26' })
    expect(search).toHaveAttribute('aria-pressed', 'true')
    expect(deck).toHaveAttribute('aria-pressed', 'false')
    expect(cls(searchPane())).toEqual(expect.arrayContaining(['flex', 'lg:flex']))
    expect(cls(deckPane())).toEqual(expect.arrayContaining(['hidden', 'lg:flex']))
    await user.click(deck)
    expect(deck).toHaveAttribute('aria-pressed', 'true')
    expect(cls(deckPane())).toEqual(expect.arrayContaining(['flex', 'lg:flex']))
    expect(cls(searchPane())).toEqual(expect.arrayContaining(['hidden', 'lg:flex']))
    expect(cls(deckPane())).not.toContain('hidden')
  })

  it('keeps the search text and the hits when the deck is looked at and the search comes back', async () => {
    const { user } = await renderBuilder()
    await user.type(searchBox(), 'bolt')
    await screen.findByRole('list', { name: 'Search results' })
    await user.click(screen.getByRole('button', { name: /^Deck/ }))
    await user.click(screen.getByRole('button', { name: 'Search' }))
    expect(searchBox()).toHaveValue('bolt')
    expect(screen.getByRole('list', { name: 'Search results' })).toBeInTheDocument()
  })

  it('counts the deck on the switch as cards are added', async () => {
    const { user } = await renderBuilder()
    await user.click(screen.getByRole('button', { name: 'Add one Lightning Bolt' }))
    expect(screen.getByRole('button', { name: 'Deck 27' })).toBeInTheDocument()
  })
})

describe('the fit of the page', () => {
  const page = () => document.querySelector<HTMLElement>('[data-slot="page"]')
  const unprefixed = (element: Element | null) =>
    (element?.className ?? '')
      .toString()
      .split(/\s+/)
      .filter((name) => !name.includes(':'))

  it('is as tall as the window under the header from sm up, a column that gives the rest to the two panes', async () => {
    await renderBuilder()
    expect(page()).toHaveClass('sm:h-[calc(100dvh-3.5rem-1px)]', 'flex', 'flex-col')
    expect(page()).toHaveAttribute('data-width', 'wide')
    const panes = searchPane().parentElement
    expect(panes).toBe(deckPane().parentElement)
    expect(panes).toHaveClass(
      'min-h-0',
      'flex-1',
      'lg:grid-cols-[minmax(0,5fr)_minmax(0,7fr)]',
      'grid-rows-[minmax(0,1fr)]',
    )
    for (const element of [searchPane(), deckPane()]) expect(element).toHaveClass('min-h-0', 'flex-col')
  })

  it('scrolls the hits and the lines inside their own regions from sm up, and nothing else', async () => {
    await renderBuilder()
    const hits = searchPane().querySelector('.min-h-0.flex-1')
    const lines = deckPane().querySelector('.min-h-0.flex-1')
    for (const region of [hits, lines]) {
      expect(region).toHaveClass('sm:overflow-y-auto', 'sm:[scrollbar-gutter:stable]')
    }
    for (const element of [page(), searchPane().parentElement, hits, lines]) {
      expect(unprefixed(element).filter((name) => /^(h-|max-h-|overflow-)/.test(name))).toEqual([])
    }
  })
})
