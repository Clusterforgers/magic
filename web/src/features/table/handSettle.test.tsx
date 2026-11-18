//! Goal: the hand dock takes or gives back its second row only at a settle point, the next step of a turn: cards drawn or played within a step leave the dock's height as it was, at every table size from a duel to six seats.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { PlayState } from '@/play/store'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from './dev/fixture'
import { HAND_GROW_VAR } from './layout'

/** The room every element reports; jsdom has no layout. */
const room = { width: 0, height: 0 }

beforeEach(() => {
  room.width = 700
  room.height = 220
  vi.spyOn(Element.prototype, 'clientWidth', 'get').mockImplementation(() => room.width)
  vi.spyOn(Element.prototype, 'clientHeight', 'get').mockImplementation(() => room.height)
})

afterEach(() => {
  vi.restoreAllMocks()
})

const grown = (): string =>
  must(document.querySelector<HTMLElement>('[data-slot="table-main"]')).style.getPropertyValue(HAND_GROW_VAR)

/** The viewer's hand cut to its first `count` cards. */
const handOf =
  (count: number) =>
  (state: PlayState): PlayState => {
    const view = must(state.view)
    return {
      ...state,
      view: {
        ...view,
        players: view.players.map((player) =>
          player.seat !== state.seat || player.hand.cards === null
            ? player
            : { ...player, hand: { ...player.hand, cards: player.hand.cards.slice(0, count), count } },
        ),
      },
    }
  }

/** The view moved on to `step`. */
const atStep =
  (step: 'Draw' | 'Main1' | 'BeginCombat') =>
  (state: PlayState): PlayState => {
    const view = must(state.view)
    return { ...state, view: { ...view, turn: { ...view.turn, step } } }
  }

describe('the hand dock settles its rows once a step', () => {
  it.each([2, 4, 6])(
    'at %i seats keeps a grown dock while cards are played and gives the row back at the next step',
    (seats) => {
      const table = renderTable({ fixture: buildFixture({ seats, hand: 16 }), state: atStep('Main1') })
      const tall = grown()
      expect(tall).not.toBe('')
      table.update(handOf(5))
      expect(grown()).toBe(tall)
      table.update(handOf(4))
      expect(grown()).toBe(tall)
      table.update(atStep('BeginCombat'))
      expect(grown()).toBe('')
    },
  )

  it.each([2, 4, 6])(
    'at %i seats keeps one row while cards are drawn and grows at the next step',
    (seats) => {
      const fixture = buildFixture({ seats, hand: 16 })
      const table = renderTable({ fixture, state: (state) => handOf(4)(atStep('Draw')(state)) })
      expect(grown()).toBe('')
      table.update((state) => {
        const full = renderedHand(fixture.view, state)
        return full
      })
      expect(grown()).toBe('')
      table.update(atStep('Main1'))
      expect(grown()).not.toBe('')
    },
  )
})

/** The state with the viewer's whole hand of the fixture back. */
function renderedHand(view: PlayState['view'], state: PlayState): PlayState {
  const seat = state.seat
  const full = must(view).players.find((player) => player.seat === seat)
  const now = must(state.view)
  return {
    ...state,
    view: {
      ...now,
      players: now.players.map((player) =>
        player.seat === seat && full !== undefined ? { ...player, hand: full.hand } : player,
      ),
    },
  }
}
