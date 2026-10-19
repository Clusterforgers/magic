// Goal: the hand dock draws the viewer's cards in one line as tall as the dock allows, with the normal gap while they fit its width and overlapping just enough when they do not, drops to the short card face under 76 px, keeps one tab stop for the whole hand, and says so when the hand is empty or hidden.
import { screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { fitRows } from './fit'
import { handCardWidth, handFan } from './handFit'

/** The room every element reports; jsdom has no layout. */
const room = { width: 0, height: 0 }

beforeEach(() => {
  room.width = 0
  room.height = 0
  vi.spyOn(Element.prototype, 'clientWidth', 'get').mockImplementation(() => room.width)
  vi.spyOn(Element.prototype, 'clientHeight', 'get').mockImplementation(() => room.height)
})

afterEach(() => {
  vi.restoreAllMocks()
})

const hand = (): HTMLElement => screen.getByRole('list', { name: "Alice's hand" })
const dock = (): HTMLElement => must(document.querySelector<HTMLElement>('[data-region="hand"]'))

describe('without layout', () => {
  it('draws the hand as a row of cards of the usual size, in the dock', () => {
    renderTable({ seats: 2 })
    expect(dock()).toContainElement(hand())
    expect(hand().querySelectorAll('button[data-object-id]')).toHaveLength(7)
    expect(hand().style.getPropertyValue('--card-w')).toBe('')
  })

  it('is a region named for the hand, with the hand as one tab stop', () => {
    renderTable({ seats: 2 })
    expect(screen.getByRole('region', { name: 'Your hand' })).toBe(dock())
    const cards = [...hand().querySelectorAll('button[data-object-id]')]
    expect(cards.filter((card) => card.getAttribute('tabindex') === '0')).toHaveLength(1)
  })
})

describe('with a size', () => {
  // The dock's own padding is 8 px a side, which is not in the box measured; the row keeps 4 px round it.
  it.each([
    [{ width: 700, height: 106 }],
    [{ width: 500, height: 106 }],
    [{ width: 260, height: 100 }],
    [{ width: 1200, height: 175 }],
  ])('makes the cards as tall as the dock allows and spaces them to fit a %o box', (box) => {
    room.width = box.width
    room.height = box.height
    renderTable({ seats: 2 })
    const width = handCardWidth(box.height - 8)
    expect(hand().style.getPropertyValue('--card-w')).toBe(`${width}px`)
    expect(hand().style.getPropertyValue('--card-h')).toBe(`${Math.round(width * 1.4)}px`)
    expect(hand().style.getPropertyValue('--card-gap')).toBe(`${handFan(box.width - 8, 7, width).gap}px`)
  })

  it('keeps the normal gap of 6 px while the seven cards fit the width', () => {
    room.width = 900
    room.height = 106
    renderTable({ seats: 2 })
    expect(hand().style.getPropertyValue('--card-gap')).toBe('6px')
  })

  it('overlaps the cards in a dock too narrow for them side by side', () => {
    room.width = 300
    room.height = 106
    renderTable({ seats: 2 })
    expect(Number.parseInt(hand().style.getPropertyValue('--card-gap'), 10)).toBeLessThan(0)
  })

  it('draws the whole face of a card from 76 px wide and the short one under it', () => {
    room.width = 900
    room.height = 175
    const { unmount } = renderTable({ seats: 2 })
    expect(
      new Set([...hand().querySelectorAll('[data-size]')].map((card) => card.getAttribute('data-size'))),
    ).toEqual(new Set(['normal']))
    unmount()
    room.height = 90
    renderTable({ seats: 2 })
    expect(handCardWidth(90 - 8)).toBeLessThan(76)
    expect(
      new Set([...hand().querySelectorAll('[data-size]')].map((card) => card.getAttribute('data-size'))),
    ).toEqual(new Set(['compact']))
  })

  it('never makes a card narrower than the 44 px floor of the fit engine', () => {
    room.width = 400
    room.height = 40
    renderTable({ seats: 2 })
    expect(hand().style.getPropertyValue('--card-w')).toBe('44px')
    expect(fitRows({ width: 400, height: 40 }, [{ stacks: 7 }]).cardWidth).toBe(44)
  })
})

describe('what the dock says', () => {
  it('says the hand is empty when it is', () => {
    renderTable({
      seats: 2,
      state: (state) => ({
        ...state,
        view: state.view && {
          ...state.view,
          players: state.view.players.map((player) =>
            player.seat === 0 ? { ...player, hand: { count: 0, cards: [] } } : player,
          ),
        },
      }),
    })
    expect(dock()).toHaveTextContent('Your hand is empty.')
    expect(screen.queryByRole('list', { name: "Alice's hand" })).toBeNull()
  })

  it('draws card backs when the cards are not known to this client', () => {
    renderTable({
      seats: 2,
      state: (state) => ({
        ...state,
        view: state.view && {
          ...state.view,
          players: state.view.players.map((player) =>
            player.seat === 0 ? { ...player, hand: { count: 3, cards: null } } : player,
          ),
        },
      }),
    })
    expect(dock().querySelector('[data-slot="hand-backs"]')).not.toBeNull()
  })

  it('shows a card drawn at once, without redrawing the others', () => {
    const table = renderTable({ seats: 2 })
    const before = hand().querySelectorAll('button[data-object-id]').length
    table.update((state) => {
      const view = must(state.view)
      const mine = must(view.players[0])
      const objects = view.objects
      const extra = must(
        objects.find(
          (object) => typeof object.zone === 'object' && 'Graveyard' in object.zone && object.owner === 0,
        ),
      )
      return {
        view: {
          ...view,
          players: view.players.map((player) =>
            player.seat === 0
              ? { ...mine, hand: { count: before + 1, cards: [...(mine.hand.cards ?? []), extra.id] } }
              : player,
          ),
        },
      }
    })
    expect(hand().querySelectorAll('button[data-object-id]')).toHaveLength(before + 1)
  })
})
