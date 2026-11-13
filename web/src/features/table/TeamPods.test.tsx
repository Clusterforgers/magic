// Goal: at a table with teams every pod and tile names its seat's team in words and colour, Two-Headed Giant shows the shared life total once per team (large on the team's first seat, small and marked shared on the teammate's) and says whom it is shared with, Teams keeps a life per seat, and a table without teams shows no team at all.
import { describe, expect, it } from 'vitest'
import type { PlayState } from '@/play/store'
import type { Teams } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from './dev/fixture'
import { tileName } from './podFigures'
import { seatTeam } from './teamLife'

const GIANT: Teams = { mode: 'two_headed_giant', seats: [0, 0, 1, 1] }
const ALTERNATE: Teams = { mode: 'teams', seats: [0, 1, 0, 1, 0, 1] }

function withTeams(teams: Teams | undefined) {
  return (state: PlayState): PlayState => {
    const view = structuredClone(must(state.view))
    if (teams === undefined) delete view.house_rules
    else view.house_rules = { teams }
    return { ...state, view }
  }
}

const root = (seat: number): HTMLElement =>
  must(document.querySelector<HTMLElement>(`[data-seat="${seat}"][data-variant], [data-tile-seat="${seat}"]`))
const badge = (seat: number): string | null =>
  root(seat).querySelector('[data-slot="team-badge"]')?.textContent ?? null
const shared = (seat: number): string | null =>
  root(seat).querySelector<HTMLElement>('[data-shared-life]')?.dataset.sharedLife ?? null

describe('pods at a table with teams', () => {
  it('names the teams of Two-Headed Giant and shows the shared life large once per team', () => {
    renderTable({
      fixture: buildFixture({ seats: 4, permanents: 2 }),
      options: { viewport: { width: 1920, height: 1080 } },
      state: withTeams(GIANT),
    })
    expect([0, 1, 2, 3].map(badge)).toEqual(['Team 1', 'Team 1', 'Team 2', 'Team 2'])
    expect([0, 1, 2, 3].map((seat) => root(seat).dataset.team)).toEqual(['0', '0', '1', '1'])
    expect([0, 1, 2, 3].map(shared)).toEqual(['team', 'teammate', 'team', 'teammate'])
    expect(root(1).querySelector('[data-slot="life"]')).toHaveTextContent(/shared/)
    expect(root(0).querySelector('[data-slot="life"]')).not.toHaveTextContent(/^shared/)
    expect(root(2).querySelector('[data-slot="life"]')).toHaveTextContent(/Team life, shared with/)
  })

  it.each<[string, Teams | undefined, (string | null)[]]>([
    ['Teams', ALTERNATE, ['Team 1', 'Team 2', 'Team 1', 'Team 2', 'Team 1', 'Team 2']],
    ['no teams', undefined, [null, null, null, null, null, null]],
  ])('with %s keeps a life of its own on every seat', (_what, teams, badges) => {
    renderTable({
      fixture: buildFixture({ seats: 6, permanents: 2 }),
      options: { viewport: { width: 1920, height: 1080 } },
      state: withTeams(teams),
    })
    expect([0, 1, 2, 3, 4, 5].map(badge)).toEqual(badges)
    expect([0, 1, 2, 3, 4, 5].map(shared)).toEqual([null, null, null, null, null, null])
  })
})

describe('the words of a seat with a team', () => {
  const name = (seat: number): string => ['Ann', 'Bo', 'Cy', 'Di'][seat] ?? '?'

  it.each<[Teams | null, number, ReturnType<typeof seatTeam>]>([
    [GIANT, 0, { team: 0, life: 'team', mates: ['Bo'], badge: true }],
    [GIANT, 1, { team: 0, life: 'teammate', mates: ['Ann'], badge: true }],
    [GIANT, 3, { team: 1, life: 'teammate', mates: ['Cy'], badge: true }],
    [ALTERNATE, 1, { team: 1, life: 'own', mates: ['Di', '?'], badge: true }],
    [null, 1, null],
  ])('reads %j for seat %i', (teams, seat, expected) => {
    expect(seatTeam(teams, seat, name)).toEqual(expected)
  })

  it("names the team and the shared life in a tile's accessible name", () => {
    const counts = { hand: 1, library: 2, graveyard: 0, exile: 0, command: 0, attractions: 0 }
    const facts = {
      name: 'Bo',
      life: 30,
      counts,
      active: false,
      priority: false,
      eliminated: false,
      focused: false,
    }
    expect(tileName({ ...facts, team: { label: 'Team 1', sharedWith: 'Ann' } })).toBe(
      "Show Bo's board. Team 1, Life 30, shared with Ann, 1 card in hand, 2 in library, 0 in graveyard, 0 in exile",
    )
    expect(tileName({ ...facts, team: { label: 'Team 2' } })).toMatch(/^Show Bo's board\. Team 2, Life 30,/)
  })
})
