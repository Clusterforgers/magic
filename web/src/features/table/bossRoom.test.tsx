// Goal: in boss mode a board without permanents is one line (a Spirit's own row, an ally's tile) while the boss's empty side keeps its pod and a quiet empty board, and a hand dock with nothing to play is a slim bar, while a board that has permanents keeps its zone; the opening-hand decision floats in the middle of the table over the dimmed boards with the seats' keep status in it; every table off a phone floats the opening-hand decision the same way, the dock keeping its place but not a second copy of the hand, and a phone keeps it in the strip.
import { screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { Answer } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { withBareBoards, withBoss, withSpirit } from './dev/devGame'
import { buildFixture } from './dev/fixture'
import { HAND_SLIM_PX } from './layout'
import type { Viewport } from './layout'

const SLIM = new RegExp(` ${HAND_SLIM_PX}px`)

const rowsOf = (container: HTMLElement): string =>
  container.querySelector('[data-slot="table-main"]')?.getAttribute('style') ?? ''

/** A boss fight of two heroes (seats 0 and 1) and a boss (seat 2). */
const fight = () => withBoss(buildFixture({ seats: 3 }), 0)

describe('boss mode with boards that draw nothing', () => {
  it("a Spirit with no permanents and no hand: its row is its pod's line and the dock a slim bar", () => {
    const { container } = renderTable({ fixture: withSpirit(fight()) })
    expect(container.querySelector('[data-battlefield-seat="0"]')).toBeNull()
    expect(container.querySelector('[data-seat-area="0"] [data-variant="line"]')).not.toBeNull()
    expect(container.querySelector('[data-seat-area="0"] [data-slot="pod-column"]')).toBeNull()
    // The boss and the ally still draw their boards.
    expect(container.querySelector('[data-battlefield-seat="1"]')).not.toBeNull()
    expect(container.querySelector('[data-battlefield-seat="2"]')).not.toBeNull()
    expect(container.querySelector('[data-slot="ally-frame"]')?.hasAttribute('data-collapsed')).toBe(false)
    expect(rowsOf(container)).toMatch(SLIM)
  })

  it('a hero with a hand keeps its board room even where it has no permanent yet, and the dock keeps its size', () => {
    const { container } = renderTable({ fixture: withBareBoards(fight()) })
    expect(container.querySelector('[data-battlefield-seat="0"]')).not.toBeNull()
    expect(container.querySelector('[data-seat-area="0"] [data-slot="pod-column"]')).not.toBeNull()
    expect(rowsOf(container)).not.toMatch(SLIM)
  })

  it("an ally's frame with no permanent is its tile, and the boss's side with none keeps its pod and a quiet empty board", () => {
    const { container } = renderTable({ fixture: withBareBoards(fight()) })
    const ally = container.querySelector('[data-slot="ally-frame"]')
    expect(ally?.hasAttribute('data-collapsed')).toBe(true)
    expect(ally?.querySelector('[data-battlefield-seat]')).toBeNull()
    expect(ally?.querySelector('[data-slot="seat-tile"], [data-seat-tile], button')).not.toBeNull()
    const boss = container.querySelector('[data-slot="boss-frame"]')
    expect(boss?.querySelector('[data-battlefield-seat]')).not.toBeNull()
    expect(boss?.querySelector('[data-slot="pod-column"]')).not.toBeNull()
    expect(boss?.textContent).toContain('No permanents')
  })

  it("the boss's board stays in place when its seat gets a permanent", () => {
    const { container, update } = renderTable({ fixture: withBareBoards(fight()) })
    expect(container.querySelector('[data-slot="boss-frame"] [data-battlefield-seat="2"]')).not.toBeNull()
    const full = fight()
    update((state) => ({
      ...state,
      view: full.view,
      objectsById: Object.fromEntries(full.view.objects.map((o) => [o.id, o])),
    }))
    expect(container.querySelector('[data-slot="boss-frame"] [data-battlefield-seat="2"]')).not.toBeNull()
    expect(container.querySelector('[data-slot="boss-frame"]')?.hasAttribute('data-collapsed')).toBe(false)
  })

  it('a table without a boss keeps its dock and rows with an empty hand', () => {
    const plain = buildFixture({ seats: 3 })
    const view = clone(plain.view)
    must(view.players[0]).hand = { count: 0, cards: [] }
    must(view.players[0]).command = { count: 0, cards: [] }
    must(view.players[0]).commanders = []
    const { container } = renderTable({ fixture: { ...plain, view } })
    expect(rowsOf(container)).not.toMatch(SLIM)
  })
})

describe('the opening hands in boss mode', () => {
  /** The table of seat 0 at its Mulligan decision, over a boss fight or a plain table of three. */
  function mulligan(boss: boolean, viewport?: Viewport) {
    const answer = vi.fn((_answer: Answer) => true)
    const base = buildFixture({ seats: 3 })
    const fixture = boss ? withBareBoards(withBoss(base, 0)) : base
    const hand = must(fixture.view.players[0]?.hand.cards)
    const table = renderTable({
      fixture,
      ...(viewport === undefined ? {} : { options: { viewport } }),
      session: { answer },
      state: (state) => {
        const view = clone(must(state.view))
        view.stage = 'Mulligan'
        for (const player of view.players) player.mulligan = player.seat === 1 ? 'Kept' : 'Deciding'
        const kind = {
          Mulligan: {
            hand,
            mulligans_taken: 0,
            can_mulligan: true,
            free_mulligans: 0,
            bottom_if_keep: 0,
            bottom_if_mulligan: 1,
          },
        }
        return { ...state, view, pending: { id: 3, decision: { player: 0, kind } } }
      },
    })
    return { ...table, answer, hand }
  }

  it.each([
    ['a boss fight', true],
    ['a plain table of three', false],
  ])(
    'floats in the overlay at %s: the hand in one row of large cards, the buttons and every seat under it',
    (_name, boss) => {
      const { container, hand } = mulligan(boss)
      const panel = container.querySelector(
        '[data-slot="choice-host"] [data-slot="choice-overlay"][data-opening]',
      )
      expect(panel).not.toBeNull()
      const cards = panel?.querySelectorAll('[data-slot="opening-hand"] [data-slot="opening-card"]')
      expect(cards).toHaveLength(hand.length)
      const row = panel?.querySelector<HTMLElement>('[data-slot="opening-hand"]')
      const width = Number.parseInt(row?.style.getPropertyValue('--card-w') ?? '0', 10)
      expect(width).toBeGreaterThanOrEqual(140)
      expect(width).toBeLessThanOrEqual(180)
      expect(panel?.querySelector('[data-slot="prompt-text"]')?.textContent).toContain('Keep this hand')
      expect(panel?.querySelectorAll('[data-slot="opening-status"] [data-seat-status]')).toHaveLength(3)
      expect(container.querySelector('[data-slot="choice-host"]')?.hasAttribute('hidden')).toBe(false)
      // The strip carries neither the hand nor the status twice.
      expect(container.querySelectorAll('[data-slot="opening-hand"]')).toHaveLength(1)
      expect(container.querySelectorAll('[data-slot="opening-status"]')).toHaveLength(1)
    },
  )

  it.each([true, false])('keeps the dock in place without a second copy of the hand (boss %s)', (boss) => {
    const { container } = mulligan(boss)
    const dock = must(container.querySelector('[data-region="hand"] [data-floating]'))
    expect(dock).toHaveClass('invisible')
    expect(dock).toHaveAttribute('aria-hidden', 'true')
  })

  it('answers Keep from the overlay', async () => {
    const { answer, user } = mulligan(false)
    await user.click(must(screen.getAllByRole('button', { name: 'Keep' })[0]))
    expect(answer).toHaveBeenCalledWith({ Mulligan: 'Keep' })
  })

  it.each([true, false])(
    'keeps the hand in the hand dock on a phone, its answers in the bar, the dock as it is (boss %s)',
    (boss) => {
      const { container } = mulligan(boss, { width: 390, height: 844 })
      expect(container.querySelector('[data-opening]')).toBeNull()
      // The hand dock shows the hand being judged; nothing draws it again over the boards.
      expect(container.querySelector('[data-slot="opening-hand"]')).toBeNull()
      expect(container.querySelector('[data-home="bar"] [data-slot="prompt-actions"]')).not.toBeNull()
      expect(container.querySelector('[data-region="hand"] [data-floating]')).toBeNull()
    },
  )
})
