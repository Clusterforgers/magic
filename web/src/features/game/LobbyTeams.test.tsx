// Goal: a lobby with teams names its mode and shows one column per team a seat may choose, each seat with its team in words and colour; the host moves any seat and a player their own through PUT /seats/{n}/team with the seat's token, the server keeps exactly that choice (no swaps, no renumbering), teams that cannot start yet are sent and named as a note, a 400 or 403 puts the old teams back with the lobby's words and a 409 stops the lobby; a free-for-all lobby shows no teams.
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
const columns = (): string[] =>
  Array.from(document.querySelectorAll<HTMLElement>('[data-team-column]')).map(
    (column) => column.querySelector('header')?.textContent ?? '',
  )
const select = (name: string): HTMLSelectElement => screen.getByLabelText(`Team of ${name}`)
const problem = (): Element | null => document.querySelector('[data-slot="teams-problem"]')
const unready = (): Element | null => document.querySelector('[data-slot="teams-unready"]')
const note = (): Element | null => document.querySelector('[data-slot="teams-note"]')

describe('what every seat sees', () => {
  it.each<[string, Teams, string[], string[]]>([
    [
      'Two-Headed Giant',
      GIANT,
      ['Team 1', 'Team 1', 'Team 2', 'Team 2'],
      ['Team 1Ana & Bo', 'Team 2Cy & Di'],
    ],
    ['Teams', ALTERNATE, ['Team 1', 'Team 2', 'Team 1', 'Team 2'], ['Team 1Ana & Cy', 'Team 2Bo & Di']],
  ])('names the mode %s, a column per team and the team of every seat', async (mode, teams, shown, heads) => {
    await openLobby({ seat: 1, names: NAMES, teams })
    expect(document.querySelector('[data-slot="lobby-mode"]')).toHaveTextContent(mode)
    expect(teamsShown()).toEqual(shown)
    expect(columns()).toEqual(heads)
    expect(seatItems().map((item) => item.dataset.team)).toEqual(teams.seats.map(String))
    // A player chooses the team of their own seat only.
    expect(document.querySelectorAll('[data-team-select]')).toHaveLength(1)
    expect(select('Bo')).toBeEnabled()
  })

  it('shows an empty column for a team nobody chose yet', async () => {
    await openLobby({ names: NAMES, teams: { mode: 'two_headed_giant', seats: [0, 0, 0, 0] } })
    expect(columns()).toEqual(['Team 1Ana, Bo, Cy & Di', 'Team 2Nobody yet'])
    expect(unready()).toHaveTextContent('Before the game can start: A team game needs at least two teams.')
  })

  it('shows no mode and no teams in a free-for-all lobby', async () => {
    await openLobby({ names: NAMES })
    expect(document.querySelector('[data-slot="lobby-mode"]')).toBeNull()
    expect(document.querySelector('[data-slot="team-columns"]')).toBeNull()
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

describe('choosing a team', () => {
  it.each<[string, number, Teams, string, number, number[], string | null]>([
    [
      'the host moves a seat in Two-Headed Giant, kept without a swap',
      0,
      GIANT,
      'Bo',
      1,
      [0, 1, 1, 1],
      'Two-Headed Giant needs two teams of two.',
    ],
    ['the host moves a seat in Teams', 0, ALTERNATE, 'Cy', 1, [0, 1, 1, 1], null],
    [
      'a player moves their own seat',
      2,
      GIANT,
      'Cy',
      0,
      [0, 0, 0, 1],
      'Two-Headed Giant needs two teams of two.',
    ],
    [
      'a player leaves one team, sent all the same',
      3,
      { mode: 'teams', seats: [0, 0, 0, 1] },
      'Di',
      0,
      [0, 0, 0, 0],
      'A team game needs at least two teams.',
    ],
  ])(
    '%s, sent with the seat token and shown at once',
    async (_what, seat, teams, name, team, seats, waiting) => {
      const h = await openLobby({ seat, names: NAMES, teams })
      const moved = NAMES.indexOf(name)
      await h.user.selectOptions(select(name), `Team ${team + 1}`)
      await h.settle()
      expect(changesOf(h.server)).toEqual([
        {
          method: 'PUT',
          path: `/api/games/${GAME_ID}/seats/${moved}/team`,
          authorization: `Bearer ${tokenOf(seat)}`,
          body: { team },
        },
      ])
      expect(teamsShown()).toEqual(seats.map((each) => `Team ${each + 1}`))
      expect(note()).toHaveTextContent(`${name} is now in Team ${team + 1}. Everyone presses Ready again.`)
      expect(problem()).toBeNull()
      if (waiting === null) expect(unready()).toBeNull()
      else expect(unready()).toHaveTextContent(`Before the game can start: ${waiting}`)
    },
  )

  it('waits with every control while the request is out', async () => {
    const gate = held()
    const h = await openLobby({ names: NAMES, teams: GIANT, change: gate.answer })
    await h.user.selectOptions(select('Bo'), 'Team 2')
    expect(note()).toHaveTextContent('Changing the teams…')
    expect(select('Cy')).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Close game' })).toBeDisabled()
    gate.release(new Response(JSON.stringify({ teams: { mode: 'two_headed_giant', seats: [0, 1, 1, 1] } })))
    await h.settle()
    expect(select('Cy')).toBeEnabled()
  })

  it.each([
    ['teams that do not fit (400)', FAILURES.teams, 'Those teams do not fit this table'],
    ['a player who may not move that seat (403)', FAILURES.forbidden, 'Only the host can change the teams.'],
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
