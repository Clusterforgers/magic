// Goal: a player chooses the deck of their own seat in the lobby until they are Ready: a guest among the demo decks, a signed-in player among the stored decks too (each with its check state, a blocked one shown with its reason and not pressable); a choice goes out with the seat's token, shows its pending state with every other control waiting, says the result as name, format and manual cards, and on every kind of failure puts the old choice back with the reason in words (a refused deck lists its issues); a seat without a deck says so and cannot press Ready, which opens when the roster shows a deck, and the chooser locks while the seat is ready.
import { screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { createAuthStore } from '@/features/auth/store'
import { deckIn } from '@/features/decks/fakeDecks'
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

interface Scene {
  table?: SeatFlags[]
  script?: ServerScript
  signedIn?: boolean
}

/** A lobby on seat 0 with the roster pushed; a signed-in page reads the stored decks of `STORED` unless told otherwise. */
async function openChooser({ table = NO_DECK, script = {}, signedIn = false }: Scene = {}) {
  const accounts = startAccounts({
    users: { ada: 'correct horse battery' },
    signedIn: signedIn ? 'ada' : undefined,
  })
  const h = await renderGame({
    routed: true,
    auth: signedIn ? createAuthStore({ api: accounts.api }) : undefined,
    script: {
      myDecks: () => jsonResponse(200, { decks: STORED }),
      game: () =>
        jsonResponse(200, {
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

const tile = (name: string | RegExp): HTMLElement => screen.getByRole('button', { name })
const deckCalls = (h: { server: Parameters<typeof changesOf>[0] }) =>
  changesOf(h.server).filter((call) => call.method === 'PUT')
const note = (): Element | null => document.querySelector('[data-slot="deck-note"]')
const problem = (): HTMLElement | null => document.querySelector('[data-slot="deck-problem"]')
const current = (): string => document.querySelector('[data-slot="deck-current"]')?.textContent ?? ''
const ready = (): HTMLElement => screen.getByRole('button', { name: /^(Not r|R)eady$/ })

describe('the decks on offer', () => {
  it('gives a guest the demo decks and asks for no stored ones', async () => {
    const h = await openChooser()
    await screen.findByRole('list', { name: 'Demo decks' })
    expect(within(screen.getByRole('list', { name: 'Demo decks' })).getAllByRole('listitem')).toHaveLength(3)
    expect(screen.queryByRole('list', { name: 'Your decks' })).toBeNull()
    expect(h.server.calls.some((call) => call.url === '/api/decks')).toBe(false)
    expect(tile('Mono Red GiantsNormal, Red, 60 cards')).toHaveAttribute('aria-pressed', 'false')
  })

  it('gives a signed-in player their stored decks first, each with its check state in words', async () => {
    await openChooser({ signedIn: true })
    const stored = await screen.findByRole('list', { name: 'Your decks' })
    expect(
      within(stored)
        .getAllByRole('listitem')
        .map((item) => item.getAttribute('data-deck-option')),
    ).toEqual(['rakdos', 'burn', 'pauper'])
    expect(within(stored).getByRole('button', { name: /Rakdos Midrange.*Verified/ })).toBeEnabled()
    expect(within(stored).getByRole('button', { name: /Burn.*Manual play/ })).toBeEnabled()
    expect(within(stored).getByRole('button', { name: /Pauper Affinity.*Blocked/ })).toBeInTheDocument()
    expect(screen.getByRole('list', { name: 'Demo decks' })).toBeInTheDocument()
  })

  it('shows a blocked deck with its reason and sends nothing when it is pressed', async () => {
    const h = await openChooser({ signedIn: true })
    const blocked = await screen.findByRole('button', { name: /Pauper Affinity/ })
    expect(blocked).toHaveAttribute('aria-disabled', 'true')
    expect(blocked).toHaveAccessibleDescription(
      /Blocked: 2 legality issues\. Fix it in the deck builder to play it\./,
    )
    await h.user.click(blocked)
    expect(deckCalls(h)).toEqual([])
  })

  it('says so when a signed-in player has no stored deck, and keeps the demo decks', async () => {
    await openChooser({ signedIn: true, script: { myDecks: () => jsonResponse(200, { decks: [] }) } })
    expect(await screen.findByText(/You have no saved decks yet/)).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'Decks page' })).toHaveAttribute('href', '/decks')
    expect(screen.getByRole('list', { name: 'Demo decks' })).toBeInTheDocument()
  })

  it('says why the stored decks are missing and offers another try, with the demo decks still usable', async () => {
    let answers = 0
    await openChooser({
      signedIn: true,
      script: { myDecks: () => (answers++ === 0 ? FAILURES.server() : jsonResponse(200, { decks: STORED })) },
    })
    expect(await screen.findByText(/The server ran into a problem/)).toBeInTheDocument()
    expect(tile('Mono Red GiantsNormal, Red, 60 cards')).toBeEnabled()
    await screen.getByRole('button', { name: 'Try again' }).click()
    expect(await screen.findByRole('list', { name: 'Your decks' })).toBeInTheDocument()
  })
})

describe('a choice that goes through', () => {
  it.each([
    [
      'a demo deck',
      'Mono Red GiantsNormal, Red, 60 cards',
      'mono-red',
      false,
      'Your deck is now Mono Red Giants, Normal, no manual steps.',
    ],
    [
      'a stored deck with manual cards',
      /Burn.*Manual play/,
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
    await screen.findByRole('list', { name: 'Demo decks' })
    if (signedIn) await screen.findByRole('list', { name: 'Your decks' })
    await h.user.click(tile(name))
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
    expect(tile(name)).toHaveAttribute('aria-pressed', 'true')
    expect(problem()).toBeNull()
  })

  it('shows the choice at once with a spinner, every other control waits, and focus stays on the deck pressed', async () => {
    const gate = held()
    const h = await openChooser({ table: WITH_DECK, script: { chooseDeck: gate.answer } })
    await screen.findByRole('list', { name: 'Demo decks' })
    await h.user.click(tile('Mono Red GiantsNormal, Red, 60 cards'))
    const pressed = tile('Mono Red GiantsNormal, Red, 60 cards')
    expect(pressed).toHaveAttribute('aria-busy', 'true')
    expect(pressed).toHaveAttribute('aria-pressed', 'true')
    expect(pressed).toHaveFocus()
    expect(note()).toHaveTextContent('Choosing Mono Red Giants…')
    expect(tile('Mono White WeeniesNormal, White, 60 cards')).toBeDisabled()
    expect(ready()).toBeDisabled()
    // The old deck is still the seat's until the answer: the roster says so.
    expect(current()).toBe('Mono Green Stompers, Normal, no manual steps')
    gate.release(jsonResponse(200, { name: 'Mono Red Giants', format: 'normal', manual_cards: 0 }))
    await h.settle()
    expect(pressed).not.toHaveAttribute('aria-busy')
    expect(ready()).toBeEnabled()
    expect(deckCalls(h)).toHaveLength(1)
  })

  it('marks the deck the roster shows, and follows the roster when it moves to another', async () => {
    const h = await openChooser({
      table: withDecks(WITH_DECK, { 0: { name: 'Mono Red Giants', format: 'normal', manual_cards: 0 } }),
    })
    await screen.findByRole('list', { name: 'Demo decks' })
    expect(tile('Mono Red GiantsNormal, Red, 60 cards')).toHaveAttribute('aria-pressed', 'true')
    expect(tile('Mono Green StompersNormal, Green, 60 cards')).toHaveAttribute('aria-pressed', 'false')
    h.deliver(lobbyMessage(WITH_DECK))
    expect(tile('Mono Red GiantsNormal, Red, 60 cards')).toHaveAttribute('aria-pressed', 'false')
    expect(tile('Mono Green StompersNormal, Green, 60 cards')).toHaveAttribute('aria-pressed', 'true')
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
    await screen.findByRole('list', { name: 'Demo decks' })
    await h.user.click(tile('Mono Red GiantsNormal, Red, 60 cards'))
    await h.settle()
    expect(problem()).toHaveTextContent('Your deck is still Mono Green Stompers, Normal, no manual steps.')
    expect(problem()).toHaveTextContent(words)
    expect(problem()).toHaveAttribute('role', 'alert')
    expect(tile('Mono Red GiantsNormal, Red, 60 cards')).toHaveAttribute('aria-pressed', 'false')
    expect(tile('Mono Green StompersNormal, Green, 60 cards')).toHaveAttribute('aria-pressed', 'true')
    expect(note()).toBeEmptyDOMElement()
    // Everything waits no longer, and another try is possible.
    expect(tile('Mono Red GiantsNormal, Red, 60 cards')).toBeEnabled()
    expect(ready()).toBeEnabled()
  })

  it('lists the issues of a refused deck to its owner and says the old deck is still in force', async () => {
    const h = await openChooser({ table: WITH_DECK, signedIn: true, script: { chooseDeck: BLOCKED } })
    await h.user.click(await screen.findByRole('button', { name: /Rakdos Midrange/ }))
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
    expect(tile('Mono Green StompersNormal, Green, 60 cards')).toHaveAttribute('aria-pressed', 'true')
  })

  it('says a refused deck with no issue listed holds cards the game cannot play', async () => {
    const h = await openChooser({
      table: WITH_DECK,
      script: {
        chooseDeck: () => jsonResponse(422, { error: 'deck_blocked', message: 'blocked', issues: [] }),
      },
    })
    await screen.findByRole('list', { name: 'Demo decks' })
    await h.user.click(tile('Mono Red GiantsNormal, Red, 60 cards'))
    await h.settle()
    expect(problem()).toHaveTextContent('The deck holds cards that the game cannot play.')
  })

  it('ends the lobby with the words that the game has started on 409 started', async () => {
    const h = await openChooser({ table: WITH_DECK, script: { chooseDeck: FAILURES.started } })
    await screen.findByRole('list', { name: 'Demo decks' })
    await h.user.click(tile('Mono Red GiantsNormal, Red, 60 cards'))
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
    await screen.findByRole('list', { name: 'Demo decks' })
    await h.user.click(tile('Mono Red GiantsNormal, Red, 60 cards'))
    await h.settle()
    expect(problem()).not.toBeNull()
    await h.user.click(tile('Mono Red GiantsNormal, Red, 60 cards'))
    await h.settle()
    expect(problem()).toBeNull()
    expect(note()).toHaveTextContent('Your deck is now Mono Red Giants')
  })
})

describe('Ready needs a deck', () => {
  it('disables Ready for a seat without a deck, with the reason beside it and the table told', async () => {
    await openChooser()
    expect(ready()).toBeDisabled()
    expect(screen.getByText('Ready needs a deck. Choose one under Your deck.')).toBeInTheDocument()
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
    expect(screen.queryByRole('list', { name: 'Demo decks' })).toBeNull()
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
