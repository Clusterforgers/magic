// Goal: the game over screen says the result in words and in a picture for every ending (a win, a loss, a draw, a concession, a result with no board to read), celebrates a winner or a winning team for every seat (the burst for each winner, a banner naming them for the others), names a winning team and its players as decoration that keeps still under reduced motion, gives each seat's time on its own turns, fits the window under the header with only the list of seats scrolling, and offers the ways on: a new game, the Play page and, for an account, My games.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { screen, waitFor, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { useIsFitWindow } from '@/components/appChrome'
import { AuthProvider } from '@/features/auth/AuthProvider'
import { createAuthStore } from '@/features/auth/store'
import { createSession } from '@/play/session'
import { initialPlayState } from '@/play/store'
import { winningSeats } from '@/lib/teams'
import type { GameResult, LossReason, Teams } from '@/protocol/generated'
import { startAccounts } from '@/test-utils/accountsServer'
import { clone } from '@/test-utils/batches'
import { renderInRouter } from '@/test-utils/router'
import { lateJoin } from '@/test-utils/transcripts'
import { GameContext } from './gameContext'
import type { GameContextValue } from './gameContext'
import { GameOver } from './GameOver'
import { pieceFlight, WIN_PIECES } from './WinBurst'

interface Ending {
  result: GameResult
  /** Why each seat that is out lost, by seat. */
  lost?: Record<number, LossReason>
  seats?: number
  /** The seat this page plays. */
  me?: number
  /** False for a page that never got a view of the board. */
  board?: boolean
  /** The teams of the table, carried in the view's house rules. */
  teams?: Teams
}

/** A game context at the end of a game: a session whose store holds the result and, unless told not to, the board. */
function endedGame({ result, lost = {}, seats = 2, me = 0, board = true, teams }: Ending): GameContextValue {
  const session = createSession({ game: 'over', name: 'test' })
  const final = clone(lateJoin.expected)
  const first = final.players[0]
  if (first === undefined) throw new Error('the fixture has no players')
  final.players = Array.from({ length: seats }, (_, seat) => ({
    ...clone(first),
    seat,
    lost: lost[seat] ?? null,
    life: lost[seat] === undefined ? 14 : 0,
    turn_time_ms: 725_000 + seat * 3_000_000,
  }))
  if (teams !== undefined) final.house_rules = { teams }
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

const GIANT: Teams = { mode: 'two_headed_giant', seats: [0, 0, 1, 1] }
const ALTERNATE: Teams = { mode: 'teams', seats: [0, 1, 0, 1] }

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
  [
    "a Two-Headed Giant win for this seat's team",
    { result: { Team: 0 }, seats: 4, me: 1, teams: GIANT, lost: { 2: 'ZeroLife', 3: 'ZeroLife' } },
    'Your team won',
    'Game over',
    'won',
  ],
  [
    "a team loss for this seat's team",
    { result: { Team: 1 }, seats: 4, teams: ALTERNATE, lost: { 0: 'ZeroLife', 2: 'ZeroLife' } },
    'You lost',
    'Game over',
    'lost',
  ],
]

const winCss = readFileSync(join(import.meta.dirname, 'winBurst.css'), 'utf8')

const winnersOf = (ending: Ending): number[] => winningSeats(ending.result, ending.teams)

/** The words of the banner every seat but the winners sees, for the stand-in names of a page without a lookup. */
function bannerOf(ending: Ending): string | null {
  const { result } = ending
  if (result === 'Draw') return null
  if ('Winner' in result) return `Seat ${result.Winner + 1} won`
  const members = winnersOf(ending).map((seat) => `Seat ${seat + 1}`)
  return `Team ${result.Team + 1} (${members.join(' & ')}) won`
}

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
      // The winner's picture is the trophy, also when the last opponent conceded.
      const mine = winnersOf(ending).includes(ending.me ?? 0)
      expect(icon).toHaveClass(mine ? ICON.won : ICON[kind as keyof typeof ICON])
    },
  )

  it.each(ENDINGS)(
    'celebrates %s: the burst for the winner, a banner naming the winner for every other seat',
    async (...args) => {
      const [, ending, headline] = args
      await renderInRouter(shown(endedGame(ending)))
      const section = screen.getByRole('region', { name: headline })
      const mine = winnersOf(ending).includes(ending.me ?? 0)
      const words = bannerOf(ending)
      const pieces = section.querySelectorAll('[data-slot="win-piece"]')
      expect(pieces).toHaveLength(mine ? WIN_PIECES : 0)
      expect(section.querySelector('[data-slot="win-badge"]') !== null).toBe(mine)
      const banner = section.querySelector('[data-slot="win-banner"]')
      if (words === null || mine) expect(banner).toBeNull()
      else expect(banner).toHaveTextContent(words)
      // The celebration is decoration: nothing in it is read out and nothing in it takes a click.
      for (const piece of pieces) expect(piece.closest('[aria-hidden="true"]')).not.toBeNull()
      if (banner !== null) {
        expect(banner).toHaveAttribute('aria-hidden', 'true')
        expect(banner).toHaveClass('pointer-events-none')
      }
    },
  )

  it('keeps every moving part of the celebration still under reduced motion', () => {
    const still = winCss.slice(winCss.indexOf('@media (prefers-reduced-motion: reduce)'))
    const moving = [...winCss.matchAll(/\[data-slot='([\w-]+)'\]\s*\{[^}]*animation:/g)].map((m) => m[1])
    expect(moving).toEqual(expect.arrayContaining(['win-badge', 'win-banner', 'win-piece']))
    for (const slot of moving) expect(still).toContain(`[data-slot='${slot}']`)
    expect(still).toMatch(/animation:\s*none/)
  })

  it('flies every piece of the burst its own way', () => {
    const flights = Array.from({ length: WIN_PIECES }, (_, n) => pieceFlight(n))
    expect(new Set(flights.map((flight) => `${flight.dx},${flight.dy}`)).size).toBe(WIN_PIECES)
    expect(flights.every((flight) => Math.hypot(flight.dx, flight.dy) > 20)).toBe(true)
  })

  it('says in words how a seat that conceded lost, beside the winner, whoever the page plays', async () => {
    await renderInRouter(shown(endedGame({ result: { Winner: 0 }, lost: { 1: 'Conceded' } })))
    expect(screen.getByText('Seat 2 conceded.')).toBeInTheDocument()
    const rows = within(screen.getByRole('list', { name: 'How each seat finished' })).getAllByRole('listitem')
    expect(rows[0]).toHaveTextContent('Winner')
    expect(rows[1]).toHaveTextContent('Lost: conceded')
  })

  it('names the winning team and its players, and marks every seat with its team', async () => {
    const ending: Ending = {
      result: { Team: 1 },
      seats: 4,
      me: 0,
      teams: GIANT,
      lost: { 0: 'ZeroLife', 1: 'ZeroLife' },
    }
    await renderInRouter(shown(endedGame(ending)))
    expect(screen.getByText(/^Team 2 wins — Seat 3 & Seat 4\./)).toBeInTheDocument()
    const rows = within(screen.getByRole('list', { name: 'How each seat finished' })).getAllByRole('listitem')
    expect(rows.map((row) => row.querySelector('[data-slot="team-badge"]')?.textContent)).toEqual([
      'Team 1',
      'Team 1',
      'Team 2',
      'Team 2',
    ])
    expect(rows.map((row) => row.textContent.includes('Winner'))).toEqual([false, false, true, true])
  })

  it('gives every seat its time on its own turns, in figures and in words', async () => {
    await renderInRouter(shown(endedGame({ result: { Winner: 0 }, lost: { 1: 'ZeroLife' } })))
    const rows = within(screen.getByRole('list', { name: 'How each seat finished' })).getAllByRole('listitem')
    expect(rows[0]).toHaveTextContent('12:05 on own turns')
    expect(rows[0]).toHaveTextContent('Time used on own turns: 12 minutes 5 seconds')
    expect(rows[1]).toHaveTextContent('1:02:05 on own turns')
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
  it('offers a new game, the replay and the Play page to a guest, and nothing about My games', async () => {
    await renderInRouter(shown(endedGame({ result: { Winner: 0 }, lost: { 1: 'ZeroLife' } })))
    const links = screen
      .getAllByRole('link')
      .filter((link) => link.closest('main, [data-slot="page"]') !== null)
    expect(links.map((link) => [link.textContent, link.getAttribute('href')])).toEqual([
      ['Create another game', '/new'],
      ['Watch the replay', '/replay/over'],
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
    ).toEqual(['Create another game', 'Watch the replay', 'Back to Play', 'My games'])
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
