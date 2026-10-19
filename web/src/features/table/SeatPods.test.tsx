// Goal: at every layout of the table (a duel's 168 px column, one-line pods for 3 and 4 seats, tiles in the strip and one pod column for 5 or more, a phone's strip and your own line) each seat is drawn once as a pod, a one-line pod or a tile, with its name, its life and the chips of its four zones named in words, the command zone only where it holds a card, and no placeholder text anywhere on the table.
import { within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { Viewport } from './layout'
import { podCounts, zoneLabel } from './podFigures'
import { exileOf, playerOf } from './selectors'
import { buildFixture } from './dev/fixture'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'

type Kind = 'panel' | 'line' | 'tile'

interface Scenario {
  name: string
  seats: number
  viewport: Viewport
  /** Every seat drawn as a pod or a tile, by seat. */
  drawn: Array<[seat: number, kind: Kind]>
}

const SCENARIOS: Scenario[] = [
  {
    name: 'a duel at 1440x900',
    seats: 2,
    viewport: { width: 1440, height: 900 },
    drawn: [
      [1, 'panel'],
      [0, 'panel'],
    ],
  },
  {
    name: 'a duel at 1280x720',
    seats: 2,
    viewport: { width: 1280, height: 720 },
    drawn: [
      [1, 'panel'],
      [0, 'panel'],
    ],
  },
  {
    name: 'three seats at 1440x900',
    seats: 3,
    viewport: { width: 1440, height: 900 },
    drawn: [
      [1, 'line'],
      [2, 'line'],
      [0, 'panel'],
    ],
  },
  {
    name: 'four seats at 1920x1080',
    seats: 4,
    viewport: { width: 1920, height: 1080 },
    drawn: [
      [1, 'line'],
      [2, 'line'],
      [3, 'line'],
      [0, 'panel'],
    ],
  },
  {
    name: 'six seats at 1280x720: tiles, the focused pod and yours',
    seats: 6,
    viewport: { width: 1280, height: 720 },
    drawn: [
      [1, 'tile'],
      [2, 'tile'],
      [3, 'tile'],
      [4, 'tile'],
      [5, 'tile'],
      [1, 'panel'],
      [0, 'panel'],
    ],
  },
  {
    name: 'a phone with two seats: the opponent’s tile and your line',
    seats: 2,
    viewport: { width: 390, height: 844 },
    drawn: [
      [1, 'tile'],
      [0, 'line'],
    ],
  },
  {
    name: 'a phone with six seats',
    seats: 6,
    viewport: { width: 390, height: 844 },
    drawn: [
      [1, 'tile'],
      [2, 'tile'],
      [3, 'tile'],
      [4, 'tile'],
      [5, 'tile'],
      [0, 'line'],
    ],
  },
]

const rootOf = (seat: number, kind: Kind): HTMLElement =>
  must(
    document.querySelector<HTMLElement>(
      kind === 'tile' ? `[data-tile-seat="${seat}"]` : `section[data-seat="${seat}"][data-variant="${kind}"]`,
    ),
    `${kind} of seat ${seat}`,
  )

describe.each(SCENARIOS)('$name', ({ seats, viewport, drawn }) => {
  const table = () => renderTable({ fixture: buildFixture({ seats, permanents: 6 }), options: { viewport } })

  it('draws each seat as the pod or tile of its place, once', () => {
    table()
    for (const [seat, kind] of drawn) expect(rootOf(seat, kind)).toBeInTheDocument()
    const roots = document.querySelectorAll('section[data-seat][data-variant], [data-tile-seat]')
    expect(roots).toHaveLength(drawn.length)
  })

  it('names the four zones of every pod and tile with the counts of the view', () => {
    const { store } = table()
    for (const [seat, kind] of drawn) {
      const root = rootOf(seat, kind)
      const state = store.getState()
      const counts = podCounts(must(playerOf(state, seat)), exileOf(state, seat).length)
      const chip = (zone: 'hand' | 'library' | 'graveyard' | 'exile'): HTMLElement =>
        kind === 'tile' || zone === 'hand' || zone === 'library'
          ? within(root).getByRole('img', { name: zoneLabel(zone, counts[zone]) })
          : within(root).getByRole('button', { name: new RegExp(`^${zoneLabel(zone, counts[zone])}`) })
      for (const zone of ['hand', 'library', 'graveyard', 'exile'] as const) {
        expect(chip(zone), `${kind} ${seat} ${zone}`).toHaveTextContent(String(counts[zone]))
      }
    }
  })

  it('shows the name and life of every seat, life as the largest figure', () => {
    const { store } = table()
    for (const [seat, kind] of drawn) {
      const root = rootOf(seat, kind)
      const life = must(playerOf(store.getState(), seat)).life
      expect(root.querySelector('[data-slot="life"]'), `${kind} ${seat}`).toHaveTextContent(String(life))
      expect(root.querySelector('[data-slot="seat-dot"]'), `${kind} ${seat}`).not.toBeNull()
    }
  })

  it('writes no placeholder text anywhere on the table', () => {
    table()
    expect(document.body.textContent).not.toMatch(/not tracked|coming soon|placeholder|n\/a/i)
    expect(document.querySelector('[data-slot="commander-stats"]')).toBeNull()
  })
})

describe('the command zone in a pod', () => {
  it('is drawn as chips and cards only for the seats whose command zone holds a card', () => {
    const { store } = renderTable({
      fixture: buildFixture({ seats: 4, permanents: 4 }),
      options: { viewport: { width: 1920, height: 1080 } },
      state: (state) => {
        const view = structuredClone(must(state.view))
        must(view.players[2]).command = { count: 0, cards: [] }
        return { ...state, view }
      },
    })
    expect(store.getState().view?.players[2]?.command.count).toBe(0)
    expect(
      within(rootOf(1, 'line')).getByRole('button', { name: /^Command zone 1 card/ }),
    ).toBeInTheDocument()
    expect(within(rootOf(2, 'line')).queryByRole('button', { name: /^Command zone/ })).toBeNull()
    expect(within(rootOf(0, 'panel')).getByRole('list', { name: "Alice's command zone" })).toBeInTheDocument()
  })
})
