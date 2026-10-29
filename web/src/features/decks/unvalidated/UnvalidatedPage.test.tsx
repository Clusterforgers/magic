// Goal: the not-validated page lists each card the game does not run whole once, the cards of most decks first, with its picture, state, decks and each unread clause quoted beside its reason; arranges them by card or by deck, narrows them to a state, copies them as text in the exact format, and says it plainly while loading, when empty, when a deck or the whole list could not be checked and when card texts could not be read.
import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { AuthContext } from '@/features/auth/context'
import { createAuthStore, offlineAccounts } from '@/features/auth/store'
import { cardsApi } from '@/net/cards'
import { decksApi } from '@/net/decks'
import type { FixMarksApi } from '@/net/fixMarks'
import { renderApp } from '@/test-utils/appHarness'
import { renderInRouter } from '@/test-utils/router'
import { fixMarksClient } from '../../fixmarks/client'
import { deckView } from '../fakeDecks'
import { FIXTURE_DECKS, fixtureCardsApi, fixtureDecksApi } from './fixture'
import type { FixtureOptions } from './fixture'
import { UnvalidatedPage } from './UnvalidatedPage'

async function renderPage({
  deckId,
  fixMarks,
  ...fixture
}: { deckId?: string; fixMarks?: FixMarksApi } & FixtureOptions = {}) {
  const decks = fixtureDecksApi(fixture)
  const cards = fixtureCardsApi(fixture)
  const auth = createAuthStore({
    api: { ...offlineAccounts, myGames: async () => [] },
    initial: { status: 'signed-in', user: { id: 'ada', username: 'Ada_L' } },
  })
  const view = await renderInRouter(
    <AuthContext value={auth}>
      <UnvalidatedPage
        {...(deckId === undefined ? {} : { deckId })}
        api={decks}
        cards={cards}
        fixMarks={fixMarks}
      />
    </AuthContext>,
  )
  return { ...view, decks, cards, user: userEvent.setup() }
}

const settled = () => screen.findByRole('list', { name: 'Cards not validated' })
const cardNames = (list: HTMLElement): string[] =>
  within(list)
    .getAllByRole('listitem')
    .filter((item) => item.hasAttribute('data-card'))
    .map((item) => item.querySelector('p')?.textContent ?? '')
const itemOf = (name: string): HTMLElement => {
  const found = [...document.querySelectorAll<HTMLElement>('li[data-card]')].find(
    (item) => item.querySelector('p')?.textContent === name,
  )
  if (found === undefined) throw new Error(`no card ${name}`)
  return found
}

const ATRAXA = 'Atraxa Superfriends'
const CHAOS = 'Chaos Brew With A Rather Long Name To Wrap'

const FULL_TEXT = [
  `Rhystic Study (Manual) - decks: ${ATRAXA}, Izzet Spells, ${CHAOS}`,
  '  - "Whenever an opponent casts a spell, you may draw a card unless that player pays {1}.": Played by hand',
  '',
  `Mana Drain (Partial) - decks: ${ATRAXA}, Izzet Spells`,
  '  - "At the beginning of your next main phase, add an amount of {C} equal to that spell’s mana value.": The delayed mana is not implemented',
  '',
  `Contract from Below (Unplayable) - decks: ${CHAOS}`,
  '  - "ante the top card of your library": Ante is not played in this game',
  '',
  'Delver of Secrets // Insectile Aberration (Partial) - decks: Izzet Spells',
  '  - "transform Delver of Secrets": Front face: transforming is not implemented',
  '  - "Flying": Back face: flying of a back face is not implemented',
  '',
  `Doubling Season (Partial) - decks: ${ATRAXA}`,
  '  - "If an effect would put one or more counters on a permanent you control, it puts twice that many of those counters on that permanent instead.": Replacement effects for counters are not implemented',
  '',
  `Mystic Remora (Stale) - decks: ${CHAOS}`,
  '  - "Cumulative upkeep {1} / Whenever an opponent casts a noncreature spell, you may draw a card unless that player pays {4}.": Its text changed since the rules were written, so it is played by hand',
].join('\n')

describe('the fix-mark button', () => {
  const api = () => ({
    list: vi.fn(async () => []),
    mark: vi.fn(async (oracleId: string) => ({
      oracle_id: oracleId,
      name: 'x',
      note: null,
      created_at: '2026-10-29T12:00:00Z',
    })),
    unmark: vi.fn(async () => undefined),
  })

  it('is on every card of the list, which are all cards the game does not run whole, when the endpoints are given', async () => {
    fixMarksClient.clear()
    const { user } = await renderPage({ fixMarks: api() })
    const list = await settled()
    const buttons = within(list).getAllByRole('button', { name: 'Mark important to fix' })
    expect(buttons).toHaveLength(cardNames(list).length)
    await waitFor(() => expect(buttons[0]).toHaveAttribute('aria-disabled', 'false'))
    await user.click(buttons[0] as HTMLElement)
    expect(buttons[0]).toHaveAttribute('aria-pressed', 'true')
  })

  it('is not drawn without them', async () => {
    await renderPage()
    expect(within(await settled()).queryByRole('button', { name: 'Mark important to fix' })).toBeNull()
  })
})

describe('every saved deck', () => {
  it('lists each card once, the cards of most decks first, then by name', async () => {
    await renderPage()
    const list = await settled()
    expect(cardNames(list)).toEqual([
      'Rhystic Study',
      'Mana Drain',
      'Contract from Below',
      'Delver of Secrets // Insectile Aberration',
      'Doubling Season',
      'Mystic Remora',
    ])
    expect(screen.getByText('6 cards in 3 of 4 decks')).toBeInTheDocument()
  })

  it.each([
    ['Rhystic Study', 'manual', 'Manual', `In 3 decks: ${ATRAXA}, Izzet Spells, ${CHAOS}`],
    ['Mana Drain', 'partial', 'Partial', `In 2 decks: ${ATRAXA}, Izzet Spells`],
    ['Contract from Below', 'unplayable', 'Unplayable', `In 1 deck: ${CHAOS}`],
    ['Mystic Remora', 'stale', 'Stale', `In 1 deck: ${CHAOS}`],
  ])('shows %s as %s with its decks, each a link to the builder', async (name, support, word, decks) => {
    await renderPage()
    await settled()
    await waitFor(() => expect(screen.queryByText('Reading card text…')).toBeNull())
    const item = itemOf(name)
    expect(item).toHaveAttribute('data-support', support)
    expect(within(item).getByText(word)).toBeInTheDocument()
    expect(item.querySelector('[data-slot="decks"]')).toHaveTextContent(decks)
    expect(
      within(item).getByRole('link', { name: new RegExp(`^${decks.split(': ')[1]?.split(', ')[0]}`) }),
    ).toHaveAttribute('href', expect.stringMatching(/^\/decks\/deck-/))
  })

  it('quotes each clause beside its reason, naming the face when it is not the front', async () => {
    await renderPage()
    await settled()
    await waitFor(() => expect(itemOf('Mana Drain').querySelector('blockquote')).not.toBeNull())
    const clauses = (name: string) =>
      [...itemOf(name).querySelectorAll('[data-clause]')].map((clause) => [
        clause.querySelector('blockquote')?.textContent?.replace('Text: ', ''),
        clause.querySelector('p')?.textContent?.replace('Reason: ', ''),
      ])
    expect(clauses('Mana Drain')).toEqual([
      [
        '“At the beginning of your next main phase, add an amount of {C} equal to that spell’s mana value.”',
        'The delayed mana is not implemented',
      ],
    ])
    expect(clauses('Delver of Secrets // Insectile Aberration')).toEqual([
      ['“transform Delver of Secrets”', 'Front face: transforming is not implemented'],
      ['“Flying”', 'Back face: flying of a back face is not implemented'],
    ])
  })

  it('quotes the whole text of a card with no clause of its own', async () => {
    await renderPage()
    await settled()
    await waitFor(() => expect(itemOf('Rhystic Study').querySelector('blockquote')).not.toBeNull())
    const clause = itemOf('Rhystic Study').querySelector('[data-clause]')
    expect(clause).toHaveTextContent(
      'Whenever an opponent casts a spell, you may draw a card unless that player pays {1}.',
    )
    expect(clause).toHaveTextContent('Played by hand')
  })

  it('shows the picture of each card by its oracle id', async () => {
    await renderPage()
    await settled()
    expect(itemOf('Mana Drain').querySelector('img')?.getAttribute('src')).toContain(
      '/api/images/oracle/oracle-mana-drain/0/small',
    )
  })

  it('arranges the cards by deck: each deck that holds some, with its own cards and a link to its page', async () => {
    const { user } = await renderPage()
    await settled()
    const arrange = screen.getByRole('group', { name: 'Arrange the list' })
    expect(within(arrange).getByRole('button', { name: 'By card' })).toHaveAttribute('aria-pressed', 'true')
    await user.click(within(arrange).getByRole('button', { name: 'By deck' }))
    const sections = [...document.querySelectorAll<HTMLElement>('section[data-deck]')]
    expect(sections.map((section) => section.querySelector('h2')?.textContent)).toEqual([
      ATRAXA,
      'Izzet Spells',
      CHAOS,
    ])
    expect(
      sections.map((section) =>
        cardNames(within(section).getByRole('list', { name: /^Cards not validated in / })),
      ),
    ).toEqual([
      ['Doubling Season', 'Mana Drain', 'Rhystic Study'],
      ['Delver of Secrets // Insectile Aberration', 'Mana Drain', 'Rhystic Study'],
      ['Contract from Below', 'Mystic Remora', 'Rhystic Study'],
    ])
    expect(sections[1]?.querySelector('[data-slot="decks"]')).toBeNull()
    expect(
      within(sections[1] as HTMLElement).getByRole('link', { name: '3 cards, this deck only' }),
    ).toHaveAttribute('href', '/decks/deck-izzet/unvalidated')
  })

  it.each([
    ['Partial', ['Mana Drain', 'Delver of Secrets // Insectile Aberration', 'Doubling Season']],
    ['Manual', ['Rhystic Study']],
    ['Stale', ['Mystic Remora']],
    ['Unplayable', ['Contract from Below']],
  ])('narrows the list to the %s cards', async (word, names) => {
    const { user } = await renderPage()
    await settled()
    const filter = screen.getByRole('group', { name: 'Show cards in state' })
    await user.click(within(filter).getByRole('button', { name: new RegExp(`^${word} `) }))
    expect(cardNames(await settled())).toEqual(names)
    expect(within(filter).getByRole('button', { name: new RegExp(`^${word} `) })).toHaveAttribute(
      'aria-pressed',
      'true',
    )
    expect(screen.getByText(new RegExp(`^${names.length} cards? in`))).toBeInTheDocument()
    await user.click(within(filter).getByRole('button', { name: /^All / }))
    expect(cardNames(await settled())).toHaveLength(6)
  })

  it('counts each state on its filter', async () => {
    await renderPage()
    await settled()
    const filter = screen.getByRole('group', { name: 'Show cards in state' })
    expect(
      within(filter)
        .getAllByRole('button')
        .map((button) => button.textContent),
    ).toEqual(['All (6)', 'Partial (3)', 'Manual (1)', 'Stale (1)', 'Unplayable (1)'])
  })

  it('asks for the decks that hold a flagged card and nothing for the deck that runs whole', async () => {
    const { decks } = await renderPage()
    await settled()
    expect(decks.asked).toContain('check deck-atraxa')
    expect(decks.asked).not.toContain('check deck-burn')
  })
})

describe('Copy as text', () => {
  it('copies one block a card in the exact format once the card texts are read', async () => {
    const { user } = await renderPage()
    await settled()
    await user.click(await screen.findByRole('button', { name: 'Copy as text' }))
    await waitFor(async () => expect(await navigator.clipboard.readText()).toBe(FULL_TEXT))
    expect(await screen.findByText('Copied')).toBeInTheDocument()
  })

  it('copies what the filter shows', async () => {
    const { user } = await renderPage()
    await settled()
    await user.click(screen.getByRole('button', { name: /^Manual / }))
    await user.click(await screen.findByRole('button', { name: 'Copy as text' }))
    await waitFor(async () => expect(await navigator.clipboard.readText()).toBe(FULL_TEXT.split('\n\n')[0]))
  })

  it('waits for the card texts: a disabled button says so instead of copying a list without quotes', async () => {
    const decks = fixtureDecksApi()
    const auth = createAuthStore({
      api: { ...offlineAccounts, myGames: async () => [] },
      initial: { status: 'signed-in', user: { id: 'ada', username: 'Ada_L' } },
    })
    await renderInRouter(
      <AuthContext value={auth}>
        <UnvalidatedPage api={decks} cards={{ get: () => new Promise(() => undefined) }} />
      </AuthContext>,
    )
    await settled()
    expect(screen.getByRole('button', { name: 'Reading card text…' })).toHaveAttribute(
      'aria-disabled',
      'true',
    )
    expect(screen.queryByRole('button', { name: 'Copy as text' })).toBeNull()
  })

  it('says so when card texts cannot be read and copies the reasons alone', async () => {
    const { user } = await renderPage({ cardsFail: true })
    await settled()
    expect(await screen.findByText(/The text of 6 cards could not be read/)).toBeInTheDocument()
    await user.click(await screen.findByRole('button', { name: 'Copy as text' }))
    await waitFor(async () =>
      expect((await navigator.clipboard.readText()).split('\n\n')[1]).toBe(
        `Mana Drain (Partial) - decks: ${ATRAXA}, Izzet Spells\n  - The delayed mana is not implemented`,
      ),
    )
  })
})

describe('one deck', () => {
  it('lists the cards of that deck only, without the decks line or the arrangement', async () => {
    const { decks } = await renderPage({ deckId: 'deck-izzet' })
    const list = await settled()
    expect(cardNames(list)).toEqual([
      'Delver of Secrets // Insectile Aberration',
      'Mana Drain',
      'Rhystic Study',
    ])
    expect(decks.asked).toEqual(['get deck-izzet'])
    expect(document.querySelector('[data-slot="decks"]')).toBeNull()
    expect(screen.queryByRole('group', { name: 'Arrange the list' })).toBeNull()
    expect(screen.getByText('3 cards in this deck')).toBeInTheDocument()
    expect(screen.getByText(/^Cards of Izzet Spells that the game does not run whole/)).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'All decks' })).toHaveAttribute('href', '/decks/unvalidated')
    expect(screen.getByRole('link', { name: 'Edit deck' })).toHaveAttribute('href', '/decks/deck-izzet')
  })

  it('copies its cards with the deck name', async () => {
    const { user } = await renderPage({ deckId: 'deck-izzet' })
    await settled()
    await user.click(await screen.findByRole('button', { name: 'Copy as text' }))
    await waitFor(async () =>
      expect(
        (await navigator.clipboard.readText()).split('\n\n').map((block) => block.split('\n')[0]),
      ).toEqual([
        'Delver of Secrets // Insectile Aberration (Partial) - decks: Izzet Spells',
        'Mana Drain (Partial) - decks: Izzet Spells',
        'Rhystic Study (Manual) - decks: Izzet Spells',
      ]),
    )
  })

  it('says a deck that runs whole needs nothing', async () => {
    await renderPage({ deckId: 'deck-burn' })
    expect(await screen.findByRole('heading', { name: 'The game runs every card' })).toBeInTheDocument()
    expect(screen.getByText('Every card of this deck is played by the game as written.')).toBeInTheDocument()
  })
})

describe('the states of the page', () => {
  it('says it is loading, and how far it has come', async () => {
    const decks = { ...fixtureDecksApi(), list: () => new Promise<never>(() => undefined) }
    const auth = createAuthStore({
      api: { ...offlineAccounts, myGames: async () => [] },
      initial: { status: 'signed-in', user: { id: 'ada', username: 'Ada_L' } },
    })
    await renderInRouter(
      <AuthContext value={auth}>
        <UnvalidatedPage api={decks} cards={fixtureCardsApi()} />
      </AuthContext>,
    )
    expect(await screen.findByText('Checking your decks…')).toBeInTheDocument()
    expect(screen.queryByRole('list', { name: 'Cards not validated' })).toBeNull()
  })

  it('shows how many decks are checked while it waits', async () => {
    await renderPage({ delayMs: 40 })
    expect(await screen.findByText(/^Checked \d of 3 decks…$/)).toBeInTheDocument()
    await settled()
  })

  it('says every card runs when no deck holds a flagged card', async () => {
    await renderPage({ decks: [FIXTURE_DECKS[1] as (typeof FIXTURE_DECKS)[number]] })
    expect(await screen.findByRole('heading', { name: 'The game runs every card' })).toBeInTheDocument()
    expect(screen.getByText('Every card of your decks is played by the game as written.')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Copy as text' })).toBeNull()
  })

  it('points at the decks page for an account without decks', async () => {
    await renderPage({ decks: [] })
    expect(await screen.findByRole('heading', { name: 'You have no decks yet' })).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'Go to your decks' })).toHaveAttribute('href', '/decks')
  })

  it('names a deck whose check failed, lists the rest and asks again on Try again', async () => {
    const { user, decks } = await renderPage({ checkFails: ['deck-izzet'] })
    const alert = await screen.findByText('Could not check 1 deck')
    expect(alert.closest('[data-slot="unchecked"]')).toHaveTextContent(
      'Izzet Spells is not in the list below.',
    )
    expect(cardNames(await settled())).not.toContain('Delver of Secrets // Insectile Aberration')
    await user.click(screen.getByRole('button', { name: 'Try again' }))
    await waitFor(() => expect(decks.asked.filter((call) => call === 'list')).toHaveLength(2))
  })

  it.each([
    ['network', 'Could not reach the server'],
    ['decks_unavailable', 'This server keeps no decks'],
  ] as const)('says a failed list (%s) in words', async (code, words) => {
    await renderPage({ listFails: code })
    expect(await screen.findByText(new RegExp(words))).toBeInTheDocument()
    expect(screen.queryByRole('list', { name: 'Cards not validated' })).toBeNull()
  })

  it('retries a failed list on Retry', async () => {
    const { user, decks } = await renderPage({ listFails: 'network' })
    await user.click(await screen.findByRole('button', { name: 'Retry' }))
    await waitFor(() => expect(decks.asked.filter((call) => call === 'list')).toHaveLength(2))
  })

  it('offers to sign in again, not to retry, when the session has ended', async () => {
    await renderPage({ listFails: 'unauthenticated' })
    const alert = await screen.findByRole('alert')
    expect(alert).toHaveTextContent('Your session has ended')
    expect(within(alert).getByRole('link', { name: 'Sign in' }).getAttribute('href')).toMatch(
      /^\/login\?next=(%2F|\/)decks(%2F|\/)unvalidated$/,
    )
    expect(within(alert).queryByRole('button', { name: 'Retry' })).toBeNull()
  })
})

describe('in the app', () => {
  const ADA = { username: 'Ada_L', password: 'correct horse battery' }
  const users = { [ADA.username]: ADA.password }

  it('sends a visitor who is not signed in to the sign-in form and back', async () => {
    const list = vi.spyOn(decksApi, 'list')
    const app = await renderApp('/decks/unvalidated', { accounts: { users } })
    expect(await screen.findByRole('form', { name: 'Sign in' })).toBeInTheDocument()
    expect(app.router.state.location.pathname).toBe('/login')
    expect(app.router.state.matches.at(-1)?.search).toEqual({ next: '/decks/unvalidated' })
    expect(list).not.toHaveBeenCalled()
    vi.restoreAllMocks()
  })

  it('opens the page of one deck at the address of its own and checks that deck', async () => {
    const get = vi
      .spyOn(decksApi, 'get')
      .mockResolvedValue(
        deckView({ id: 'deck-izzet', name: 'Izzet Spells', report: FIXTURE_DECKS[2]!.report }),
      )
    vi.spyOn(cardsApi, 'get').mockRejectedValue(new Error('offline'))
    await renderApp('/decks/deck-izzet/unvalidated', { accounts: { users, signedIn: ADA.username } })
    expect(await screen.findByText('3 cards in this deck')).toBeInTheDocument()
    expect(get).toHaveBeenCalledWith('deck-izzet')
    vi.restoreAllMocks()
  })
})
