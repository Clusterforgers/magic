/** Goal: a teammate's zones open from every layout (grid pods, a Delves hero's frame, a phone's ally peek) as your own do, and a hand the view lists of another seat is a Hand chip that opens its cards with the preview, marked as theirs and not yours to play, while a hand the view only counts stays a count. */
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ObjectId, StateView } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { card } from '@/test-utils/cards'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from './dev/fixture'
import type { Fixture } from './dev/fixture'
import { withBoss } from './dev/devGame'

/** `view` where the hand of `seat` is listed to the viewer: two cards named "Held <seat>-<n>". */
function withListedHand(view: StateView, seat: number): StateView {
  const next = structuredClone(view)
  const ids: ObjectId[] = [8000 + 10 * seat, 8001 + 10 * seat]
  next.objects.push(
    ...ids.map((id, at) =>
      card({
        id,
        owner: seat,
        controller: seat,
        zone: { Hand: seat },
        permanent: null,
        name: `Held ${seat}-${at}`,
      }),
    ),
  )
  must(next.players[seat]).hand = { count: ids.length, cards: ids }
  return next
}

const listed = (fixture: Fixture, seat: number): Fixture => ({
  ...fixture,
  view: withListedHand(fixture.view, seat),
})

const PHONE = { width: 390, height: 844 }

describe('a hand the view lists of another seat', () => {
  it('is a Hand chip on that pod that opens its cards as theirs; a counted hand stays a count', async () => {
    const { user } = renderTable({ fixture: listed(buildFixture({ seats: 4 }), 2) })
    const mate = must(document.querySelector<HTMLElement>('section[data-seat="2"]'))
    const other = must(document.querySelector<HTMLElement>('section[data-seat="1"]'))
    expect(other.querySelector('button[data-zone="hand"]')).toBeNull()
    const chip = must(mate.querySelector<HTMLElement>('button[data-zone="hand"]'))
    expect(chip).toHaveAccessibleName('Hand 2 cards, opens the list')
    expect(chip).not.toHaveAttribute('data-legal')
    await user.click(chip)
    const dialog = screen.getByRole('dialog')
    expect(within(dialog).getByRole('heading').textContent).toMatch(/’s hand$/)
    expect(dialog.textContent).toContain('Only')
    expect(dialog.textContent).toContain('can play them')
    const list = must(dialog.querySelector('[data-slot="their-hand"]'))
    // The hand keeps its own order.
    expect(
      [...list.querySelectorAll('[data-object-id]')].map((el) => el.getAttribute('data-object-id')),
    ).toEqual(['8020', '8021'])
  })
})

describe("a Delves hero's zones", () => {
  it.each([
    ['beside yours', undefined],
    ['on a phone', PHONE],
  ] as const)('open from the board %s, the hand among them', async (_, viewport) => {
    // Heroes 0, 1 and 2 against the boss at seat 3; seat 1's hand is listed to the viewer, the boss's is not.
    const fixture = listed(withBoss(buildFixture({ seats: 4 }), 0), 1)
    const { user } = renderTable({ fixture, ...(viewport === undefined ? {} : { options: { viewport } }) })
    if (viewport !== undefined) {
      // A phone tiles the party: a tap on the hero's tile opens its board.
      await user.click(must(document.querySelector<HTMLElement>('[data-tile-seat="1"]')))
    }
    const area = must(document.querySelector<HTMLElement>('[data-seat-area="1"]'))
    const zones = must(area.querySelector<HTMLElement>('[data-slot="board-zones"]'))
    const graveyard = must(zones.querySelector<HTMLElement>('button[data-zone="graveyard"]'))
    await user.click(graveyard)
    expect(within(screen.getByRole('dialog')).getByRole('heading').textContent).toMatch(/graveyard$/)
    await user.keyboard('{Escape}')
    await user.click(must(zones.querySelector<HTMLElement>('button[data-zone="hand"]')))
    expect(screen.getByRole('dialog').querySelector('[data-object-id="8010"]')).not.toBeNull()
    await user.keyboard('{Escape}')
    expect(document.querySelector('[data-seat-area="3"] button[data-zone="hand"]')).toBeNull()
    expect(document.querySelector('[data-seat-area="0"] button[data-zone="hand"]')).toBeNull()
  })
})
