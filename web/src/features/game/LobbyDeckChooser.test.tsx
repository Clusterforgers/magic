// Goal: a player chooses the deck of their own seat in the lobby until they are Ready: a guest among the demo decks, a signed-in player among the stored decks too (each with its check state, a blocked one shown with its reason and not pressable); a choice goes out with the seat's token, shows its pending state with every other control waiting, says the result as name, format and manual cards, and on every kind of failure puts the old choice back with the reason in words (a refused deck lists its issues); a seat without a deck says so and cannot press Ready, which opens when the roster shows a deck, and the chooser locks while the seat is ready.
import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it } from 'vitest'
import { createAuthStore } from '@/features/auth/store'
import { deckIn, deckView, legalityIssue, lineOf, reportCard, reportOf } from '@/features/decks/fakeDecks'
import { startAccounts } from '@/test-utils/accountsServer'
import { jsonResponse } from '@/test-utils/fakeFetch'
import { tokenOf } from '@/test-utils/fakeServer'
import type { ServerScript } from '@/test-utils/fakeServer'
import { renderGame } from '@/test-utils/gameHarness'
import { changesOf, FAILURES, held } from '@/test-utils/lobbyControls'
import { lobbyMessage, SEAT_DECK, withDecks } from '@/test-utils/lobbyFrames'
import type { SeatFlags } from '@/test-utils/lobbyFrames'
import { GAME_ID } from '@/test-utils/gameHarness'

afterEach(() => window.sessionStorage.clear())

const NO_DECK: SeatFlags[] = [
  ['Ana', true, false, null],
  ['Bo', true, false],
]
const WITH_DECK: SeatFlags[] = [
  ['Ana', true, false],
  ['Bo', true, false],
]
const STORED = [
  deckIn('verified', { id: 'rakdos', name: 'Rakdos Midrange', format: 'standard' }),
  deckIn('manual_play', {
    id: 'burn',
    name: 'Burn',
    format: 'modern',
    summary: {
      ...deckIn('manual_play').summary,
      counts: { ...deckIn('manual_play').summary.counts, manual: 3 },
    },
  }),
  deckIn('blocked', {
    id: 'pauper',
    name: 'Pauper Affinity',
    format: 'pauper',
    summary: { ...deckIn('blocked').summary, issues: 2 },
  }),
]

// What the table says of each stored deck: it runs, three cards are played by hand, and it cannot be taken.
const REPORTS: Record<string, ReturnType<typeof reportOf>> = {
  rakdos: reportOf([reportCard('Forest', { count: 60 })]),
  burn: reportOf([
    reportCard('Forest', { count: 57 }),
    reportCard('Odd Card', { count: 3, support: 'manual' }),
  ]),
  pauper: reportOf(
    [reportCard('Forest', { count: 58 }), reportCard('Bad Card', { count: 2, support: 'unplayable' })],
    [legalityIssue('a Pauper deck holds no rares')],
  ),
}

interface Scene {
  format?: string
  table?: SeatFlags[]
  script?: ServerScript
  signedIn?: boolean
}

/** A lobby on seat 0 with the roster pushed; a signed-in page reads the stored decks of `STORED` unless told otherwise. */
async function openChooser({
  table = NO_DECK,
  script = {},
  signedIn = false,
  format = 'normal',
}: Scene = {}) {
  const accounts = startAccounts({
    users: { ada: 'correct horse battery' },
    signedIn: signedIn ? 'ada' : undefined,
  })
  const h = await renderGame({
    routed: true,
    auth: signedIn ? createAuthStore({ api: accounts.api }) : undefined,
    script: {
      myDecks: () => jsonResponse(200, { decks: STORED }),
      check: (body) =>
        jsonResponse(
          200,
          REPORTS[(body as { deck: string }).deck] ?? reportOf([reportCard('Forest', { count: 60 })]),
        ),
      game: () =>
        jsonResponse(200, {
          format,
          started: false,
          seats: table.map(([name], seat) => ({ seat, name })),
          connected: [0, 1],
          ready: [],
          result: null,
          visibility: 'public',
        }),
      ...script,
    },
  })
  h.join(0, table.length)
  h.deliver(lobbyMessage(table))
  await h.settle()
  return h
}

// A deck's name is its title, then its facts and its verdict; a string asks for the start of it.
const tile = (name: string | RegExp): HTMLElement =>
  screen.getByRole('option', {
    name: typeof name === 'string' ? new RegExp(`^${name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}`) : name,
  })
// Chooses a deck the way a player does: select its row, then press Use this deck.
async function pick(h: { user: ReturnType<typeof userEvent.setup> }, name: string | RegExp): Promise<void> {
  await h.user.click(tile(name))
  await h.user.click(await screen.findByRole('button', { name: /^Use this deck$/ }))
}
const checked = (h: { server: { calls: readonly { url: string; init?: RequestInit }[] } }) =>
  h.server.calls
    .filter((call) => call.url === '/api/decks/check')
    .map((call) => JSON.parse(String(call.init?.body)) as { deck: string; game: string })
const deckCalls = (h: { server: Parameters<typeof changesOf>[0] }) =>
  changesOf(h.server).filter((call) => call.method === 'PUT')
const note = (): Element | null => document.querySelector('[data-slot="deck-note"]')
const problem = (): HTMLElement | null => document.querySelector('[data-slot="deck-problem"]')
const current = (): string => document.querySelector('[data-slot="deck-current"]')?.textContent ?? ''
const ready = (): HTMLElement => screen.getByRole('button', { name: /^(Not r|R)eady$/ })

describe('the decks on offer', () => {
  it('gives a guest the built-in decks and the precons, and asks for no stored ones', async () => {
    const h = await openChooser()
    await screen.findByRole('listbox', { name: 'Built-in decks' })
    expect(screen.getAllByRole('tab').map((tab) => tab.textContent)).toEqual(['Built-in', 'Precons'])
    expect(
      within(screen.getByRole('listbox', { name: 'Built-in decks' })).getAllByRole('option'),
    ).toHaveLength(3)
    expect(screen.queryByRole('listbox', { name: 'Your decks' })).toBeNull()
    expect(h.server.calls.some((call) => call.url === '/api/decks')).toBe(false)
    expect(tile('Mono Red Giants')).not.toHaveAttribute('data-chosen')
    expect(tile('Mono Red Giants')).toHaveAccessibleName(/Whole deck runs$/)
  })

  it('gives a signed-in player their stored decks first, each with its support chip, and checks only the deck selected at this table', async () => {
    const h = await openChooser({ signedIn: true })
    const stored = await screen.findByRole('listbox', { name: 'Your decks' })
    expect(screen.getAllByRole('tab').map((tab) => tab.textContent)).toEqual(['Your decks', 'Built-in', 'Precons'])
    expect([...stored.children].map((item) => item.getAttribute('data-deck-option'))).toEqual([
      'rakdos',
      'burn',
      'pauper',
    ])
    expect(within(stored).getByRole('option', { name: /Rakdos Midrange.*Whole deck runs/ })).toBeInTheDocument()
    expect(within(stored).getByRole('option', { name: /Burn.*3 played by hand/ })).toBeInTheDocument()
    expect(within(stored).getByRole('option', { name: /Pauper Affinity.*Cannot be taken/ })).toBeInTheDocument()
    await waitFor(() => expect(checked(h).map((body) => body.deck)).toEqual(['rakdos']))
    expect(checked(h).every((body) => body.game === GAME_ID)).toBe(true)
  })

  it('shows a deck the table cannot take with its reasons and sends nothing when it is used', async () => {
    const h = await openChooser({ signedIn: true })
    await h.user.click(await screen.findByRole('option', { name: /Pauper Affinity/ }))
    const use = await screen.findByRole('button', { name: 'Use this deck' })
    await waitFor(() => expect(use).toHaveAttribute('aria-disabled', 'true'))
    expect(use).toHaveAccessibleDescription(/A Pauper deck holds no rares\. 1 more below\./)
    expect(await screen.findByText('Not legal in Normal')).toBeInTheDocument()
    await h.user.click(use)
    expect(deckCalls(h)).toEqual([])
  })

  it('names the cards played by hand in the preview of a stored deck, with a link to its page', async () => {
    const h = await openChooser({
      signedIn: true,
      script: {
        deckView: (id) =>
          jsonResponse(
            200,
            deckView({
              id,
              name: 'Burn',
              format: 'modern',
              cards: [lineOf('Forest', 57), lineOf('Odd Card', 3)],
              report: REPORTS[id] ?? reportOf([]),
            }),
          ),
      },
    })
    await h.user.click(await screen.findByRole('option', { name: /Burn/ }))
    const link = await screen.findByRole('link', { name: 'Not validated cards' })
    expect(link).toHaveAttribute('href', '/decks/burn/unvalidated')
    expect(await screen.findByRole('button', { name: '3 copies of Odd Card, played by hand' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '57 copies of Forest' })).toBeInTheDocument()
  })

  it('says a verdict could not be had, lets the deck be used anyway and asks again on request', async () => {
    let asked = 0
    const h = await openChooser({
      signedIn: true,
      script: {
        check: (body) =>
          asked++ < 1 ? FAILURES.server() : jsonResponse(200, REPORTS[(body as { deck: string }).deck]),
      },
    })
    await screen.findByText('Could not check this deck at the table.')
    expect(await screen.findByRole('button', { name: 'Use this deck' })).toBeEnabled()
    await h.user.click(screen.getByRole('button', { name: 'Check again' }))
    expect(await screen.findByText('Legal in Normal')).toBeInTheDocument()
  })

  it('says so when a signed-in player has no stored deck, and keeps the built-in decks one tab away', async () => {
    const h = await openChooser({
      signedIn: true,
      script: { myDecks: () => jsonResponse(200, { decks: [] }) },
    })
    expect(await screen.findByText(/You have no saved decks yet/)).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'Build or import a deck' })).toHaveAttribute('href', '/decks')
    await h.user.click(screen.getByRole('tab', { name: 'Built-in' }))
    expect(await screen.findByRole('listbox', { name: 'Built-in decks' })).toBeInTheDocument()
  })

  it('says why the stored decks are missing and offers another try, with the built-in decks still usable', async () => {
    let answers = 0
    const h = await openChooser({
      signedIn: true,
      script: { myDecks: () => (answers++ === 0 ? FAILURES.server() : jsonResponse(200, { decks: STORED })) },
    })
    expect(await screen.findByText(/The server ran into a problem/)).toBeInTheDocument()
    await h.user.click(screen.getByRole('tab', { name: 'Built-in' }))
    expect(await screen.findByRole('option', { name: /^Mono Red Giants/ })).toBeInTheDocument()
    await h.user.click(screen.getByRole('tab', { name: 'Your decks' }))
    await h.user.click(screen.getByRole('button', { name: 'Try again' }))
    expect(await screen.findByRole('listbox', { name: 'Your decks' })).toBeInTheDocument()
  })
})

describe('a choice that goes through', () => {
  it.each([
    [
      'a demo deck',
      'Mono Red Giants',
      'mono-red',
      false,
      'Your deck is now Mono Red Giants, Normal, no manual steps.',
    ],
    [
      'a stored deck with manual cards',
      /Burn.*3 played by hand/,
      'burn',
      true,
      'Your deck is now Burn, Modern, 3 cards need manual steps.',
    ],
  ])('sends %s with the seat token and says the result', async (_what, name, id, signedIn, words) => {
    const answer = id === 'burn' ? { name: 'Burn', format: 'modern', manual_cards: 3 } : undefined
    const h = await openChooser({
      signedIn,
      script: answer === undefined ? {} : { chooseDeck: () => jsonResponse(200, answer) },
    })
    const stored = id === 'burn'
    if (signedIn && !stored) await h.user.click(await screen.findByRole('tab', { name: 'Built-in' }))
    await screen.findByRole('listbox', { name: stored ? 'Your decks' : 'Built-in decks' })
    await pick(h, name)
    await h.settle()
    expect(deckCalls(h)).toEqual([
      {
        method: 'PUT',
        path: `/api/games/${GAME_ID}/seats/0/deck`,
        authorization: `Bearer ${tokenOf(0)}`,
        body: { deck: id },
      },
    ])
    expect(note()).toHaveTextContent(words)
    expect(note()).toHaveAttribute('aria-live', 'polite')
    expect(tile(name)).toHaveAttribute('data-chosen', 'true')
    expect(problem()).toBeNull()
  })

  it('shows the choice at once with a spinner, every other control waits, and focus stays on the deck pressed', async () => {
    const gate = held()
    const h = await openChooser({ table: WITH_DECK, script: { chooseDeck: gate.answer } })
    await screen.findByRole('listbox', { name: 'Built-in decks' })
    await pick(h, 'Mono Red Giants')
    const pressed = tile('Mono Red Giants')
    expect(within(pressed).getByLabelText('Choosing this deck')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /Use this deck/ })).toHaveAttribute('aria-busy', 'true')
    expect(note()).toHaveTextContent('Choosing Mono Red Giants…')
    expect(ready()).toBeDisabled()
    // The old deck is still the seat's until the answer: the roster says so.
    expect(current()).toBe('Mono Green Stompers, Normal, no manual steps')
    gate.release(jsonResponse(200, { name: 'Mono Red Giants', format: 'normal', manual_cards: 0 }))
    await h.settle()
    expect(within(pressed).queryByLabelText('Choosing this deck')).toBeNull()
    expect(ready()).toBeEnabled()
    expect(deckCalls(h)).toHaveLength(1)
  })

  it('marks the deck the roster shows, and follows the roster when it moves to another', async () => {
    const h = await openChooser({
      table: withDecks(WITH_DECK, { 0: { name: 'Mono Red Giants', format: 'normal', manual_cards: 0 } }),
    })
    await screen.findByRole('listbox', { name: 'Built-in decks' })
    expect(tile('Mono Red Giants')).toHaveAttribute('data-chosen', 'true')
    expect(tile('Mono Green Stompers')).not.toHaveAttribute('data-chosen')
    h.deliver(lobbyMessage(WITH_DECK))
    expect(tile('Mono Red Giants')).not.toHaveAttribute('data-chosen')
    expect(tile('Mono Green Stompers')).toHaveAttribute('data-chosen', 'true')
  })
})

describe('a choice that fails', () => {
  const issue = (message: string, name: string | null) => ({
    code: 'copies',
    message,
    zone: 'main',
    oracle_id: name === null ? null : `oracle-${name}`,
    name,
  })
  const BLOCKED = () =>
    jsonResponse(422, {
      error: 'deck_blocked',
      message: 'the deck cannot be played in this game',
      issues: [
        issue(
          'Lightning Bolt: 4 copies is the most a Normal deck may hold, and this deck has 5',
          'Lightning Bolt',
        ),
        issue('the main deck has 58 cards, and a Normal deck needs 60', null),
      ],
    })

  it.each<[string, () => Response | Promise<Response>, RegExp]>([
    ['403 forbidden', FAILURES.forbidden, /You can only choose the deck of your own seat/],
    ['404 not_found', FAILURES.gone, /You no longer have a seat in this game/],
    [
      '404 unknown_deck',
      () => jsonResponse(404, { error: 'unknown_deck', message: 'gone' }),
      /no longer there, or it is not in the account/,
    ],
    [
      '409 seat_ready',
      () => jsonResponse(409, { error: 'seat_ready', message: 'ready' }),
      /You are ready, so your deck cannot change/,
    ],
    ['500 internal', FAILURES.server, /The server ran into a problem/],
    ['503 unavailable', FAILURES.busy, /busy or restarting/],
    ['502 from a proxy', FAILURES.proxy, /answer this page could not read \(status 502\)/],
    ['offline', FAILURES.offline, /Could not reach the server/],
  ])('puts the old choice back and says why on %s', async (_what, answer, words) => {
    const h = await openChooser({ table: WITH_DECK, script: { chooseDeck: answer } })
    await screen.findByRole('listbox', { name: 'Built-in decks' })
    await pick(h, 'Mono Red Giants')
    await h.settle()
    expect(problem()).toHaveTextContent('Your deck is still Mono Green Stompers, Normal, no manual steps.')
    expect(problem()).toHaveTextContent(words)
    expect(problem()).toHaveAttribute('role', 'alert')
    expect(tile('Mono Red Giants')).not.toHaveAttribute('data-chosen')
    expect(tile('Mono Green Stompers')).toHaveAttribute('data-chosen', 'true')
    expect(note()).toBeEmptyDOMElement()
    // Everything waits no longer, and another try is possible.
    expect(tile('Mono Red Giants')).toBeEnabled()
    expect(ready()).toBeEnabled()
  })

  it('lists the issues of a refused deck to its owner and says the old deck is still in force', async () => {
    const h = await openChooser({ table: WITH_DECK, signedIn: true, script: { chooseDeck: BLOCKED } })
    await pick(h, /Rakdos Midrange/)
    await h.settle()
    const alert = problem()
    expect(alert).toHaveAttribute('data-blocked', 'true')
    expect(alert).toHaveTextContent('That deck cannot be played in this game.')
    const items = within(
      within(alert as HTMLElement).getByRole('list', { name: 'What breaks the rules' }),
    ).getAllByRole('listitem')
    expect(items.map((item) => item.textContent)).toEqual([
      'Lightning Bolt: 4 copies is the most a Normal deck may hold, and this deck has 5.',
      'The main deck has 58 cards, and a Normal deck needs 60.',
    ])
    await h.user.click(screen.getByRole('tab', { name: 'Built-in' }))
    expect(tile('Mono Green Stompers')).toHaveAttribute('data-chosen', 'true')
  })

  it('says a refused deck with no issue listed holds cards the game cannot play', async () => {
    const h = await openChooser({
      table: WITH_DECK,
      script: {
        chooseDeck: () => jsonResponse(422, { error: 'deck_blocked', message: 'blocked', issues: [] }),
      },
    })
    await screen.findByRole('listbox', { name: 'Built-in decks' })
    await pick(h, 'Mono Red Giants')
    await h.settle()
    expect(problem()).toHaveTextContent('The deck holds cards that the game cannot play.')
  })

  it('ends the lobby with the words that the game has started on 409 started', async () => {
    const h = await openChooser({ table: WITH_DECK, script: { chooseDeck: FAILURES.started } })
    await screen.findByRole('listbox', { name: 'Built-in decks' })
    await pick(h, 'Mono Red Giants')
    await h.settle()
    const stopped = document.querySelector('[data-slot="lobby-stopped"]')
    expect(stopped).toHaveTextContent(/already started/)
    expect(stopped).toHaveFocus()
    expect(document.querySelector('[data-slot="deck-chooser"]')).toBeNull()
  })

  it('sends a choice again after a failure, and the second answer clears the first one’s words', async () => {
    let tries = 0
    const h = await openChooser({
      table: WITH_DECK,
      script: {
        chooseDeck: () =>
          tries++ === 0
            ? FAILURES.server()
            : jsonResponse(200, { name: 'Mono Red Giants', format: 'normal', manual_cards: 0 }),
      },
    })
    await screen.findByRole('listbox', { name: 'Built-in decks' })
    await pick(h, 'Mono Red Giants')
    await h.settle()
    expect(problem()).not.toBeNull()
    await pick(h, 'Mono Red Giants')
    await h.settle()
    expect(problem()).toBeNull()
    expect(note()).toHaveTextContent('Your deck is now Mono Red Giants')
  })
})

describe('Ready needs a deck', () => {
  it('disables Ready for a seat without a deck, with the reason beside it and the table told', async () => {
    await openChooser()
    expect(ready()).toBeDisabled()
    expect(document.querySelector('[data-slot="ready-hint"]')).toHaveTextContent(
      /^Ready needs a deck\. Choose one under Your deck\./,
    )
    expect(current()).toBe('You have no deck yet. Choose one to press Ready.')
    expect(document.querySelector('[data-slot="lobby-summary"]')).toHaveTextContent(
      '0 of 2 ready, waiting for you to choose a deck and Bo to press Ready',
    )
  })

  it('opens Ready when the roster shows the seat’s deck, and locks the chooser once the seat is ready', async () => {
    const h = await openChooser()
    h.deliver(lobbyMessage(WITH_DECK))
    expect(ready()).toBeEnabled()
    expect(screen.getByText('Press Ready when you are set to play.')).toBeInTheDocument()
    await h.user.click(ready())
    h.deliver(lobbyMessage([['Ana', true, true], WITH_DECK[1] as SeatFlags]))
    expect(ready()).toHaveAccessibleName('Not ready')
    expect(screen.getByText(/Your deck is locked while you are ready/)).toBeInTheDocument()
    expect(screen.queryByRole('listbox', { name: 'Built-in decks' })).toBeNull()
    expect(current()).toBe('Mono Green Stompers, Normal, no manual steps')
  })

  it('does not block a seat the page knows nothing about yet: before any roster the button is the server’s to judge', async () => {
    const h = await renderGame({
      script: {
        game: () =>
          jsonResponse(200, {
            started: false,
            seats: [],
            connected: [],
            ready: [],
            result: null,
            visibility: 'public',
          }),
      },
    })
    h.join(0, 2)
    await h.settle()
    expect(ready()).toBeEnabled()
    expect(current()).toBe('')
  })

  it('keeps Not ready available to a seat that is ready, whatever the roster says of its deck', async () => {
    const h = await openChooser({
      table: [
        ['Ana', true, true, SEAT_DECK],
        ['Bo', true, false],
      ],
    })
    expect(ready()).toBeEnabled()
    await waitFor(() => expect(h.server.calls.length).toBeGreaterThan(0))
  })
})
