// Goal: the precon page lists every precon with its deck check as an icon and words, filters by search, set and state, shows the loading, empty, failed and no-card-data states, opens the details with each card's mark and clauses and a Copy list of the cards that are not Verified, and Play makes a game with `precon:<code>` (one click for an account, a name first for a guest) and opens its lobby, while a deck the game cannot take has a disabled Play and a 422 says so.
import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { AuthContext } from '@/features/auth/context'
import { createAuthStore, offlineAccounts } from '@/features/auth/store'
import { startCards } from '../builder/fakeCards'
import { cardDetail } from '../builder/fakeCards'
import { spanOf } from '../decks/unvalidated/fixture'
import { ApiError } from '@/net/api'
import type { Api } from '@/net/api'
import type { CardsApi } from '@/net/cards'
import { PreconError } from '@/net/precons'
import { renderInRouter } from '@/test-utils/router'
import { board, cardLine, checked, issue, preconItem, preconView, reason, startPrecons } from './fakePrecons'
import type { PreconsScript } from './fakePrecons'
import { PreconsPage } from './PreconsPage'

afterEach(() => {
  vi.restoreAllMocks()
  window.localStorage.clear()
})

const LIST = [
  checked(
    preconItem({ code: 'm3c-eldrazi-unbound', name: 'Eldrazi Unbound', set: 'M3C', commanders: ['Ulalek'] }),
    { partial: 1, manual: 2 },
  ),
  preconItem({ code: 'c18-adaptive-enchantment', name: 'Adaptive Enchantment' }),
  checked(
    preconItem({
      code: 'c20-arcane-maelstrom',
      name: 'Arcane Maelstrom',
      set: 'C20',
      commanders: ['Kalamax'],
    }),
    { unplayable: 1 },
  ),
  checked(
    preconItem({
      code: 'chl-izzet-challenger',
      name: 'Izzet Challenger',
      set: 'CHL',
      commanders: [],
      format: 'freeform',
      card_count: 75,
    }),
    { manual: 1 },
  ),
]

const created = { game: 'g1', code: 'K7M29QXA', seats: [{ seat: 0, token: 'tok0' }] }

interface Options extends PreconsScript {
  user?: string
  createGame?: Api['createGame']
  /** The card endpoint the clauses are quoted from; a server that knows Kozilek and Seer by default. */
  cards?: Pick<CardsApi, 'get'>
}

const KOZILEK_TEXT = 'When you cast this spell, draw two cards. Then discard two cards.'
const SEER_TEXT = 'Flash\nWhen this enters, exile a card from an opponent\u2019s hand.'

const textOf = (name: string, text: string) =>
  cardDetail(name, { faces: [{ ...cardDetail(name).faces[0]!, oracle_text: text }] })

const cardServer = () =>
  startCards({ details: [textOf('Kozilek', KOZILEK_TEXT), textOf('Seer', SEER_TEXT)] }).api

async function open({ user, createGame, cards = cardServer(), ...script }: Options = {}) {
  const server = startPrecons({ precons: LIST, ...script })
  const games = { createGame: vi.fn(createGame ?? (() => Promise.resolve(created))) }
  const auth = createAuthStore({
    api: { ...offlineAccounts, myGames: async () => [] },
    initial:
      user === undefined
        ? { status: 'signed-out', user: null }
        : { status: 'signed-in', user: { id: 'u1', username: user, is_admin: false } },
  })
  const view = await renderInRouter(
    <AuthContext.Provider value={auth}>
      <PreconsPage api={server.api} games={games} cards={cards} />
    </AuthContext.Provider>,
  )
  return { ...view, server, games, user: userEvent.setup() }
}

const rows = () => screen.getAllByRole('row').slice(1)
const rowOf = (name: string) => {
  const row = rows().find((item) => within(item).queryByRole('rowheader', { name: new RegExp(name) }))
  if (row === undefined) throw new Error(`no row for ${name}`)
  return row
}
const listed = () => screen.findByRole('table', { name: 'Precons, newest release first' })

describe('the list', () => {
  it('shows each precon with its set, release, commanders, card count and a check badge of an icon and words', async () => {
    await open()
    await listed()
    expect(rows()).toHaveLength(4)
    const eldrazi = rowOf('Eldrazi Unbound')
    expect(within(eldrazi).getByText('M3C')).toBeInTheDocument()
    expect(within(eldrazi).getByText('Ulalek')).toBeInTheDocument()
    expect(within(eldrazi).getByText('100')).toBeInTheDocument()
    expect(
      within(eldrazi).getByText('3 cards played by hand').closest('[data-precon-status]'),
    ).toHaveAttribute('data-precon-status', 'hand')
    expect(within(rowOf('Adaptive Enchantment')).getByText('Whole deck runs')).toBeInTheDocument()
    expect(within(rowOf('Arcane Maelstrom')).getByText('Cannot be taken')).toBeInTheDocument()
    expect(within(rowOf('Izzet Challenger')).getByText('No commander')).toBeInTheDocument()
    // The state is words and an icon, not a colour alone.
    for (const badge of document.querySelectorAll('[data-precon-status]')) {
      expect(badge.querySelector('svg')).not.toBeNull()
      expect(badge.textContent?.length).toBeGreaterThan(5)
    }
    expect(screen.getByRole('status', { name: '' })).toHaveTextContent('4 precons')
  })

  it.each([
    ['the search by name', 'adaptive', ['Adaptive Enchantment']],
    ['the search by commander', 'kalamax', ['Arcane Maelstrom']],
    ['the search by set', 'chl', ['Izzet Challenger']],
  ])('narrows with %s', async (_, text, names) => {
    const page = await open()
    await listed()
    await page.user.type(screen.getByRole('searchbox', { name: 'Search' }), text)
    expect(rows().map((row) => within(row).getByRole('rowheader').textContent)).toEqual(
      names.map((name) => expect.stringContaining(name)),
    )
    expect(screen.getByRole('status', { name: '' })).toHaveTextContent(`${names.length} of 4 precons`)
  })

  it('narrows by set and by deck check, and clears every filter', async () => {
    const page = await open()
    await listed()
    await page.user.selectOptions(screen.getByLabelText('Set'), 'M3C')
    expect(rows()).toHaveLength(1)
    await page.user.selectOptions(screen.getByLabelText('Set'), '')
    await page.user.selectOptions(screen.getByLabelText('Deck check'), 'Cannot be taken')
    expect(rows().map((row) => within(row).getByRole('rowheader').textContent)).toEqual([
      expect.stringContaining('Arcane Maelstrom'),
    ])
    await page.user.type(screen.getByRole('searchbox', { name: 'Search' }), 'zzz')
    expect(await screen.findByRole('heading', { name: 'No precons match' })).toBeInTheDocument()
    const empty = screen
      .getByRole('heading', { name: 'No precons match' })
      .closest('[data-slot="empty-state"]')
    await page.user.click(within(empty as HTMLElement).getByRole('button', { name: 'Clear filters' }))
    expect(rows()).toHaveLength(4)
  })
})

describe('the reads', () => {
  const READS = [
    checked(preconItem({ code: 'r1', name: 'On Reads', set: 'RRR' }), { read: 4 }),
    checked(preconItem({ code: 'r2', name: 'Waiting', set: 'WWW' }), { manual: 3, toRead: 2 }),
    preconItem({ code: 'r3', name: 'Plain', set: 'PPP' }),
  ]

  it('gives a deck that runs on reads its own badge and a deck with drafts waiting its count', async () => {
    await open({ precons: READS })
    await listed()
    const onReads = rowOf('On Reads')
    expect(within(onReads).getByText('Runs on reads').closest('[data-precon-status]')).toHaveAttribute(
      'data-precon-status',
      'read',
    )
    expect(within(onReads).queryByText(/to read/)).toBeNull()
    const waiting = rowOf('Waiting')
    expect(within(waiting).getByText('3 cards played by hand')).toBeInTheDocument()
    expect(within(waiting).getByText('2 cards to read').closest('[data-slot="to-read"]')).toHaveAttribute(
      'data-read-path',
      '/read?precon=r2',
    )
    expect(within(rowOf('Plain')).queryByText(/to read/)).toBeNull()
  })

  it('filters by the state of a deck that runs on reads', async () => {
    const page = await open({ precons: READS })
    await listed()
    await page.user.selectOptions(screen.getByLabelText('Deck check'), 'Runs on reads')
    expect(rows().map((row) => within(row).getByRole('rowheader').textContent)).toEqual([
      expect.stringContaining('On Reads'),
    ])
  })

  it('says the reads and the drafts waiting in the details', async () => {
    const page = await open({ precons: READS })
    await listed()
    await page.user.click(screen.getByRole('button', { name: 'Details of Waiting' }))
    const dialog = await screen.findByRole('dialog', { name: 'Waiting' })
    expect(within(dialog).getByText('2 cards to read')).toBeInTheDocument()
    expect(within(dialog).getByText('97 of 100 cards verified; 3 played by hand')).toBeInTheDocument()
  })
})

describe('the other states', () => {
  it('says it is loading while the list is on its way', async () => {
    await open({ override: () => new Promise<Response>(() => undefined) })
    expect(screen.getByText('Loading the precons…')).toBeInTheDocument()
  })

  it('has an empty state for a server with no precons', async () => {
    await open({ precons: [] })
    expect(await screen.findByRole('heading', { name: 'No precons loaded' })).toBeInTheDocument()
  })

  it('says a server without card data has no precons, with no retry', async () => {
    await open({ fail: { code: 'cards_unavailable', status: 503 } })
    expect(await screen.findByRole('heading', { name: 'No card data on this server' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Retry' })).toBeNull()
  })

  it('shows a failed load with its words and a Retry that asks again', async () => {
    let fail = true
    const page = await open({
      override: (call) =>
        fail && call.url === '/api/precons' ? Promise.reject(new TypeError('offline')) : undefined,
    })
    expect(await screen.findByText('Could not load the precons')).toBeInTheDocument()
    expect(screen.getByText(/Could not reach the server/)).toBeInTheDocument()
    fail = false
    await page.user.click(screen.getByRole('button', { name: 'Retry' }))
    await listed()
    expect(rows()).toHaveLength(4)
  })
})

describe('the details', () => {
  const view = preconView(LIST[0]!, undefined, {
    boards: [
      board('commander', [cardLine('Ulalek')]),
      board('main', [
        cardLine('Sol Ring'),
        cardLine('Kozilek', 'partial', {
          reasons: [
            reason(
              'the discard is not implemented',
              spanOf(KOZILEK_TEXT, 'Then discard two cards.').start,
              spanOf(KOZILEK_TEXT, 'Then discard two cards.').end,
            ),
          ],
        }),
        cardLine('Seer', 'manual'),
      ]),
    ],
  })

  it('lists the cards by board with a mark each, and the clauses played by hand', async () => {
    const page = await open({ views: [view] })
    await listed()
    await page.user.click(screen.getByRole('button', { name: 'Details of Eldrazi Unbound' }))
    const dialog = await screen.findByRole('dialog', { name: 'Eldrazi Unbound' })
    const commander = await within(dialog).findByRole('region', { name: /Commander/ })
    expect(within(commander).getByText('Ulalek')).toBeInTheDocument()
    const main = within(dialog).getByRole('region', { name: /Main deck/ })
    const kozilek = within(main).getByText('Kozilek').closest('li')
    expect(kozilek).toHaveAttribute('data-support', 'partial')
    expect(within(kozilek as HTMLElement).getByText('Partial')).toBeInTheDocument()
    expect(kozilek).toHaveTextContent('Played by hand:')
    // The words of the card the clause covers stand beside the reason, once the card has been read.
    await waitFor(() => expect(kozilek?.querySelector('blockquote')).not.toBeNull())
    const clause = (kozilek as HTMLElement).querySelector('[data-clause]')
    expect(clause?.querySelector('blockquote')).toHaveTextContent('“Then discard two cards.”')
    expect(clause?.querySelector('p')).toHaveTextContent('The discard is not implemented')
    const seer = within(main).getByText('Seer').closest('li')
    await waitFor(() => expect(seer?.querySelector('blockquote')).not.toBeNull())
    expect(seer?.querySelector('blockquote')).toHaveTextContent(
      '“Flash / When this enters, exile a card from an opponent\u2019s hand.”',
    )
    expect(seer).toHaveTextContent('Played by hand')
    expect(within(main).getByText('Sol Ring').closest('li')).toHaveAttribute('data-support', 'verified')
    expect(within(main).getByText('Seer').closest('li')).toHaveTextContent('Manual')
    await page.user.click(within(dialog).getByRole('switch', { name: 'Only cards played by hand' }))
    expect(within(main).queryByText('Sol Ring')).toBeNull()
    expect(within(main).getByText('Seer')).toBeInTheDocument()
    expect(screen.queryByRole('region', { name: /Commander/ })).toBeNull()
  })

  it('copies the cards that are not Verified, a block each with its clauses quoted beside the reasons', async () => {
    const page = await open({ views: [view] })
    await listed()
    await page.user.click(screen.getByRole('button', { name: 'Details of Eldrazi Unbound' }))
    await page.user.click(
      await screen.findByRole('button', { name: 'Copy list of the cards played by hand' }),
    )
    // user-event's own clipboard stands in for the browser's.
    await waitFor(async () =>
      expect(await navigator.clipboard.readText()).toBe(
        [
          'Kozilek (Partial) - decks: Eldrazi Unbound',
          '  - "Then discard two cards.": The discard is not implemented',
          '',
          'Seer (Manual) - decks: Eldrazi Unbound',
          '  - "Flash / When this enters, exile a card from an opponent\u2019s hand.": Played by hand',
        ].join('\n'),
      ),
    )
  })

  it('waits for the card texts before it copies, so a list never leaves without its quotes', async () => {
    const page = await open({ views: [view], cards: { get: () => new Promise(() => undefined) } })
    await listed()
    await page.user.click(screen.getByRole('button', { name: 'Details of Eldrazi Unbound' }))
    expect(await screen.findByRole('button', { name: 'Reading card text…' })).toHaveAttribute(
      'aria-disabled',
      'true',
    )
    expect(screen.queryByRole('button', { name: 'Copy list of the cards played by hand' })).toBeNull()
  })

  it('shows and copies the reasons alone when the card texts cannot be read', async () => {
    const page = await open({ views: [view], cards: { get: () => Promise.reject(new Error('offline')) } })
    await listed()
    await page.user.click(screen.getByRole('button', { name: 'Details of Eldrazi Unbound' }))
    await page.user.click(
      await screen.findByRole('button', { name: 'Copy list of the cards played by hand' }),
    )
    await waitFor(async () =>
      expect(await navigator.clipboard.readText()).toBe(
        [
          'Kozilek (Partial) - decks: Eldrazi Unbound',
          '  - The discard is not implemented',
          '',
          'Seer (Manual) - decks: Eldrazi Unbound',
          '  - Played by hand',
        ].join('\n'),
      ),
    )
    expect(document.querySelector('blockquote')).toBeNull()
  })

  it('names what stops a deck the game cannot take, from the server’s issues', async () => {
    const blocked = preconView(LIST[2]!, undefined, { issues: [issue('Sol Ring appears twice.')] })
    const page = await open({ views: [blocked] })
    await listed()
    await page.user.click(screen.getByRole('button', { name: 'Details of Arcane Maelstrom' }))
    const dialog = await screen.findByRole('dialog', { name: 'Arcane Maelstrom' })
    expect(await within(dialog).findByText('Sol Ring appears twice.')).toBeInTheDocument()
    expect(within(dialog).getByRole('button', { name: 'Play Arcane Maelstrom' })).toHaveAttribute(
      'aria-disabled',
      'true',
    )
  })

  it('says a failed load of the cards and asks again on Retry, spending one request a precon until then', async () => {
    let fail = true
    const page = await open({
      views: [view],
      override: (call) =>
        fail && call.url.endsWith('/m3c-eldrazi-unbound')
          ? Promise.reject(new TypeError('offline'))
          : undefined,
    })
    await listed()
    await page.user.click(screen.getByRole('button', { name: 'Details of Eldrazi Unbound' }))
    expect(await screen.findByText('Could not load the cards')).toBeInTheDocument()
    fail = false
    await page.user.click(screen.getByRole('button', { name: 'Retry' }))
    expect(await screen.findByRole('region', { name: /Main deck/ })).toBeInTheDocument()
    await page.user.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
    await page.user.click(screen.getByRole('button', { name: 'Details of Eldrazi Unbound' }))
    await screen.findByRole('region', { name: /Main deck/ })
    expect(page.server.calls.filter((call) => call.url.endsWith('/m3c-eldrazi-unbound'))).toHaveLength(2)
  })
})

describe('Play', () => {
  it('makes a game with the precon as the account’s deck in one click and opens the lobby on seat 1', async () => {
    const page = await open({ user: 'Ada_L' })
    await listed()
    await page.user.click(screen.getByRole('button', { name: 'Play Eldrazi Unbound' }))
    await waitFor(() => expect(page.router.state.location.pathname).toBe('/game/g1'))
    expect(page.games.createGame).toHaveBeenCalledWith({
      format: 'commander',
      seats: [{ name: 'Ada_L', deck: 'precon:m3c-eldrazi-unbound' }, { name: 'Player 2' }],
      visibility: 'public',
      held: 1,
    })
    expect(screen.getByTestId('game-stub')).toHaveTextContent('g1 /game/g1#seat=0&token=tok0')
  })

  it('makes a Normal game for a deck without a commander', async () => {
    const page = await open({ user: 'Ada_L' })
    await listed()
    await page.user.click(screen.getByRole('button', { name: 'Play Izzet Challenger' }))
    await waitFor(() => expect(page.games.createGame).toHaveBeenCalled())
    expect(page.games.createGame.mock.calls[0]?.[0]).toMatchObject({
      format: 'normal',
      seats: [{ name: 'Ada_L', deck: 'precon:chl-izzet-challenger' }, { name: 'Player 2' }],
    })
  })

  it('asks a guest for a name, remembers it and then makes the game', async () => {
    const page = await open()
    await listed()
    await page.user.click(screen.getByRole('button', { name: 'Play Adaptive Enchantment' }))
    const dialog = await screen.findByRole('dialog', { name: 'Play Adaptive Enchantment' })
    expect(page.games.createGame).not.toHaveBeenCalled()
    await page.user.type(within(dialog).getByLabelText('Your name'), 'Bo')
    await page.user.click(within(dialog).getByRole('button', { name: 'Play' }))
    await waitFor(() => expect(page.router.state.location.pathname).toBe('/game/g1'))
    expect(page.games.createGame.mock.calls[0]?.[0]).toMatchObject({
      seats: [{ name: 'Bo', deck: 'precon:c18-adaptive-enchantment' }, { name: 'Player 2' }],
    })
    expect(window.localStorage.getItem('magic.guestName')).toBe('Bo')
  })

  it('has a Play that cannot be pressed for a deck the game cannot take, and sends nothing', async () => {
    const page = await open({ user: 'Ada_L' })
    await listed()
    const play = screen.getByRole('button', { name: 'Play Arcane Maelstrom' })
    expect(play).toHaveAttribute('aria-disabled', 'true')
    expect(play).toHaveAccessibleDescription('Cannot be taken')
    await page.user.click(play)
    expect(page.games.createGame).not.toHaveBeenCalled()
  })

  it('says so when the server refuses the deck, and offers the reasons', async () => {
    const page = await open({
      user: 'Ada_L',
      createGame: () =>
        Promise.reject(new ApiError('unknown', 422, 'that deck cannot be played in this game')),
    })
    await listed()
    await page.user.click(screen.getByRole('button', { name: 'Play Eldrazi Unbound' }))
    const alert = await screen.findByRole('alert')
    expect(alert).toHaveTextContent('Could not start a game with Eldrazi Unbound')
    expect(alert).toHaveTextContent('The game would not take Eldrazi Unbound')
    expect(page.router.state.location.pathname).toBe('/')
    await page.user.click(within(alert).getByRole('button', { name: 'Show why' }))
    expect(await screen.findByRole('dialog', { name: 'Eldrazi Unbound' })).toBeInTheDocument()
  })

  it('keeps a guest’s dialog up with the reason when the server cannot be reached, so the same name can be sent again', async () => {
    let fails = true
    const page = await open({
      createGame: () =>
        fails ? Promise.reject(new ApiError('network', 0, 'offline')) : Promise.resolve(created),
    })
    await listed()
    await page.user.click(screen.getByRole('button', { name: 'Play Adaptive Enchantment' }))
    const dialog = await screen.findByRole('dialog')
    await page.user.type(within(dialog).getByLabelText('Your name'), 'Bo')
    await page.user.click(within(dialog).getByRole('button', { name: 'Play' }))
    expect(await within(dialog).findByText(/Could not reach the server/)).toBeInTheDocument()
    fails = false
    await page.user.click(within(dialog).getByRole('button', { name: 'Play' }))
    await waitFor(() => expect(page.router.state.location.pathname).toBe('/game/g1'))
  })

  it('names a failed list load as a PreconError, not a crash, when the error is not one of ours', async () => {
    const server = startPrecons({ precons: LIST })
    const odd = { list: () => Promise.reject(new Error('boom')), get: server.api.get }
    await renderInRouter(<PreconsPage api={odd} games={{ createGame: vi.fn() }} />)
    expect(await screen.findByText('Something unexpected went wrong. Try again.')).toBeInTheDocument()
    expect(new PreconError('network', 0, 'x').name).toBe('PreconError')
  })
})
