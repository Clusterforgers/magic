/** Goal: a Delves hero's Satchel shows at two to four seats: every pod with one has a Satchel chip that lists its cards, the tile counts it, your own Satchel waits at the head of the hand dock with its mark and cost in the card's name, castable like the hand in a priority window, and the dock's list is named for what it holds. */
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { buildFixture } from '@/features/table/dev/fixture'
import { DEV_PROMPTS } from '@/features/table/dev/prompts'
import type { ObjectId, StateView } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { card } from '@/test-utils/cards'
import { renderTable } from '@/test-utils/tableHarness'
import { headLabel } from './libraryTop'
import { chipZones, podCounts, tileName } from './podFigures'

/** The fixture with `cards` Satchel cards for each seat of `seats`, ids from 7000 + 10 × seat. */
function withSatchels(view: StateView, seats: readonly number[], cards = 2): StateView {
  const next = structuredClone(view)
  for (const seat of seats) {
    const ids: ObjectId[] = Array.from({ length: cards }, (_, at) => 7000 + 10 * seat + at)
    next.objects.push(
      ...ids.map((id, at) =>
        card({
          id,
          owner: seat,
          controller: seat,
          zone: { Command: seat },
          permanent: null,
          name: `Loot ${seat}-${at}`,
          mana_cost: [{ Generic: 1 }, { Colored: 'Green' }],
        }),
      ),
    )
    must(next.players[seat]).satchel = ids
  }
  return next
}

function show(seats: number, owners: readonly number[], prompt = false) {
  const fixture = buildFixture({ seats })
  const view = withSatchels(fixture.view, owners)
  const withView = { ...fixture, view }
  const pending = prompt ? must(DEV_PROMPTS['priority'])(withView) : null
  return renderTable({
    fixture: withView,
    options: { viewport: { width: 1920, height: 1080 } },
    state: (state) => ({ ...state, pending }),
  })
}

const pod = (seat: number): HTMLElement =>
  must(document.querySelector<HTMLElement>(`section[data-seat="${seat}"]`), `pod ${seat}`)

describe('the figures', () => {
  it('count the Satchel as a chip zone and in the tile name, only once it holds a card', () => {
    const view = withSatchels(buildFixture({ seats: 2 }).view, [1], 3)
    const counts = podCounts(must(view.players[1]), 0)
    expect(counts.satchel).toBe(3)
    expect(chipZones(counts)).toContain('satchel')
    expect(chipZones(podCounts(must(view.players[0]), 0))).not.toContain('satchel')
    const name = tileName({
      name: 'Bo',
      life: 40,
      counts,
      active: false,
      priority: false,
      eliminated: false,
      focused: false,
    })
    expect(name).toContain('3 in the Satchel')
  })

  it.each<[string, { top?: boolean; satchel?: boolean }[], string]>([
    ['commanders and the Satchel', [{}, { satchel: true }], "Alice's commanders in the command zone and the Satchel"],
    ['the Satchel', [{ satchel: true }], "Alice's Satchel"],
    [
      'all three',
      [{}, { satchel: true }, { top: true }],
      "Alice's commanders in the command zone, the Satchel and the top card of the library",
    ],
    ['commanders', [{}], "Alice's commanders in the command zone"],
  ])('name the head of the dock for %s', (_name, entries, label) => {
    const head = new Map(entries.map((entry, at) => [at, { tax: 0, words: '', ...entry }]))
    expect(headLabel('Alice', head)).toBe(label)
  })
})

describe('at the table', () => {
  it.each([2, 3, 4])('every pod with a Satchel has its chip at %i seats', (seats) => {
    const owners = Array.from({ length: seats }, (_, seat) => seat)
    show(seats, owners)
    for (const seat of owners) {
      const chip = must(pod(seat).querySelector<HTMLElement>('[data-zone="satchel"]'), `seat ${seat}`)
      expect(chip.getAttribute('aria-label')).toMatch(/^Satchel 2 cards/)
    }
  })

  it('lists the Satchel of another seat in its dialog', async () => {
    const table = show(3, [1])
    await table.user.click(must(pod(1).querySelector<HTMLElement>('[data-zone="satchel"]')))
    const dialog = screen.getByRole('dialog')
    expect(within(dialog).getByText(/’s satchel/)).toBeInTheDocument()
    expect(within(dialog).getAllByRole('button', { name: /Loot 1-/ })).toHaveLength(2)
  })

  it('puts your Satchel at the head of the hand dock with its mark and cost', () => {
    show(2, [0])
    const list = screen.getByRole('list', { name: "Alice's commanders in the command zone and the Satchel" })
    const marked = list.querySelectorAll('[data-satchel]')
    expect(marked).toHaveLength(2)
    expect(list.querySelectorAll('[data-slot="hand-satchel"]')).toHaveLength(2)
    expect(within(list).getByRole('button', { name: /Loot 0-0.*in your Satchel, costs/ })).toBeInTheDocument()
  })

  it('marks your Satchel castable in a priority window, as the hand', () => {
    show(2, [0], true)
    const list = screen.getByRole('list', { name: "Alice's commanders in the command zone and the Satchel" })
    for (const slot of list.querySelectorAll('[data-satchel]'))
      expect(must(slot.querySelector('button'))).toHaveAttribute('data-highlight', 'legal')
  })
})
