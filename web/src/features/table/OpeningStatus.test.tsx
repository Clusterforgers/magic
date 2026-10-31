// Goal: during the opening hands the table lists every seat with its public status (deciding, kept, taking a mulligan, how many mulligans taken), whatever the table size or layout, names this seat "You", follows the statuses as batches change them, and is not there before or after that stage.
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { buildFixture } from '@/features/table/dev/fixture'
import type { PlayState } from '@/play/store'
import type { MulliganStatus, StateView } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import type { TableSetup } from '@/test-utils/tableHarness'
import { KINDS } from '@/test-utils/decisions'
import { describeOpening } from './turnText'

/** This seat still deciding: every seat is asked at once, so it reads like the others. */
const YOU_WAIT = 'You: deciding'

type Each = (seat: number) => [MulliganStatus, number]

/** The view in the opening hands with each seat's status and mulligans taken set by `each`. */
function setOpening(view: StateView, each: Each): StateView {
  return {
    ...view,
    stage: 'Mulligan',
    players: view.players.map((player) => {
      const [mulligan, mulligans] = each(player.seat)
      return { ...player, mulligan, mulligans }
    }),
  }
}

/** The state of a seat in the opening hands that has nothing to answer right now. */
const openingState =
  (each: Each) =>
  (state: PlayState): PlayState => ({ ...state, pending: null, view: setOpening(must(state.view), each) })

/** The fixture table in the opening hands. */
const opening = (seats: number, each: Each, more: TableSetup = {}) =>
  renderTable({ seats, state: openingState(each), ...more })

const list = (): HTMLElement => screen.getByRole('region', { name: 'Opening hand decisions' })
const entries = (): string[] =>
  within(list())
    .getAllByRole('listitem')
    .map((item) => item.textContent ?? '')

describe('describeOpening', () => {
  it.each<[MulliganStatus, number, string]>([
    ['Deciding', 0, 'deciding'],
    ['Deciding', 1, 'deciding · 1 mulligan taken'],
    ['Deciding', 3, 'deciding · 3 mulligans taken'],
    ['Kept', 0, 'kept'],
    ['Kept', 2, 'kept · 2 mulligans taken'],
    [{ Bottoming: 1 }, 1, 'kept, bottoming 1 · 1 mulligan taken'],
    [{ Bottoming: 2 }, 2, 'kept, bottoming 2 · 2 mulligans taken'],
    ['Mulliganing', 0, 'taking a mulligan'],
    ['Mulliganing', 1, 'taking a mulligan · 1 mulligan taken'],
    ['Scrying', 1, 'scrying · 1 mulligan taken'],
    ['NeedsHand', 1, 'getting a new hand · 1 mulligan taken'],
  ])('says %j after %i mulligans as "%s"', (status, mulligans, text) => {
    expect(describeOpening(status, mulligans)).toBe(text)
  })
})

describe.each([2, 3, 4, 6, 12])('the opening hands at a table of %i seats', (seats) => {
  it('lists every seat in seat order, this one as "You"', () => {
    opening(seats, () => ['Deciding', 0])
    const names = ['Alice', 'Bo', 'Cy', 'Dee', 'Eli', 'Fay', 'Gus', 'Hana', 'Ivo', 'Jun', 'Kit', 'Lena']
    expect(entries()).toEqual(
      Array.from({ length: seats }, (_, seat) => (seat === 0 ? YOU_WAIT : `${names[seat]}: deciding`)),
    )
  })

  it('shows each seat in its own state, mulligans taken included', () => {
    opening(seats, (seat) => {
      const states: [MulliganStatus, number][] = [
        ['Deciding', 0],
        ['Kept', 0],
        ['Mulliganing', 1],
        [{ Bottoming: 2 }, 2],
      ]
      return states[seat % 4] ?? ['Deciding', 0]
    })
    const expected = [
      YOU_WAIT,
      'Bo: kept',
      'Cy: taking a mulligan · 1 mulligan taken',
      'Dee: kept, bottoming 2 · 2 mulligans taken',
    ]
    expect(entries().slice(0, 4)).toEqual(expected.slice(0, Math.min(seats, 4)))
    expect(entries()).toHaveLength(seats)
  })
})

describe('the opening hands list', () => {
  it('marks who is still choosing apart from who is done, for tests and styling to use', () => {
    opening(3, (seat) => (seat === 1 ? ['Deciding', 0] : ['Kept', 0]))
    const kinds = within(list())
      .getAllByRole('listitem')
      .map((item) => item.getAttribute('data-status'))
    expect(kinds).toEqual(['done', 'choosing', 'done'])
  })

  it('follows the table as batches change who has decided', () => {
    const h = opening(3, () => ['Deciding', 0])
    expect(entries()).toEqual([YOU_WAIT, 'Bo: deciding', 'Cy: deciding'])
    h.update((state) => ({
      view: setOpening(must(state.view), (seat) => (seat === 0 ? ['Kept', 0] : ['Deciding', 0])),
    }))
    expect(entries()).toEqual(['You: kept', 'Bo: deciding', 'Cy: deciding'])
    h.update((state) => ({
      view: setOpening(must(state.view), (seat) => (seat === 1 ? ['Mulliganing', 0] : ['Kept', 0])),
    }))
    expect(entries()).toEqual(['You: kept', 'Bo: taking a mulligan', 'Cy: kept'])
  })

  it('says "deciding" for this seat whether or not its prompt has arrived, since every seat is asked at once', () => {
    const h = opening(3, () => ['Deciding', 0])
    expect(entries()[0]).toBe(YOU_WAIT)
    h.update(() => ({ pending: { id: 1, decision: { player: 0, kind: KINDS.Mulligan } } }))
    expect(entries()).toEqual([YOU_WAIT, 'Bo: deciding', 'Cy: deciding'])
  })

  it('uses the names the lookup gives, and "Seat N" for a seat it does not know', () => {
    opening(3, () => ['Deciding', 0], { lookup: { game: null, missing: false, failed: false } })
    expect(entries()).toEqual([YOU_WAIT, 'Seat 2: deciding', 'Seat 3: deciding'])
  })

  it('is on the table in the layout for a big table too, where only one opponent has a panel', () => {
    opening(8, () => ['Deciding', 0], { options: { mode: 'focus' } })
    expect(entries()).toHaveLength(8)
    expect(document.querySelectorAll('[data-seat]').length).toBeLessThan(8)
  })

  it('is not there once turns are played', () => {
    renderTable({ seats: 3 })
    expect(screen.queryByRole('region', { name: 'Opening hand decisions' })).toBeNull()
  })

  it('does not take the place of the seat panels, which say the same at greater length', () => {
    opening(2, () => ['Deciding', 1])
    expect(document.querySelector('[data-seat="1"] [data-slot="mulligan"]')).toHaveTextContent(
      'Mulligan: Deciding whether to keep · 1 mulligan',
    )
  })

  it('prints a name as plain text, long or not', () => {
    const fixture = buildFixture({ seats: 2 })
    const odd = `<b>${'x'.repeat(29)}`
    renderTable({
      fixture,
      state: openingState(() => ['Kept', 0]),
      lookup: {
        game: {
          started: true,
          seats: [
            { seat: 0, name: 'Ada' },
            { seat: 1, name: odd },
          ],
          connected: [0, 1],
          ready: [0, 1],
          result: null,
        },
        missing: false,
        failed: false,
      },
    })
    expect(entries()[1]).toBe(`${odd}: kept`)
    expect(list().querySelector('b')).toBeNull()
  })
})
