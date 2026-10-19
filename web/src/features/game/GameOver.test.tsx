// Goal: the game over screen says the result in words and in a picture for every ending (a win, a loss, a draw, a concession, a result with no board to read), fits the window under the header with only the list of seats scrolling, and offers the ways on: a new game, the Play page and, for an account, My games.
import { screen, waitFor, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { useIsFitWindow } from '@/components/appChrome'
import { AuthProvider } from '@/features/auth/AuthProvider'
import { createAuthStore } from '@/features/auth/store'
import { createSession } from '@/play/session'
import { initialPlayState } from '@/play/store'
import type { GameResult, LossReason } from '@/protocol/generated'
import { startAccounts } from '@/test-utils/accountsServer'
import { clone } from '@/test-utils/batches'
import { renderInRouter } from '@/test-utils/router'
import { lateJoin } from '@/test-utils/transcripts'
import { GameContext } from './gameContext'
import type { GameContextValue } from './gameContext'
import { GameOver } from './GameOver'

interface Ending {
  result: GameResult
  /** Why each seat that is out lost, by seat. */
  lost?: Record<number, LossReason>
  seats?: number
  /** The seat this page plays. */
  me?: number
  /** False for a page that never got a view of the board. */
  board?: boolean
}

/** A game context at the end of a game: a session whose store holds the result and, unless told not to, the board. */
function endedGame({ result, lost = {}, seats = 2, me = 0, board = true }: Ending): GameContextValue {
  const session = createSession({ game: 'over', name: 'test' })
  const final = clone(lateJoin.expected)
  const first = final.players[0]
  if (first === undefined) throw new Error('the fixture has no players')
  final.players = Array.from({ length: seats }, (_, seat) => ({
    ...clone(first),
    seat,
    lost: lost[seat] ?? null,
    life: lost[seat] === undefined ? 14 : 0,
  }))
  session.store.setState({
    ...initialPlayState(),
    status: 'open',
    seat: me,
    seatCount: seats,
    view: board ? final : null,
    result,
  })
  return { game: 'over', session, lookup: { game: null, missing: false, failed: false } }
}

const ENDINGS: [string, Ending, string, string, string][] = [
  ['a win on the board', { result: { Winner: 0 }, lost: { 1: 'ZeroLife' } }, 'You won', 'Game over', 'won'],
  [
    'a loss on the board',
    { result: { Winner: 1 }, lost: { 0: 'ZeroLife' } },
    'You lost',
    'Game over',
    'lost',
  ],
  ['a draw', { result: 'Draw' }, 'The game is a draw', 'Game over', 'drawn'],
  [
    'the other seat conceding',
    { result: { Winner: 0 }, lost: { 1: 'Conceded' } },
    'You won',
    'Game over · Conceded',
    'conceded',
  ],
  [
    'this seat conceding',
    { result: { Winner: 1 }, lost: { 0: 'Conceded' } },
    'You lost',
    'Game over · Conceded',
    'conceded',
  ],
  [
    'a result with no board to read',
    { result: { Winner: 1 }, board: false },
    'Seat 2 won',
    'Game over',
    'lost',
  ],
]

const ICON = {
  won: 'lucide-trophy',
  lost: 'lucide-swords',
  drawn: 'lucide-handshake',
  conceded: 'lucide-flag',
}

const shown = (game: GameContextValue) => (
  <GameContext value={game}>
    <GameOver />
  </GameContext>
)

describe('the result', () => {
  it.each(ENDINGS)(
    'for %s says the headline, the line above it and draws its picture',
    async (_what, ending, headline, eyebrow, kind) => {
      await renderInRouter(shown(endedGame(ending)))
      expect(screen.getByRole('heading', { level: 1, name: headline })).toHaveFocus()
      expect(screen.getByText(eyebrow)).toBeInTheDocument()
      const section = screen.getByRole('region', { name: headline })
      expect(section).toHaveAttribute('data-end', kind)
      const icon = section.querySelector('header svg')
      expect(icon).toHaveAttribute('aria-hidden', 'true')
      expect(icon).toHaveClass(ICON[kind as keyof typeof ICON])
    },
  )

  it('says in words how a seat that conceded lost, beside the winner, whoever the page plays', async () => {
    await renderInRouter(shown(endedGame({ result: { Winner: 0 }, lost: { 1: 'Conceded' } })))
    expect(screen.getByText('Seat 2 conceded.')).toBeInTheDocument()
    const rows = within(screen.getByRole('list', { name: 'How each seat finished' })).getAllByRole('listitem')
    expect(rows[0]).toHaveTextContent('Winner')
    expect(rows[1]).toHaveTextContent('Lost: conceded')
  })

  it('lists no seats when the page never saw the board, and still offers the way on', async () => {
    await renderInRouter(shown(endedGame({ result: { Winner: 1 }, board: false })))
    expect(screen.queryByRole('list', { name: 'How each seat finished' })).toBeNull()
    expect(screen.getByRole('link', { name: 'Create another game' })).toBeInTheDocument()
  })

  it('shows nothing while there is no result', async () => {
    const game = endedGame({ result: 'Draw' })
    game.session.store.setState({ result: null })
    await renderInRouter(shown(game))
    expect(screen.queryByRole('heading')).toBeNull()
  })
})

describe('the ways on', () => {
  it('offers a new game and the Play page to a guest, and nothing about My games', async () => {
    await renderInRouter(shown(endedGame({ result: { Winner: 0 }, lost: { 1: 'ZeroLife' } })))
    const links = screen
      .getAllByRole('link')
      .filter((link) => link.closest('main, [data-slot="page"]') !== null)
    expect(links.map((link) => [link.textContent, link.getAttribute('href')])).toEqual([
      ['Create another game', '/new'],
      ['Back to Play', '/'],
    ])
  })

  it('adds My games for an account, last, so the main way on stays first', async () => {
    const store = createAuthStore({
      api: startAccounts({ users: { Ada_L: 'correct horse battery' }, signedIn: 'Ada_L' }).api,
    })
    await renderInRouter(
      <AuthProvider store={store}>
        {shown(endedGame({ result: { Winner: 0 }, lost: { 1: 'ZeroLife' } }))}
      </AuthProvider>,
    )
    expect(await screen.findByRole('link', { name: 'My games' })).toHaveAttribute('href', '/games')
    const page = screen.getByRole('region', { name: 'You won' })
    expect(
      within(page)
        .getAllByRole('link')
        .map((link) => link.textContent),
    ).toEqual(['Create another game', 'Back to Play', 'My games'])
  })

  it('makes the new game the one primary button and the others outlined', async () => {
    await renderInRouter(shown(endedGame({ result: 'Draw' })))
    expect(screen.getByRole('link', { name: 'Create another game' })).toHaveClass('bg-primary')
    expect(screen.getByRole('link', { name: 'Back to Play' })).not.toHaveClass('bg-primary')
  })
})

describe('the fit', () => {
  const page = () => screen.getByRole('region', { name: /./ }).closest('[data-slot="page"]')

  it('sits in the page width and takes the window under the header, while it is on screen', async () => {
    function Probe() {
      return <p data-testid="fit">{String(useIsFitWindow())}</p>
    }
    const { unmount } = await renderInRouter(
      <>
        <Probe />
        {shown(endedGame({ result: 'Draw' }))}
      </>,
    )
    expect(page()).toHaveAttribute('data-width', 'page')
    expect(page()).toHaveClass('flex', 'min-h-0', 'flex-1', 'flex-col')
    await waitFor(() => expect(screen.getByTestId('fit')).toHaveTextContent('true'))
    unmount()
  })

  it('scrolls the list of seats inside itself, so the headline and the buttons stay in the window', async () => {
    const lost = Object.fromEntries(Array.from({ length: 11 }, (_, i) => [i + 1, 'ZeroLife'])) as Record<
      number,
      LossReason
    >
    await renderInRouter(shown(endedGame({ result: { Winner: 0 }, seats: 12, lost })))
    const list = screen.getByRole('list', { name: 'How each seat finished' })
    expect(within(list).getAllByRole('listitem')).toHaveLength(12)
    expect(list).toHaveClass('overflow-y-auto', 'min-h-0')
    for (const link of screen.getAllByRole('link')) expect(list).not.toContainElement(link)
    expect(list).not.toContainElement(screen.getByRole('heading', { level: 1 }))
  })

  it('never asks a seat row to stay on one line, so a long status wraps instead of pushing the page sideways', async () => {
    await renderInRouter(shown(endedGame({ result: { Winner: 0 }, lost: { 1: 'EmptyLibrary' } })))
    const row = within(screen.getByRole('list', { name: 'How each seat finished' })).getAllByRole(
      'listitem',
    )[1]
    expect(row).toHaveTextContent('Lost: could not draw from an empty library')
    expect(row).toHaveClass('grid')
    expect(row?.className).not.toContain('whitespace-nowrap')
  })
})
