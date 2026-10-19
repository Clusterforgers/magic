// Goal: the hand dock draws the viewer's cards as a fan as tall as the dock allows, with the normal gap while they fit its width and an overlap of at least 62 % showing when they do not (smaller cards before a scroll, and 44 px strips under a touch screen), drops to the short card face under 76 px, rises a hovered or focused card by 40 % and opens its preview, moves focus along the hand with the arrow keys, keeps one tab stop, takes no pointer above the cards, and says so when the hand is empty or hidden, for 1 to 20 cards.
import { act, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { PlayState } from '@/play/store'
import { indexById } from '@/play/view'
import { clone, must } from '@/test-utils/batches'
import { card } from '@/test-utils/cards'
import { renderTable } from '@/test-utils/tableHarness'
import { fitRows } from './fit'
import { handCardWidth, handLayout } from './handFit'

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
  vi.unstubAllGlobals()
})

const hand = (): HTMLElement => screen.getByRole('list', { name: "Alice's hand" })
const dock = (): HTMLElement => must(document.querySelector<HTMLElement>('[data-region="hand"]'))
const cardsOf = (): HTMLElement[] => [...hand().querySelectorAll<HTMLElement>('button[data-object-id]')]
const vars = (name: string): number => Number.parseInt(hand().style.getPropertyValue(name), 10)

/** The table with `count` cards in the viewer's hand. */
function withHand(count: number): (state: PlayState) => PlayState {
  return (state) => {
    const view = clone(must(state.view))
    const ids = Array.from({ length: count }, (_, index) => 9000 + index)
    view.objects = [
      ...view.objects.filter((object) => !(typeof object.zone === 'object' && 'Hand' in object.zone)),
      ...ids.map((id) =>
        card({ id, owner: 0, controller: 0, zone: { Hand: 0 }, permanent: null, name: `Card ${id}` }),
      ),
    ]
    must(view.players[0]).hand = { count, cards: ids }
    return { ...state, view, objectsById: indexById(view.objects) }
  }
}

/** The room of a dock `width` by `height` as the dock measures it: its own 4 px of padding are kept round the line. */
const layoutOf = (width: number, height: number, count: number, coarse = false) =>
  handLayout({ width: width - 8, height: height - 8, count, coarse })

describe('without layout', () => {
  it('draws the hand as a row of cards of the usual size, in the dock', () => {
    renderTable({ seats: 2 })
    expect(dock()).toContainElement(hand())
    expect(cardsOf()).toHaveLength(7)
    expect(hand().style.getPropertyValue('--card-w')).toBe('')
  })

  it('is a region named for the hand, with the hand as one tab stop', () => {
    renderTable({ seats: 2 })
    expect(screen.getByRole('region', { name: 'Your hand' })).toBe(dock())
    expect(cardsOf().filter((each) => each.getAttribute('tabindex') === '0')).toHaveLength(1)
  })
})

describe('with a size', () => {
  it.each([
    [{ width: 700, height: 106 }],
    [{ width: 500, height: 106 }],
    [{ width: 260, height: 100 }],
    [{ width: 1200, height: 175 }],
  ])('makes the cards as tall as the dock allows and spaces them to fit a %o box', (box) => {
    room.width = box.width
    room.height = box.height
    renderTable({ seats: 2 })
    const expected = layoutOf(box.width, box.height, 7)
    expect(vars('--card-w')).toBe(expected.box.width)
    expect(vars('--card-h')).toBe(Math.round(expected.box.width * 1.4))
    expect(vars('--card-gap')).toBe(expected.gap)
    expect(vars('--hand-lift')).toBe(expected.lift)
  })

  it('keeps the normal gap of 6 px while the seven cards fit the width', () => {
    room.width = 900
    room.height = 106
    renderTable({ seats: 2 })
    expect(vars('--card-gap')).toBe(6)
  })

  it('overlaps the cards, smaller than the dock is tall, in a dock too narrow for them side by side', () => {
    room.width = 300
    room.height = 106
    renderTable({ seats: 2 })
    expect(vars('--card-gap')).toBeLessThan(0)
    expect(vars('--card-w')).toBeLessThan(handCardWidth(106 - 8))
    expect(hand()).not.toHaveAttribute('data-scrolls')
  })

  it('draws the whole face of a card from 76 px wide and the short one under it', () => {
    room.width = 900
    room.height = 175
    const { unmount } = renderTable({ seats: 2 })
    expect(new Set(cardsOf().map((each) => each.getAttribute('data-size')))).toEqual(new Set(['normal']))
    unmount()
    room.height = 90
    renderTable({ seats: 2 })
    expect(handCardWidth(90 - 8)).toBeLessThan(76)
    expect(new Set(cardsOf().map((each) => each.getAttribute('data-size')))).toEqual(new Set(['compact']))
  })

  it('never makes a card narrower than the 44 px floor of the fit engine', () => {
    room.width = 400
    room.height = 40
    renderTable({ seats: 2 })
    expect(vars('--card-w')).toBe(44)
    expect(fitRows({ width: 400, height: 40 }, [{ stacks: 7 }]).cardWidth).toBe(44)
  })
})

describe('the fan for 1 to 20 cards', () => {
  it.each([1, 2, 7, 12, 20])('draws %i cards as the layout says, in one list of one tab stop', (count) => {
    room.width = 560
    room.height = 120
    renderTable({ seats: 2, state: withHand(count) })
    const expected = layoutOf(560, 120, count)
    expect(cardsOf()).toHaveLength(count)
    expect(cardsOf().filter((each) => each.getAttribute('tabindex') === '0')).toHaveLength(1)
    expect(vars('--card-w')).toBe(expected.box.width)
    expect(vars('--card-gap')).toBe(expected.gap)
    expect(hand().hasAttribute('data-scrolls')).toBe(expected.scrolls)
    // 62 % of every card but the last shows, and the line fits the dock unless it scrolls.
    const step = expected.box.width + expected.gap
    if (count > 1) expect(step).toBeGreaterThanOrEqual(Math.ceil(expected.box.width * 0.62))
    if (!expected.scrolls) expect(expected.box.width + (count - 1) * step).toBeLessThanOrEqual(560 - 8)
  })

  it('rises a card by 40 % of its height and keeps the room for it above the cards', () => {
    room.width = 700
    room.height = 120
    renderTable({ seats: 2 })
    expect(vars('--hand-lift')).toBe(Math.round(vars('--card-h') * 0.4))
    expect(hand()).toHaveClass('pt-(--hand-lift)', 'pointer-events-none')
    const slots = hand().querySelectorAll('[data-slot="hand-slot"]')
    expect(slots).toHaveLength(7)
    for (const slot of slots) {
      expect(slot).toHaveClass('pointer-events-auto', 'hover:z-10', 'focus-within:z-10')
      const lifter = must(slot.firstElementChild)
      expect(lifter.className).toContain('group-hover/slot:-translate-y-[40%]')
      expect(lifter.className).toContain('group-has-focus-visible/slot:-translate-y-[40%]')
      expect(lifter.className).toContain('pointer-coarse:group-focus-within/slot:-translate-y-[40%]')
      expect(lifter.className).toContain('motion-safe:transition-transform')
    }
  })

  it('keeps a strip of 44 px of every card under a touch screen, and scrolls rather than overlap more', () => {
    vi.stubGlobal(
      'matchMedia',
      (query: string) =>
        ({
          matches: query === '(pointer: coarse)',
          addEventListener: () => undefined,
          removeEventListener: () => undefined,
        }) as unknown as MediaQueryList,
    )
    room.width = 300
    room.height = 120
    renderTable({ seats: 2, state: withHand(12) })
    expect(vars('--card-w') + vars('--card-gap')).toBeGreaterThanOrEqual(44)
    expect(hand()).toHaveAttribute('data-scrolls', 'true')
    expect(vars('--card-w')).toBe(handCardWidth(120 - 8))
  })
})

describe('keyboard and preview', () => {
  it('moves focus along the hand with the left and right arrows, and keeps the tab stop on the focused card', async () => {
    room.width = 700
    room.height = 120
    const { user } = renderTable({ seats: 2 })
    const cards = cardsOf()
    act(() => cards[0]?.focus())
    expect(cards[0]).toHaveFocus()
    await user.keyboard('{ArrowRight}')
    expect(cards[1]).toHaveFocus()
    await user.keyboard('{ArrowRight}{ArrowRight}')
    expect(cards[3]).toHaveFocus()
    expect(cards[3]).toHaveAttribute('tabindex', '0')
    expect(cards[0]).toHaveAttribute('tabindex', '-1')
    await user.keyboard('{ArrowLeft}')
    expect(cards[2]).toHaveFocus()
    await user.keyboard('{End}')
    expect(cards.at(-1)).toHaveFocus()
    await user.keyboard('{ArrowRight}')
    expect(cards.at(-1)).toHaveFocus()
    await user.keyboard('{Home}{ArrowLeft}')
    expect(cards[0]).toHaveFocus()
  })

  it('opens the preview of a card on hover and closes it when the pointer leaves', async () => {
    const { user } = renderTable({ seats: 2 })
    const first = must(cardsOf()[0])
    await user.hover(first)
    expect(screen.getByTestId('card-preview')).toHaveTextContent(
      first.getAttribute('aria-label')?.split(',')[0] ?? '',
    )
    await user.unhover(first)
    expect(screen.queryByTestId('card-preview')).toBeNull()
  })

  it('opens the preview of the card the arrow keys reach, and moves it with the focus', async () => {
    const { user } = renderTable({ seats: 2 })
    const cards = cardsOf()
    act(() => cards[0]?.focus())
    expect(screen.getByTestId('card-preview')).toBeInTheDocument()
    await user.keyboard('{ArrowRight}')
    const shown = screen.getByTestId('card-preview')
    expect(shown).toHaveTextContent(must(cards[1]?.getAttribute('aria-label')).split(',')[0] ?? '')
    act(() => cards[1]?.blur())
    expect(screen.queryByTestId('card-preview')).toBeNull()
  })
})

describe('what the dock says', () => {
  it('says the hand is empty when it is', () => {
    renderTable({ seats: 2, state: withHand(0) })
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
    const before = cardsOf().length
    table.update((state) => {
      const view = must(state.view)
      const mine = must(view.players[0])
      const extra = must(
        view.objects.find(
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
    expect(cardsOf()).toHaveLength(before + 1)
  })
})
