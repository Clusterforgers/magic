// Goal: the Play page lists the open games as a table of host, format, seats, decks, age and a Join named after the game, counts them in a polite status, dims and explains a full game, waits 300 ms before it draws a skeleton, shows an empty list with both ways forward, keeps the old rows dimmed beside an error with Retry, filters a long list, and keeps the join code field disabled and explained.
import { screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { installFakeClock, removeFakeClock, settle } from '@/test-utils/fakeClock'
import { LobbyError } from '@/net/lobbyErrors'
import { fakeLobbies, lobbyRow, lobbyRows } from '@/test-utils/lobbies'
import type { FakeLobbies } from '@/test-utils/lobbies'
import { renderInRouter } from '@/test-utils/router'
import { CODE_NOTE } from './JoinWithCode'
import { PlayPage } from './PlayPage'

beforeEach(installFakeClock)
afterEach(removeFakeClock)

const user = () => userEvent.setup({ delay: null })
const codeField = () => screen.getByRole('textbox', { name: 'Join with a code' })

async function open(lobbies: FakeLobbies) {
  const view = await renderInRouter(<PlayPage api={lobbies.api} />)
  await settle()
  return { ...view, user: user() }
}

const table = () => screen.getByRole('table', { name: 'Open games, newest first' })
const rowOf = (host: string) => within(table()).getByRole('row', { name: new RegExp(`^${host}\\b`) })

describe('the open games', () => {
  it('is a table with a column for each fact and a row for each game, newest first', async () => {
    const lobbies = fakeLobbies([
      lobbyRow({ game: 'a', host: 'Bo', filled: 2, total: 4, age_secs: 180 }),
      lobbyRow({ game: 'b', host: 'Cy', filled: 1, total: 2, age_secs: 45 }),
    ])
    await open(lobbies)
    expect(
      within(table())
        .getAllByRole('columnheader')
        .map((cell) => cell.textContent),
    ).toEqual(['Host', 'Format', 'Seats', 'Decks', 'Age', 'Action'])
    const bo = within(rowOf('Bo'))
    expect(bo.getByRole('rowheader')).toHaveTextContent('Bo')
    expect(bo.getAllByRole('cell').map((cell) => cell.textContent)).toEqual([
      'Normal',
      '2 of 4',
      'Your own deck',
      '3 min ago',
      'Join',
    ])
    expect(within(rowOf('Cy')).getByText('just now')).toBeInTheDocument()
    expect(within(table()).getAllByRole('row').slice(1)).toHaveLength(2)
  })

  it('puts the creation stamp in a time element', async () => {
    await open(fakeLobbies([lobbyRow({ created_at: '2026-10-19T12:30:00Z' })]))
    expect(within(table()).getByText('3 min ago')).toHaveAttribute('datetime', '2026-10-19T12:30:00Z')
    expect(within(table()).getByText('3 min ago').tagName).toBe('TIME')
  })

  it("names each Join after the game's host and its seats, and leaves it enabled", async () => {
    const rows = [
      lobbyRow({ game: 'a', host: 'Bo', filled: 2, total: 4 }),
      lobbyRow({ game: 'b', host: 'Cy', filled: 1, total: 2 }),
    ]
    await open(fakeLobbies(rows))
    expect(screen.getByRole('button', { name: "Join Cy's game, 1 of 2 seats" })).toBeEnabled()
    expect(screen.getByRole('button', { name: "Join Bo's game, 2 of 4 seats" })).toBeEnabled()
    expect(screen.queryByText(/next piece/)).not.toBeInTheDocument()
  })

  it('counts the games in a polite status that is in the page before any game is', async () => {
    const lobbies = fakeLobbies(lobbyRows(3))
    const release = lobbies.hold()
    await renderInRouter(<PlayPage api={lobbies.api} />)
    const status = screen.getAllByRole('status').find((node) => node.tagName === 'P')
    expect(status).toHaveTextContent('')
    release()
    await settle()
    expect(status).toHaveTextContent('3 open games')
  })

  it.each([
    [1, '1 open game'],
    [2, '2 open games'],
  ])('says %i game as "%s"', async (count, words) => {
    await open(fakeLobbies(lobbyRows(count)))
    expect(screen.getByText(words)).toBeInTheDocument()
  })

  it('dims a full game, says Full, and keeps its Join in place, disabled and explained', async () => {
    const lobbies = fakeLobbies([
      lobbyRow({ game: 'a', host: 'Bo', filled: 4, total: 4 }),
      lobbyRow({ game: 'b', host: 'Cy', filled: 1, total: 4 }),
    ])
    const { user } = await open(lobbies)
    const row = within(rowOf('Bo'))
    expect(row.getByText('Full')).toBeInTheDocument()
    expect(row.getByRole('rowheader')).toHaveClass('text-muted-foreground')
    expect(within(rowOf('Cy')).getByRole('rowheader')).not.toHaveClass('text-muted-foreground')
    const button = row.getByRole('button', { name: "Join Bo's game, 4 of 4 seats" })
    expect(button).toHaveAttribute('aria-disabled', 'true')
    expect(button).toHaveAccessibleDescription('Full')
    button.focus()
    expect(button).toHaveFocus()
    await user.click(button)
    expect(lobbies.joinGame).not.toHaveBeenCalled()
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    expect(within(rowOf('Cy')).queryByText('Full')).not.toBeInTheDocument()
  })

  it('draws a dot for each seat, filled where somebody sits, and hides them from a screen reader', async () => {
    await open(fakeLobbies([lobbyRow({ filled: 2, total: 4 })]))
    const dots = within(table()).getByText('2 of 4').previousElementSibling
    expect(dots).toHaveAttribute('aria-hidden', 'true')
    expect(dots?.children).toHaveLength(4)
    expect([...(dots?.children ?? [])].map((dot) => dot.className.includes('bg-foreground'))).toEqual([
      true,
      true,
      false,
      false,
    ])
  })

  it('brings the ages up to date every 30 s and counts them on from when the rows arrived', async () => {
    await open(fakeLobbies([lobbyRow({ age_secs: 50 })]))
    expect(within(table()).getByText('just now')).toBeInTheDocument()
    await settle(29_000)
    expect(within(table()).getByText('just now')).toBeInTheDocument()
    await settle(1_000)
    expect(within(table()).getByText('1 min ago')).toBeInTheDocument()
    await settle(60_000)
    expect(within(table()).getByText('2 min ago')).toBeInTheDocument()
  })
})

describe('the code form', () => {
  it('is disabled and says why, while every row can be joined', async () => {
    const lobbies = fakeLobbies(lobbyRows(2))
    await open(lobbies)
    expect(screen.getByText(CODE_NOTE)).toBeVisible()
    const button = within(screen.getByRole('form', { name: 'Join with a code' })).getByRole('button', {
      name: 'Join',
    })
    expect(button).toHaveAttribute('aria-disabled', 'true')
    expect(button).toHaveAccessibleDescription(CODE_NOTE)
    for (const join of within(table()).getAllByRole('button')) {
      expect(join).toBeEnabled()
      expect(join).not.toHaveAttribute('aria-disabled', 'true')
    }
  })

  it('sends nothing when a code is typed and Enter is pressed', async () => {
    const lobbies = fakeLobbies([])
    const { user } = await open(lobbies)
    await user.type(codeField(), 'K7M29QXA{Enter}')
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    expect(lobbies.joinGame).not.toHaveBeenCalled()
  })
})

describe('waiting', () => {
  it('says it is loading at once and draws the skeleton only after 300 ms', async () => {
    const lobbies = fakeLobbies(lobbyRows(2))
    const release = lobbies.hold()
    await renderInRouter(<PlayPage api={lobbies.api} />)
    expect(screen.getByText('Loading the open games…')).toBeInTheDocument()
    const skeletons = () => document.querySelectorAll('[data-slot="skeleton"]')
    await settle(299)
    expect(skeletons()).toHaveLength(0)
    await settle(1)
    expect(skeletons().length).toBeGreaterThan(0)
    release()
    await settle()
    expect(skeletons()).toHaveLength(0)
    expect(table()).toBeInTheDocument()
  })

  it('never draws the skeleton for an answer that comes within 300 ms', async () => {
    await open(fakeLobbies(lobbyRows(2)))
    expect(document.querySelectorAll('[data-slot="skeleton"]')).toHaveLength(0)
    await settle(1_000)
    expect(document.querySelectorAll('[data-slot="skeleton"]')).toHaveLength(0)
  })
})

describe('an empty list', () => {
  it('says so, offers Create game and the code field, and counts none', async () => {
    const { user } = await open(fakeLobbies([]))
    expect(screen.getByRole('heading', { name: 'No open games' })).toBeInTheDocument()
    expect(screen.getByText('Create one, or join with a code.')).toBeInTheDocument()
    expect(screen.queryByRole('table')).not.toBeInTheDocument()
    expect(screen.getByRole('status')).toHaveTextContent('0 open games')
    const links = screen.getAllByRole('link', { name: 'Create game' })
    expect(links.map((link) => link.getAttribute('href'))).toEqual(['/new', '/new'])
    await user.click(screen.getByRole('button', { name: 'Join with a code' }))
    expect(codeField()).toHaveFocus()
  })

  it('gives way to the rows when a game appears', async () => {
    const lobbies = fakeLobbies([])
    await open(lobbies)
    lobbies.set(lobbyRows(1))
    await settle(5_000)
    expect(screen.queryByRole('heading', { name: 'No open games' })).not.toBeInTheDocument()
    expect(table()).toBeInTheDocument()
  })
})

describe('an error', () => {
  it('on the first load shows an alert with the reason and Retry, and no rows', async () => {
    const lobbies = fakeLobbies(lobbyRows(2))
    lobbies.fail(new LobbyError('lobbies_unavailable', 503, 'busy'))
    const { user } = await open(lobbies)
    const alert = screen.getByRole('alert')
    expect(alert).toHaveTextContent('Could not load the open games')
    expect(alert).toHaveTextContent('The server cannot list its games right now. Try again in a moment.')
    expect(screen.queryByRole('table')).not.toBeInTheDocument()
    expect(screen.queryByRole('heading', { name: 'No open games' })).not.toBeInTheDocument()
    lobbies.heal()
    await user.click(within(alert).getByRole('button', { name: 'Retry' }))
    await settle()
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    expect(table()).toBeInTheDocument()
  })

  it('after rows keeps them, dimmed, beside an alert with Retry, and restores them when it works again', async () => {
    const lobbies = fakeLobbies([lobbyRow({ game: 'a', host: 'Bo' })])
    const { user } = await open(lobbies)
    const region = screen.getByRole('region', { name: 'Open games' })
    expect(region).not.toHaveAttribute('data-stale')
    expect(within(table()).getByRole('rowheader')).not.toHaveClass('text-muted-foreground')
    lobbies.fail(new LobbyError('network', 0, 'down'))
    await settle(5_000)
    const alert = screen.getByRole('alert')
    expect(alert).toHaveTextContent('Could not refresh the open games')
    expect(alert).toHaveTextContent('Could not reach the server.')
    expect(alert).toHaveTextContent('The games below may be out of date.')
    expect(within(table()).getByText('Bo')).toBeInTheDocument()
    expect(region).toHaveAttribute('data-stale')
    expect(within(table()).getByRole('rowheader')).toHaveClass('text-muted-foreground')
    expect(screen.getByText('1 open game')).toBeInTheDocument()
    lobbies.heal()
    await user.click(within(alert).getByRole('button', { name: 'Retry' }))
    await settle()
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    expect(region).not.toHaveAttribute('data-stale')
  })

  it('words a rate limit with the wait the server asked for', async () => {
    const lobbies = fakeLobbies(lobbyRows(1))
    lobbies.fail(new LobbyError('rate_limited', 429, 'slow', 12))
    await open(lobbies)
    expect(screen.getByRole('alert')).toHaveTextContent('It can be asked for again in 12 seconds.')
  })
})

describe('Refresh', () => {
  it('asks again now, shows it is working and keeps its place', async () => {
    const lobbies = fakeLobbies(lobbyRows(1))
    const { user } = await open(lobbies)
    const button = screen.getByRole('button', { name: 'Refresh' })
    expect(button).not.toHaveAttribute('aria-busy')
    const release = lobbies.hold()
    await user.click(button)
    expect(lobbies.listLobbies).toHaveBeenCalledTimes(2)
    expect(button).toHaveAttribute('aria-busy', 'true')
    expect(button).toHaveFocus()
    release()
    await settle()
    expect(button).not.toHaveAttribute('aria-busy')
    expect(button).toHaveFocus()
  })
})

describe('filters', () => {
  const mixed = [
    lobbyRow({ game: 'a', host: 'A1', total: 2, filled: 1, format: 'normal' }),
    lobbyRow({ game: 'b', host: 'B1', total: 4, filled: 4, format: 'normal' }),
    lobbyRow({ game: 'c', host: 'C1', total: 4, filled: 2, format: 'commander' }),
    lobbyRow({ game: 'd', host: 'D1', total: 2, filled: 2, format: 'commander' }),
    lobbyRow({ game: 'e', host: 'E1', total: 6, filled: 3, format: 'normal' }),
    lobbyRow({ game: 'f', host: 'F1', total: 4, filled: 1, format: 'normal' }),
    lobbyRow({ game: 'g', host: 'G1', total: 4, filled: 3, format: 'normal' }),
    lobbyRow({ game: 'h', host: 'H1', total: 2, filled: 1, format: 'normal' }),
    lobbyRow({ game: 'i', host: 'I1', total: 4, filled: 2, format: 'normal' }),
  ]
  const group = () => screen.queryByRole('group', { name: 'Filter the open games' })
  const hosts = () =>
    within(table())
      .getAllByRole('rowheader')
      .map((cell) => cell.textContent)

  it('are not offered for 8 rows or fewer, and are for 9', async () => {
    await open(fakeLobbies(mixed.slice(0, 8)))
    expect(group()).not.toBeInTheDocument()
    await open(fakeLobbies(mixed))
    expect(group()).toBeInTheDocument()
  })

  it('narrow the rows by seats, by format and by "hide full", and say how many are left', async () => {
    const { user } = await open(fakeLobbies(mixed))
    expect(screen.getByText('9 open games')).toBeInTheDocument()

    await user.selectOptions(screen.getByLabelText('Seats'), '4')
    expect(hosts()).toEqual(['B1', 'C1', 'F1', 'G1', 'I1'])
    expect(screen.getByRole('status')).toHaveTextContent('5 of 9 open games')

    await user.selectOptions(screen.getByLabelText('Format'), 'normal')
    expect(hosts()).toEqual(['B1', 'F1', 'G1', 'I1'])

    await user.click(screen.getByRole('switch', { name: 'Hide full games' }))
    expect(hosts()).toEqual(['F1', 'G1', 'I1'])
    expect(screen.getByRole('status')).toHaveTextContent('3 of 9 open games')

    await user.click(screen.getByRole('button', { name: 'Clear filters' }))
    expect(hosts()).toHaveLength(9)
    expect(screen.getByRole('switch', { name: 'Hide full games' })).not.toBeChecked()
    expect(screen.queryByRole('button', { name: 'Clear filters' })).not.toBeInTheDocument()
  })

  it('offer the seat counts and formats that exist', async () => {
    await open(fakeLobbies(mixed))
    const options = (label: string) =>
      within(screen.getByLabelText(label))
        .getAllByRole('option')
        .map((option) => option.textContent)
    expect(options('Seats')).toEqual(['Any number', '2 seats', '4 seats', '6 seats'])
    expect(options('Format')).toEqual(['Any format', 'Commander', 'Normal'])
  })

  it('say when nothing matches, and the way back', async () => {
    const { user } = await open(fakeLobbies(mixed))
    await user.selectOptions(screen.getByLabelText('Seats'), '6')
    await user.selectOptions(screen.getByLabelText('Format'), 'commander')
    expect(screen.getByRole('heading', { name: 'No games match these filters' })).toBeInTheDocument()
    expect(screen.queryByRole('table')).not.toBeInTheDocument()
    await user.click(screen.getAllByRole('button', { name: 'Clear filters' })[0] as HTMLElement)
    expect(hosts()).toHaveLength(9)
  })

  it('stop applying once the list is short enough to have no filters in the page', async () => {
    const lobbies = fakeLobbies(mixed)
    const { user } = await open(lobbies)
    await user.selectOptions(screen.getByLabelText('Seats'), '6')
    expect(hosts()).toEqual(['E1'])
    lobbies.set(mixed.slice(0, 4))
    await settle(5_000)
    expect(group()).not.toBeInTheDocument()
    expect(hosts()).toEqual(['A1', 'B1', 'C1', 'D1'])
  })
})
