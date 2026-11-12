// Goal: a lobby with teams names its mode and each seat's team in words and colour; the host moves a seat to another team through PUT /teams with the seat's token (Two-Headed Giant swaps teammates so both teams keep two, Teams renumbers an emptied team), sees the new teams at once and the roster's word after, refuses teams the server would refuse without sending them, puts the old teams back with the lobby's words on a 400 or 403 and stops on a 409; other seats see the teams only, and a free-for-all lobby shows none.
import { screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { clearFlash } from '@/lib/flash'
import type { Teams } from '@/protocol/generated'
import { tokenOf } from '@/test-utils/fakeServer'
import { GAME_ID } from '@/test-utils/gameHarness'
import { changesOf, FAILURES, held, openLobby, seatItems } from '@/test-utils/lobbyControls'
import { lobbyMessage } from '@/test-utils/lobbyFrames'
import type { SeatFlags } from '@/test-utils/lobbyFrames'

afterEach(clearFlash)

const NAMES = ['Ana', 'Bo', 'Cy', 'Di']
const GIANT: Teams = { mode: 'two_headed_giant', seats: [0, 0, 1, 1] }
const ALTERNATE: Teams = { mode: 'teams', seats: [0, 1, 0, 1] }

const teamsShown = (): (string | null)[] =>
  seatItems().map((item) => {
    const select = item.querySelector('select')
    if (select !== null) return select.selectedOptions[0]?.textContent ?? null
    return item.querySelector('[data-slot="team-badge"]')?.textContent ?? null
  })
const select = (name: string): HTMLSelectElement => screen.getByLabelText(`Team of ${name}`)
const problem = (): Element | null => document.querySelector('[data-slot="teams-problem"]')
const note = (): Element | null => document.querySelector('[data-slot="teams-note"]')

describe('what every seat sees', () => {
  it.each<[string, Teams, string[]]>([
    ['Two-Headed Giant', GIANT, ['Team 1', 'Team 1', 'Team 2', 'Team 2']],
    ['Teams', ALTERNATE, ['Team 1', 'Team 2', 'Team 1', 'Team 2']],
  ])('names the mode %s and the team of every seat, read only for a player', async (mode, teams, shown) => {
    await openLobby({ seat: 1, names: NAMES, teams })
    expect(document.querySelector('[data-slot="lobby-mode"]')).toHaveTextContent(mode)
    expect(teamsShown()).toEqual(shown)
    expect(seatItems().map((item) => item.dataset.team)).toEqual(teams.seats.map(String))
    expect(document.querySelector('[data-team-select]')).toBeNull()
  })

  it('shows no mode and no teams in a free-for-all lobby', async () => {
    await openLobby({ names: NAMES })
    expect(document.querySelector('[data-slot="lobby-mode"]')).toBeNull()
    expect(teamsShown()).toEqual([null, null, null, null])
  })

  it('follows the teams of the latest roster', async () => {
    const h = await openLobby({ seat: 1, names: NAMES, teams: GIANT })
    const roster = lobbyMessage(NAMES.map((name): SeatFlags => [name, true, false]))
    h.deliver({ ...roster, teams: ALTERNATE })
    await h.settle()
    expect(teamsShown()).toEqual(['Team 1', 'Team 2', 'Team 1', 'Team 2'])
  })
})

describe('the host changes a team', () => {
  it.each<[string, Teams, string, number, number[]]>([
    ['swaps teammates in Two-Headed Giant', GIANT, 'Bo', 1, [0, 1, 0, 1]],
    ['moves a seat in Teams', { mode: 'teams', seats: [0, 1, 0, 1] }, 'Cy', 1, [0, 1, 1, 1]],
    ['makes a new team in Teams', ALTERNATE, 'Di', 2, [0, 1, 0, 2]],
  ])('%s, sent with the seat token and shown at once', async (_what, teams, name, team, seats) => {
    const h = await openLobby({ names: NAMES, teams })
    await h.user.selectOptions(select(name), `Team ${team + 1}`)
    await h.settle()
    expect(changesOf(h.server)).toEqual([
      {
        method: 'PUT',
        path: `/api/games/${GAME_ID}/teams`,
        authorization: `Bearer ${tokenOf(0)}`,
        body: { teams: { mode: teams.mode, seats } },
      },
    ])
    expect(teamsShown()).toEqual(seats.map((each) => `Team ${each + 1}`))
    expect(note()).toHaveTextContent(`${name} is now in Team ${team + 1}. Everyone presses Ready again.`)
    expect(problem()).toBeNull()
  })

  it('waits with every control while the request is out', async () => {
    const gate = held()
    const h = await openLobby({ names: NAMES, teams: GIANT, change: gate.answer })
    await h.user.selectOptions(select('Bo'), 'Team 2')
    expect(note()).toHaveTextContent('Changing the teams…')
    expect(select('Cy')).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Close game' })).toBeDisabled()
    gate.release(new Response(JSON.stringify({ teams: { mode: 'two_headed_giant', seats: [0, 1, 0, 1] } })))
    await h.settle()
    expect(select('Cy')).toBeEnabled()
  })

  it('does not send teams that leave only one team', async () => {
    const h = await openLobby({
      names: ['Ana', 'Bo', 'Cy', 'Di'],
      teams: { mode: 'teams', seats: [0, 0, 0, 1] },
    })
    await h.user.selectOptions(select('Di'), 'Team 1')
    expect(changesOf(h.server)).toEqual([])
    expect(problem()).toHaveTextContent('The teams stay as they are. A team game needs at least two teams.')
    expect(teamsShown()).toEqual(['Team 1', 'Team 1', 'Team 1', 'Team 2'])
  })

  it.each([
    ['teams that do not fit (400)', FAILURES.teams, 'Those teams do not fit this table'],
    ['a player who is not the host (403)', FAILURES.forbidden, 'Only the host can change the teams.'],
  ])('puts the teams back on %s, with the reason', async (_what, change, words) => {
    const h = await openLobby({ names: NAMES, teams: GIANT, change })
    await h.user.selectOptions(select('Bo'), 'Team 2')
    await h.settle()
    expect(problem()).toHaveTextContent(`The teams stay as they were. ${words}`)
    expect(teamsShown()).toEqual(['Team 1', 'Team 1', 'Team 2', 'Team 2'])
  })

  it('takes the controls away when the game has started (409)', async () => {
    const h = await openLobby({ names: NAMES, teams: GIANT, change: FAILURES.started })
    await h.user.selectOptions(select('Bo'), 'Team 2')
    await h.settle()
    expect(document.querySelector('[data-slot="lobby-stopped"]')).toHaveTextContent(
      'The game has already started, so the teams can no longer change.',
    )
    expect(document.querySelector('[data-team-select]')).toBeNull()
    expect(teamsShown()).toEqual(['Team 1', 'Team 1', 'Team 2', 'Team 2'])
  })
})
