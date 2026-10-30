/** Goal: a four-seat Commander table shows every commander: an opponent's pod draws its commanders (both of a partner pair) with a tax chip only once one was cast, the damaged seat's pod names each source with the lethal amount and a tone that rises at 15 and 18 of 21 (moving with a house-rule amount), the tile repeats it, and your own commanders wait in the hand dock with the tax in their name, castable like the hand. */
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { buildFixture } from '@/features/table/dev/fixture'
import type { Fixture } from '@/features/table/dev/fixture'
import { DEV_PROMPTS, DEV_TABLES } from '@/features/table/dev/prompts'
import type { StateView } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'

const FIXTURE = buildFixture({ seats: 4 })
const ATRAXA = 'Atraxa, Praetors’ Voice'
const pod = (seat: number): HTMLElement =>
  must(document.querySelector<HTMLElement>(`section[data-seat="${seat}"]`), `pod ${seat}`)
const chips = (root: HTMLElement, slot: string): string[] =>
  [...root.querySelectorAll(`[data-slot="${slot}"]`)].map((chip) => chip.getAttribute('aria-label') ?? '')

function show(edit: (view: StateView) => void = () => undefined, fixture: Fixture = FIXTURE) {
  const view = structuredClone(fixture.view)
  edit(view)
  return renderTable({ fixture: { ...fixture, view }, options: { viewport: { width: 1920, height: 1080 } } })
}

describe('the pods', () => {
  it.each([
    [1, ['Atraxa, Praetors’ Voice: cast 2 times from the command zone, tax 4 mana']],
    [2, []],
    [3, ['Meren of Clan Nel Toth: cast 1 time from the command zone, tax 2 mana']],
  ])('seat %i shows the tax chips %j', (seat, expected) => {
    show()
    expect(chips(pod(seat), 'commander-tax')).toEqual(expected)
  })

  it('writes the tax chip as "Cast 2× · tax {4}"', () => {
    show()
    expect(must(pod(1).querySelector('[data-slot="commander-tax"]'))).toHaveTextContent('Cast 2× · tax {4}')
  })

  it.each<[number, number | undefined, string, string]>([
    [14, 21, 'plain', 'Atraxa 14/21'],
    [15, 21, 'warn', 'Atraxa 15/21'],
    [18, 21, 'danger', 'Atraxa 18/21'],
    [20, 21, 'danger', 'Atraxa 20/21'],
    [21, 21, 'lethal', 'Atraxa 21/21'],
    [12, 15, 'danger', 'Atraxa 12/15'],
    [9, 30, 'plain', 'Atraxa 9/30'],
    [30, undefined, 'plain', 'Atraxa 30'],
  ])('%i damage of %s reads %s at level %s', (damage, lethal, level, text) => {
    show((view) => {
      const seat = must(view.players[2])
      const atraxa = must(view.players[1]?.commanders?.[0]).object
      seat.commander_damage = [{ commander: atraxa, name: ATRAXA, damage }]
      seat.commander_lethal = lethal
    })
    const chip = must(pod(2).querySelector<HTMLElement>('[data-slot="commander-damage"]'))
    expect(chip).toHaveAttribute('data-level', level)
    expect(chip).toHaveTextContent(text)
    expect(chip).toHaveAccessibleName(
      `${lethal === undefined ? damage : `${damage} of ${lethal}`} commander damage from Atraxa, Praetors’ Voice${level === 'lethal' ? ', lethal' : ''}`,
    )
  })

  it.each([
    ['the server', ATRAXA, ATRAXA],
    ['the object when the server sends no name', '', ATRAXA],
  ])('names a commander the page cannot see (a hand or a library) by %s', (_, sent, shown) => {
    const table = show((view) => {
      const atraxa = must(view.players[1]?.commanders?.[0]).object
      must(view.players[2]).commander_damage = [{ commander: atraxa, name: sent, damage: 6 }]
    })
    const atraxa = must(table.store.getState().view?.players[1]?.commanders?.[0]).object
    if (sent !== '') {
      table.update((state) => {
        const objectsById = { ...state.objectsById }
        delete objectsById[atraxa]
        return { objectsById }
      })
    }
    expect(must(pod(2).querySelector('[data-slot="commander-damage"]'))).toHaveAccessibleName(
      `6 of 21 commander damage from ${shown}`,
    )
  })

  it('draws no damage chip on a seat that took none', () => {
    show()
    expect(pod(3).querySelector('[data-slot="commander-damage"]')).toBeNull()
  })

  it('draws both commanders of a partner pair in the 168 px pod of an opponent', () => {
    // Seat 0's partners seen from seat 1 across a duel, whose pods have room for cards.
    show(undefined, buildFixture({ seats: 2, viewer: 1 }))
    const list = within(pod(0)).getByRole('list', { name: "Alice's command zone" })
    expect(
      within(list)
        .getAllByRole('button')
        .map((card) => card.getAttribute('aria-label')),
    ).toEqual([expect.stringMatching(/^Tymna the Weaver/), expect.stringMatching(/^Thrasios, Triton Hero/)])
    expect(chips(pod(0), 'commander-tax')).toEqual([
      'Tymna the Weaver: cast 1 time from the command zone, tax 2 mana',
    ])
  })

  it('shows a seat lost to commander damage as such', () => {
    const fixture = must(DEV_TABLES['commander-return'])(FIXTURE)
    renderTable({ fixture, options: { viewport: { width: 1920, height: 1080 } } })
    expect(pod(3)).toHaveTextContent('Eliminated: lost to commander damage')
    expect(must(pod(3).querySelector('[data-slot="commander-damage"]'))).toHaveAttribute(
      'data-level',
      'lethal',
    )
  })
})

describe('your commanders in the hand dock', () => {
  it('lists them before the hand with the tax in their names', () => {
    show()
    const list = screen.getByRole('list', { name: "Alice's commanders in the command zone" })
    const names = within(list)
      .getAllByRole('button')
      .map((card) => card.getAttribute('aria-label') ?? '')
    expect(names[0]).toMatch(/^Tymna the Weaver.*, in the command zone, costs \{1\}\{W\}\{B\} \+ \{2\} tax/)
    expect(names[1]).toMatch(/^Thrasios, Triton Hero.*, in the command zone, costs \{G\}\{U\}|\{U\}\{G\}/)
    expect(list.querySelectorAll('[data-slot="hand-commander-tax"]')).toHaveLength(2)
  })

  it('stand in a box as high as the dock, so the divider after them never rises over the battlefield', () => {
    show()
    const box = must(document.querySelector('[data-slot="hand-command"]'))
    expect(box).toHaveClass('h-full', 'border-r')
  })

  it('marks them castable in a priority window, as the hand', () => {
    const fixture = FIXTURE
    const pending = must(DEV_PROMPTS['priority'])(fixture)
    renderTable({ fixture, state: (state) => ({ ...state, pending }) })
    const list = screen.getByRole('list', { name: "Alice's commanders in the command zone" })
    for (const card of within(list).getAllByRole('button'))
      expect(card).toHaveAttribute('data-highlight', 'legal')
  })
})
