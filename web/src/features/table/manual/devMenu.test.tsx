// Goal: `?menu=` names the card the dev table opens its manual menu on, finds it among the fixture's cards, and the dev session answers a manual action by logging it and keeping priority.
import { act, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { parseDevSearch } from '../devSearch'
import type { DevMenu } from '../devSearch'
import { buildFixture } from '../dev/fixture'
import { fixtureGame } from '../dev/devGame'
import { GameTable } from '../GameTable'
import { renderTable } from '@/test-utils/tableHarness'
import { menuCandidates, rightClickFirst } from './devMenu'

describe('?menu=', () => {
  it.each([
    [1, 'permanent'],
    ['1', 'permanent'],
    [true, 'permanent'],
    ['hand', 'hand'],
    ['opp', 'opponent'],
    ['opponent', 'opponent'],
    ['nonsense', undefined],
    [0, undefined],
    [undefined, undefined],
  ])('reads %j as %j', (value, expected) => {
    expect(parseDevSearch({ menu: value }).menu).toBe(expected)
  })
})

describe('the card the menu opens on', () => {
  const view = buildFixture({ seats: 3 }).view
  const kinds: [DevMenu, (controller: number, zone: unknown) => boolean][] = [
    ['permanent', (controller, zone) => controller === view.viewer && zone === 'Battlefield'],
    ['opponent', (controller, zone) => controller !== view.viewer && zone === 'Battlefield'],
  ]
  it.each(kinds)('for %s is a creature on the battlefield of the right seat', (kind, fits) => {
    const ids = menuCandidates(view, view.viewer, kind)
    expect(ids.length).toBeGreaterThan(0)
    for (const id of ids) {
      const object = view.objects.find((entry) => entry.id === id)
      expect(object?.types).toContain('Creature')
      expect(fits(object?.controller ?? -1, object?.zone)).toBe(true)
    }
  })

  it('for the hand is the cards of the viewer hand', () => {
    const hand = view.players.find((player) => player.seat === view.viewer)?.hand.cards
    expect(menuCandidates(view, view.viewer, 'hand')).toEqual(hand)
  })

  it('puts a creature with counters first', () => {
    const mine = view.objects.filter(
      (entry) =>
        entry.controller === view.viewer && entry.zone === 'Battlefield' && entry.types.includes('Creature'),
    )
    const last = mine.at(-1)
    const marked = {
      ...view,
      objects: view.objects.map((entry) =>
        entry.id === last?.id ? { ...entry, counters: [['+1/+1', 1]] as ['+1/+1', number][] } : entry,
      ),
    }
    expect(menuCandidates(marked, view.viewer, 'permanent')[0]).toBe(last?.id)
  })
})

describe('on the dev table', () => {
  it('opens the menu of the first card that is drawn, as a right click does', () => {
    renderTable({ seats: 2, ui: <GameTable /> })
    const view = buildFixture({ seats: 2 }).view
    const ids = menuCandidates(view, view.viewer, 'permanent')
    act(() => {
      expect(rightClickFirst(ids)).toBe(true)
    })
    expect(screen.getByRole('menu')).toBeInTheDocument()
  })

  it('answers a manual action with a line in the log and keeps the priority decision', () => {
    const game = fixtureGame({ seats: 2, holding: true })
    const before = game.session.store.getState()
    const logged = before.log.length
    const sent = game.session.answer({ Priority: { Manual: { Draw: { count: 1 } } } })
    const after = game.session.store.getState()
    expect(sent).toBe(true)
    expect(after.pending?.id).toBe(before.pending?.id)
    expect(after.log).toHaveLength(logged + 1)
    expect(after.log.at(-1)?.event).toEqual({
      Public: { ManualAction: { player: before.seat, action: { Draw: { count: 1 } } } },
    })
  })

  it('still clears the decision for any other answer', () => {
    const game = fixtureGame({ seats: 2, holding: true })
    game.session.answer({ Priority: 'Pass' })
    expect(game.session.store.getState().pending).toBeNull()
  })
})
