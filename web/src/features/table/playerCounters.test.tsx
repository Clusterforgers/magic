// Goal: every kind of counter on a player is a chip at its seat, an icon for rad, poison, energy, experience and tickets and a short label for any other kind, with its count and words, in the panel pod, the one-line pod and the tile, for any seat; a kind at 0 or a seat without counters draws nothing; and `?counters=1` puts counters on every seat.
import { screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import type { PlayState } from '@/play/store'
import type { CounterKind, StateView } from '@/protocol/generated'
import { parseDevSearch } from './devSearch'
import { withPlayerCounters } from './dev/countersScene'
import { buildFixture } from './dev/fixture'
import { playerCounterChips, playerCounterWords } from './playerCounters'
import { SeatPanel } from './SeatPanel'
import { SeatTile } from './SeatTile'
import { CombatProviders } from './combat/CombatProviders'

const KINDS: [string, CounterKind, string | null][] = [
  ['rad', { Other: 'rad' }, 'rad'],
  ['poison', 'poison', 'poison'],
  ['energy', { Other: 'energy' }, 'energy'],
  ['experience', { Other: 'experience' }, 'experience'],
  ['ticket', { Other: 'ticket' }, 'ticket'],
  ['acorn', { Other: 'acorn' }, null],
]

function edited(edit: (view: StateView) => void): (state: PlayState) => PlayState {
  return (state) => {
    const view = clone(must(state.view))
    edit(view)
    return { ...state, view }
  }
}

type Place = 'panel' | 'line' | 'tile'

function show(place: Place, seat: number, counters: [CounterKind, number][]) {
  const ui =
    place === 'tile' ? (
      <SeatTile seat={seat} focused={false} onFocusSeat={() => undefined} />
    ) : (
      <SeatPanel seat={seat} variant={place} />
    )
  return renderTable({
    fixture: buildFixture({ seats: 3, permanents: 6 }),
    ui: <CombatProviders>{ui}</CombatProviders>,
    state: edited((view) => {
      must(view.players[seat]).counters = counters
    }),
  })
}

const chipsShown = () => [...document.querySelectorAll('[data-slot="player-counter"]')]

describe('playerCounterChips', () => {
  it.each(KINDS)('gives %s its icon or a short label', (name, kind, icon) => {
    const [chip] = playerCounterChips({ counters: [[kind, 4]] })
    expect(chip).toEqual({
      key: name,
      icon,
      short: name.charAt(0).toUpperCase() + name.slice(1, 4),
      count: 4,
      words: `4 ${name} counters`,
    })
  })

  it.each([
    [1, '1 rad counter'],
    [2, '2 rad counters'],
    [12, '12 rad counters'],
  ])('says %s counters', (count, words) => {
    expect(playerCounterWords('rad', count)).toBe(words)
  })

  it('keeps the order of the view and leaves out a kind at 0 and a seat without counters', () => {
    const chips = playerCounterChips({
      counters: [
        ['poison', 2],
        [{ Other: 'energy' }, 0],
        [{ Other: 'rad' }, 5],
      ],
    })
    expect(chips.map((chip) => chip.key)).toEqual(['poison', 'rad'])
    expect(playerCounterChips({})).toEqual([])
  })
})

describe('the counter chips at a seat', () => {
  const places: Place[] = ['panel', 'line', 'tile']
  const cases = places.flatMap((place) =>
    [0, 1, 2].flatMap((seat) => KINDS.map((kind) => [place, seat, ...kind] as const)),
  )

  it.each(cases)('the %s of seat %s shows %s with its count', (place, seat, name, kind) => {
    show(place, seat, [[kind, 7]])
    const [chip, ...rest] = chipsShown()
    expect(rest).toEqual([])
    expect(chip).toHaveAttribute('data-kind', name)
    expect(chip).toHaveTextContent('7')
    expect(chip?.getAttribute('aria-label')).toMatch(new RegExp(`: 7 ${name} counters$`))
    if (place === 'tile') {
      expect(screen.getByRole('button', { name: new RegExp(`7 ${name} counters`) })).toBeInTheDocument()
    } else {
      expect(screen.getByRole('img', { name: new RegExp(`: 7 ${name} counters$`) })).toBe(chip)
    }
  })

  it.each(places)('the %s shows every kind a seat has, one chip each', (place) => {
    show(place, 1, [
      ['poison', 9],
      [{ Other: 'rad' }, 12],
      [{ Other: 'energy' }, 1],
    ])
    expect(chipsShown().map((chip) => chip.getAttribute('data-kind'))).toEqual(['poison', 'rad', 'energy'])
    expect(chipsShown().map((chip) => chip.textContent)).toEqual(['9', '12', '1'])
  })

  it.each(places)('the %s of a seat without counters draws no chip', (place) => {
    show(place, 1, [])
    expect(chipsShown()).toEqual([])
  })
})

describe('the counters scene', () => {
  it.each([
    ['1', true],
    [1, true],
    [true, true],
    ['yes', undefined],
    [undefined, undefined],
  ])('reads counters=%s as %s', (value, parsed) => {
    expect(parseDevSearch({ counters: value }).counters).toBe(parsed)
  })

  it.each([2, 4, 12])('puts counters on all %s seats, rad among them on every opponent', (seats) => {
    const scene = withPlayerCounters(buildFixture({ seats }))
    for (const player of scene.view.players) {
      const kinds = playerCounterChips(player).map((chip) => chip.key)
      expect(kinds.length).toBeGreaterThan(0)
      if (player.seat !== scene.view.viewer) expect(kinds).toContain('rad')
    }
  })
})
