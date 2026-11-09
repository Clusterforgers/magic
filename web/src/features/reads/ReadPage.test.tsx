// Goal: the review screen lists a deck's cards (or every card, with its count and ranks) to read beside the card being read, loads the next card ahead, reopens where the reader was, highlights a clause the game words differently, gives no verdict for the first 2 s, takes Correct with C or the button and moves to the next card, needs a note for Wrong (W opens it) and then offers to play the card by hand in the deck, asks to load a draft that changed meanwhile, ends with every card read, and the table's ReadDialog shows the same sheet.
import { act, fireEvent, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { AuthContext } from '@/features/auth/context'
import { createAuthStore, offlineAccounts } from '@/features/auth/store'
import type { ByHandApi } from '@/features/decks/byHand'
import { deckView } from '@/features/decks/fakeDecks'
import type { ReadsApi } from '@/net/reads'
import { renderInRouter } from '@/test-utils/router'
import { ReadsApiContext } from './apiContext'
import { READ_CARDS, fixtureReadsApi } from './dev/fixture'
import type { ReadFixtureOptions } from './dev/fixture'
import { READ_DELAY_MS } from './model'
import { ReadDialog } from './ReadDialog'
import { ReadPage } from './ReadPage'

const [FIRST, SECOND, THIRD] = Object.values(READ_CARDS) as [
  (typeof READ_CARDS)[string],
  (typeof READ_CARDS)[string],
  (typeof READ_CARDS)[string],
]

function signedIn() {
  return createAuthStore({
    api: { ...offlineAccounts, myGames: async () => [] },
    initial: { status: 'signed-in', user: { id: 'ada', username: 'Ada_L', is_admin: false } },
  })
}

function spied(options: ReadFixtureOptions = {}) {
  const api = fixtureReadsApi(options)
  const judge = vi.fn(api.judge)
  return { api: { ...api, judge } satisfies ReadsApi, judge }
}

async function renderPage(options: ReadFixtureOptions = {}, decks?: ByHandApi) {
  const { api, judge } = spied(options)
  await renderInRouter(
    <AuthContext value={signedIn()}>
      <ReadPage target={{ deck: 'deck-izzet' }} api={api} {...(decks === undefined ? {} : { decks })} />
    </AuthContext>,
  )
  await screen.findByRole('heading', { level: 2, name: FIRST.name })
  return { judge }
}

const button = (name: RegExp) => screen.getByRole('button', { name })
// Effects first, so the delay's timer is set before the clock moves on.
const pass = async (ms: number) => {
  await act(async () => {})
  await act(async () => {
    vi.advanceTimersByTime(ms)
  })
}

beforeEach(() => {
  vi.useFakeTimers({ shouldAdvanceTime: true })
})

afterEach(() => {
  vi.useRealTimers()
  window.localStorage.clear()
})

describe('ReadPage', () => {
  it('lists the cards and shows the first with its differing clause highlighted', async () => {
    await renderPage()
    const list = screen.getByRole('navigation', { name: 'Cards to read' })
    expect(
      within(list)
        .getAllByRole('button')
        .map((item) => item.textContent),
    ).toEqual([
      expect.stringContaining(FIRST.name),
      expect.stringContaining(SECOND.name),
      expect.stringContaining(THIRD.name),
    ])
    expect(within(list).getByRole('button', { current: true })).toHaveTextContent(FIRST.name)
    const rows = screen.getByRole('list', { name: "Clauses beside the game's reading" })
    const tones = [...rows.querySelectorAll(':scope > li')].map((row) => row.getAttribute('data-tone'))
    expect(tones).toEqual(['written', 'differs', 'unwritten'])
    expect(screen.getByText('1 clause is worded differently by the game. Compare it closely.')).toBeVisible()
  })

  it('gives no verdict for the first two seconds, by button or key', async () => {
    const { judge } = await renderPage()
    expect(button(/^Correct/)).toHaveAttribute('aria-disabled', 'true')
    fireEvent.click(button(/^Correct/))
    fireEvent.keyDown(document.body, { key: 'c' })
    expect(judge).not.toHaveBeenCalled()
    await pass(READ_DELAY_MS)
    expect(button(/^Correct/)).toHaveAttribute('aria-disabled', 'false')
  })

  it('takes Correct with C and moves on to the next card', async () => {
    const { judge } = await renderPage()
    await pass(READ_DELAY_MS)
    fireEvent.keyDown(document.body, { key: 'c' })
    await screen.findByRole('heading', { level: 2, name: SECOND.name })
    expect(judge).toHaveBeenCalledWith(FIRST.oracle_id, {
      read_hash: FIRST.read_hash,
      verdict: 'correct',
      note: null,
    })
    const list = screen.getByRole('navigation', { name: 'Cards to read' })
    expect(within(list).getAllByRole('button')[0]).toHaveTextContent('You said correct')
    expect(screen.getByText(/saved\. Next: Ember Archive\./)).toBeInTheDocument()
    // The next card waits its own two seconds.
    expect(button(/^Correct/)).toHaveAttribute('aria-disabled', 'true')
  })

  it('needs a note for Wrong, which W opens', async () => {
    const { judge } = await renderPage()
    await pass(READ_DELAY_MS)
    fireEvent.keyDown(document.body, { key: 'w' })
    const note = screen.getByRole('textbox', { name: 'What do the rules do wrong?' })
    await waitFor(() => expect(note).toHaveFocus())
    // The key typed into the note is no verdict.
    fireEvent.keyDown(note, { key: 'c' })
    fireEvent.click(button(/^Send: wrong/))
    expect(screen.getByText('Say what the rules do wrong.')).toBeVisible()
    expect(note).toHaveAttribute('aria-invalid', 'true')
    expect(judge).not.toHaveBeenCalled()
    fireEvent.change(note, { target: { value: '  it never discards  ' } })
    fireEvent.click(button(/^Send: wrong/))
    await screen.findByRole('heading', { level: 2, name: SECOND.name })
    expect(judge).toHaveBeenCalledWith(FIRST.oracle_id, {
      read_hash: FIRST.read_hash,
      verdict: 'wrong',
      note: 'it never discards',
    })
  })

  it('offers to play a card found wrong by hand in the deck being read, and nothing after Correct', async () => {
    const decks = {
      get: vi.fn(async (id: string) => deckView({ id })),
      update: vi.fn(async (id: string) => deckView({ id })),
    }
    await renderPage({}, decks)
    await pass(READ_DELAY_MS)
    fireEvent.keyDown(document.body, { key: 'w' })
    fireEvent.change(screen.getByRole('textbox', { name: 'What do the rules do wrong?' }), {
      target: { value: 'it never discards' },
    })
    fireEvent.click(button(/^Send: wrong/))
    await screen.findByRole('heading', { level: 2, name: SECOND.name })
    const offer = screen.getByText(`${FIRST.name} is found wrong`).closest('[data-slot="wrong-offer"]')
    expect(offer).not.toBeNull()
    fireEvent.click(
      within(offer as HTMLElement).getByRole('button', {
        name: `Play ${FIRST.name} by hand in Izzet Spellcraft`,
      }),
    )
    expect(await within(offer as HTMLElement).findByText('Played by hand in Izzet Spellcraft')).toBeVisible()
    expect(decks.update).toHaveBeenCalledWith('deck-izzet', expect.anything())

    await pass(READ_DELAY_MS)
    fireEvent.keyDown(document.body, { key: 'c' })
    await screen.findByRole('heading', { level: 2, name: THIRD.name })
    expect(screen.queryByText(/is found wrong$/)).toBeNull()
  })

  it('asks to load a draft that changed since the page loaded it', async () => {
    await renderPage({ stale: true })
    await pass(READ_DELAY_MS)
    fireEvent.click(button(/^Correct/))
    const alert = await screen.findByRole('alert')
    expect(alert).toHaveTextContent('The rules changed')
    fireEvent.click(within(alert).getByRole('button', { name: 'Load the new version' }))
    await screen.findByRole('heading', { level: 2, name: FIRST.name })
    expect(screen.queryByRole('alert')).toBeNull()
  })

  it('ends with every card read', async () => {
    await renderPage()
    for (const next of [SECOND.name, THIRD.name]) {
      await pass(READ_DELAY_MS)
      fireEvent.keyDown(document.body, { key: 'c' })
      await screen.findByRole('heading', { level: 2, name: next })
    }
    await pass(READ_DELAY_MS)
    fireEvent.keyDown(document.body, { key: 'c' })
    expect(await screen.findByRole('heading', { name: 'Every card is read' })).toBeVisible()
  })

  it('reads every card with no deck in the address, with the count and the ranks', async () => {
    await renderInRouter(
      <AuthContext value={signedIn()}>
        <ReadPage target={null} api={fixtureReadsApi()} />
      </AuthContext>,
    )
    expect(await screen.findByRole('heading', { level: 1, name: 'Read every card' })).toBeVisible()
    expect(document.querySelector('[data-slot="read-count"]')).toHaveTextContent(
      '3 to read, 2 of them in your decks.',
    )
    const list = screen.getByRole('navigation', { name: 'Cards to read' })
    expect(within(list).getAllByRole('button')[0]).toHaveTextContent('#14')
    expect(within(list).getAllByRole('button')[1]).toHaveTextContent('#1,204')
    expect(within(list).getAllByRole('button')[0]).toHaveTextContent('in your decks')
  })

  it('reads the cards of the decks under its own title, without saying each is in a deck', async () => {
    await renderInRouter(
      <AuthContext value={signedIn()}>
        <ReadPage target={{ mine: true }} api={fixtureReadsApi()} />
      </AuthContext>,
    )
    expect(await screen.findByRole('heading', { level: 1, name: 'Cards in your decks' })).toBeVisible()
    expect(document.title).toContain('Cards in your decks')
    expect(document.querySelector('[data-slot="read-count"]')).toHaveTextContent(/^2 to read\.$/)
    const list = screen.getByRole('navigation', { name: 'Cards to read' })
    expect(within(list).getAllByRole('button')).toHaveLength(2)
    expect(list).not.toHaveTextContent('in your decks')
    expect(screen.getByRole('link', { name: 'Cards to check' })).toHaveAttribute('href', '/decks/unvalidated')
  })

  it('loads the next card ahead, so a verdict opens it at once, and counts down', async () => {
    const api = fixtureReadsApi()
    const get = vi.fn(api.get)
    await renderInRouter(
      <AuthContext value={signedIn()}>
        <ReadPage target={null} api={{ ...api, get }} />
      </AuthContext>,
    )
    await screen.findByRole('heading', { level: 2, name: FIRST.name })
    await waitFor(() => expect(get).toHaveBeenCalledWith(SECOND.oracle_id))
    await pass(READ_DELAY_MS)
    fireEvent.keyDown(document.body, { key: 'c' })
    await screen.findByRole('heading', { level: 2, name: SECOND.name })
    expect(get.mock.calls.filter(([id]) => id === SECOND.oracle_id)).toHaveLength(1)
    expect(document.querySelector('[data-slot="read-count"]')).toHaveTextContent(
      '2 to read, 1 of them in your decks.',
    )
  })

  it('opens the queue again at the card the reader was on', async () => {
    window.localStorage.setItem('mtg.read-position:deck:deck-izzet', SECOND.oracle_id)
    try {
      const { api } = spied()
      await renderInRouter(
        <AuthContext value={signedIn()}>
          <ReadPage target={{ deck: 'deck-izzet' }} api={api} />
        </AuthContext>,
      )
      await screen.findByRole('heading', { level: 2, name: SECOND.name })
      const list = screen.getByRole('navigation', { name: 'Cards to read' })
      fireEvent.click(within(list).getByRole('button', { name: new RegExp(THIRD.name) }))
      await screen.findByRole('heading', { level: 2, name: THIRD.name })
      expect(window.localStorage.getItem('mtg.read-position:deck:deck-izzet')).toBe(THIRD.oracle_id)
    } finally {
      window.localStorage.clear()
    }
  })

  it('shows a failed list with Try again', async () => {
    await renderInRouter(
      <AuthContext value={signedIn()}>
        <ReadPage target={{ precon: 'c18' }} api={fixtureReadsApi({ listFails: 'network' })} />
      </AuthContext>,
    )
    expect(await screen.findByText('The cards could not be loaded')).toBeVisible()
    expect(screen.getByRole('button', { name: 'Try again' })).toBeVisible()
  })
})

describe('ReadDialog', () => {
  it('shows the sheet of one card from the context and closes', async () => {
    const onClose = vi.fn()
    await renderInRouter(
      <ReadsApiContext value={fixtureReadsApi()}>
        <ReadDialog oracleId={FIRST.oracle_id} name={FIRST.name} onClose={onClose} />
      </ReadsApiContext>,
    )
    const dialog = await screen.findByRole('dialog', { name: `Read the rules of ${FIRST.name}` })
    expect(
      await within(dialog).findByRole('list', { name: "Clauses beside the game's reading" }),
    ).toBeVisible()
    fireEvent.click(within(dialog).getByRole('button', { name: 'Close' }))
    expect(onClose).toHaveBeenCalled()
  })
})
