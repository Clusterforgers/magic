// Goal: a battlefield draws equal lands as one stack with a ×N chip and opens it in place on a click, Enter or Space and closes it with its chip or Escape, the focus on the first card each time; while a prompt marks a member the members are separate, marked and clickable and the stack goes back when the prompt does; a prompt that marks none leaves it closed and dimmed; a tapped card has the box of an upright one.
import { screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { Answer, ObjectId, PriorityOptions } from '@/protocol/generated'
import { indexById } from '@/play/view'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { battlefieldStacks } from './rowStacks'
import type { StackIds } from './rowStacks'

const options = (over: Partial<PriorityOptions> = {}): PriorityOptions => ({
  playable_lands: [],
  castable: [],
  mana_abilities: [],
  can_act: false,
  can_pass: true,
  ...over,
})

type Table = ReturnType<typeof renderTable>

const stacked = (setup: Parameters<typeof renderTable>[0] = {}): Table =>
  renderTable({ seats: 2, ...setup, options: { stacks: true, ...setup.options } })

const lands = (seat: number): HTMLElement =>
  screen.getByRole('list', { name: `${seat === 0 ? 'Alice' : 'Bo'}'s lands` })
const cards = (row: HTMLElement): HTMLElement[] => [
  ...row.querySelectorAll<HTMLElement>('button[data-object-id]'),
]
const byId = (row: HTMLElement, id: ObjectId): HTMLElement | null =>
  row.querySelector<HTMLElement>(`button[data-object-id="${id}"]`)

/** Makes the first `count` untapped lands of a seat one land, so that a stack of that size is there to open. */
function alike(table: Table, seat: number, count: number): void {
  table.update((state) => {
    const view = clone(must(state.view))
    const mine = view.objects
      .filter(
        (object) =>
          object.zone === 'Battlefield' &&
          object.controller === seat &&
          object.types.includes('Land') &&
          object.permanent?.tapped === false,
      )
      .slice(0, count)
    const model = must(mine[0])
    for (const object of mine) {
      Object.assign(object, {
        name: model.name,
        subtypes: model.subtypes,
        colors: model.colors,
        abilities: model.abilities,
      })
    }
    return { view, objectsById: indexById(view.objects) }
  })
}

/** The lands of a seat that sit in stacks of at least `size`, as the selector says. */
function pileOf(table: Table, seat: number, size = 2, untapped = false): StackIds {
  const state = table.store.getState()
  const piles = battlefieldStacks(state, seat).plain.lands.filter(
    (stack) =>
      stack.ids.length >= size &&
      (!untapped || state.objectsById[must(stack.ids[0])]?.permanent?.tapped === false),
  )
  return must(piles[0], `a stack of ${size} lands`)
}

describe('equal lands', () => {
  it('are drawn as one card with a ×N chip, the other members out of the row', () => {
    const table = stacked()
    const piles = battlefieldStacks(table.store.getState(), 1).plain.lands
    const row = lands(1)
    expect(cards(row)).toHaveLength(piles.length)
    expect(piles.some((stack) => stack.ids.length > 1)).toBe(true)
    for (const stack of piles) {
      const [first, ...rest] = stack.ids
      const button = must(byId(row, must(first)))
      if (stack.ids.length === 1) {
        expect(button).not.toHaveAttribute('data-stack')
        continue
      }
      expect(button).toHaveAttribute('data-stack', String(stack.ids.length))
      expect(button).toHaveAttribute('aria-expanded', 'false')
      expect(within(button).getByText(`×${stack.ids.length}`)).toBeInTheDocument()
      expect(button).toHaveAccessibleName(expect.stringMatching(new RegExp(`^${stack.ids.length} × `)))
      for (const hidden of rest) expect(byId(row, hidden)).toBeNull()
    }
  })

  it('are all drawn when the table does not stack', () => {
    const table = renderTable({ seats: 2 })
    const grouped = battlefieldStacks(table.store.getState(), 1).plain.lands
    const members = grouped.reduce((sum, stack) => sum + stack.ids.length, 0)
    expect(members).toBeGreaterThan(grouped.length)
    expect(cards(lands(1))).toHaveLength(members)
    expect(document.querySelector('[data-stack]')).toBeNull()
  })
})

describe('a stack opened in place', () => {
  it('shows every member on a click and gives the focus to the first', async () => {
    const table = stacked()
    const pile = pileOf(table, 1)
    const row = lands(1)
    await table.user.click(must(byId(row, must(pile.ids[0]))))
    for (const id of pile.ids) expect(byId(row, id), `member ${id}`).not.toHaveAttribute('data-stack')
    expect(cards(row)).toHaveLength(cards(row).length)
    expect(byId(row, must(pile.ids[0]))).toHaveFocus()
    expect(row.querySelector(`[data-stack-open="${pile.ids.length}"]`)).not.toBeNull()
    // Opening pinned no preview: the click acted.
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it.each([['Enter'], [' ']])('opens from the keyboard with %j', async (key) => {
    const table = stacked()
    const pile = pileOf(table, 1)
    const row = lands(1)
    const top = must(byId(row, must(pile.ids[0])))
    top.focus()
    await table.user.keyboard(key === ' ' ? ' ' : `{${key}}`)
    for (const id of pile.ids) expect(byId(row, id), `member ${id}`).not.toBeNull()
    expect(byId(row, must(pile.ids[1]))).not.toHaveAttribute('data-stack')
  })

  it('closes with its chip, back to one card that has the focus', async () => {
    const table = stacked()
    const pile = pileOf(table, 1)
    const row = lands(1)
    await table.user.click(must(byId(row, must(pile.ids[0]))))
    await table.user.click(within(row).getByRole('button', { name: `Close the stack of ${pile.ids.length}` }))
    const top = must(byId(row, must(pile.ids[0])))
    expect(top).toHaveAttribute('data-stack', String(pile.ids.length))
    expect(top).toHaveFocus()
    for (const id of pile.ids.slice(1)) expect(byId(row, id)).toBeNull()
  })

  it('closes with Escape from any of its members', async () => {
    const table = stacked()
    const pile = pileOf(table, 1)
    const row = lands(1)
    await table.user.click(must(byId(row, must(pile.ids[0]))))
    must(byId(row, must(pile.ids.at(-1)))).focus()
    await table.user.keyboard('{Escape}')
    expect(must(byId(row, must(pile.ids[0])))).toHaveAttribute('data-stack', String(pile.ids.length))
    expect(byId(row, must(pile.ids.at(-1)))).toBeNull()
  })

  it('stays open when one of its members taps and leaves it', async () => {
    const table = stacked()
    alike(table, 1, 3)
    const pile = pileOf(table, 1, 3, true)
    const row = lands(1)
    await table.user.click(must(byId(row, must(pile.ids[0]))))
    const last = must(pile.ids.at(-1))
    table.update((state) => {
      const view = clone(must(state.view))
      const object = must(view.objects.find((candidate) => candidate.id === last))
      must(object.permanent).tapped = true
      return { view, objectsById: indexById(view.objects) }
    })
    // The members that stayed are still drawn one by one: the stack keeps its key while any member stays.
    for (const id of pile.ids.slice(0, -1))
      expect(byId(row, id), `member ${id}`).not.toHaveAttribute('data-stack')
  })
})

describe('a prompt that needs the members of a stack', () => {
  const tap = (object: ObjectId) => ({ source: { object, ability: 0 }, produces: 'Green' as const })

  function prompted(table: Table, given: PriorityOptions, answer = vi.fn((_answer: Answer) => true)) {
    table.update(() => ({ pending: { id: 4, decision: { player: 0, kind: { Priority: given } } } }))
    return answer
  }

  it('opens the stack, marks the members it offers and dims the rest, and closes it with the prompt', () => {
    const table = stacked()
    alike(table, 0, 3)
    const pile = pileOf(table, 0, 3, true)
    const [first, second] = pile.ids as [ObjectId, ObjectId]
    const other = must(
      battlefieldStacks(table.store.getState(), 0).plain.lands.find((stack) => stack !== pile),
    )
    const row = lands(0)
    prompted(table, options({ mana_abilities: [tap(first), tap(second)] }))
    for (const id of pile.ids) expect(byId(row, id), `member ${id}`).not.toBeNull()
    expect(byId(row, first)).toHaveAttribute('data-highlight', 'legal')
    expect(byId(row, second)).toHaveAttribute('data-highlight', 'legal')
    for (const id of pile.ids.slice(2)) expect(byId(row, id)).toHaveAttribute('data-highlight', 'dimmed')
    expect(byId(row, first)).not.toHaveAttribute('data-stack')
    // A stack the prompt marks nothing in stays one card, dimmed with the rest of the viewer.
    const closed = must(byId(row, must(other.ids[0])))
    if (other.ids.length > 1) expect(closed).toHaveAttribute('data-stack', String(other.ids.length))
    expect(closed).toHaveAttribute('data-highlight', 'dimmed')
    // The player cannot close what the prompt holds open.
    expect(within(row).queryByRole('button', { name: /^Close the stack/ })).toBeNull()
    table.update(() => ({ pending: null }))
    expect(byId(row, first)).toHaveAttribute('data-stack', String(pile.ids.length))
    expect(byId(row, second)).toBeNull()
  })

  it('answers for the member that was clicked', async () => {
    const answer = vi.fn((_answer: Answer) => true)
    const table = stacked({ session: { answer } })
    alike(table, 0, 3)
    const [a, b] = pileOf(table, 0, 3, true).ids as [ObjectId, ObjectId]
    prompted(table, options({ mana_abilities: [tap(a), tap(b)] }))
    await table.user.click(must(byId(lands(0), b)))
    expect(answer).toHaveBeenCalledTimes(1)
    expect(answer).toHaveBeenCalledWith({ Priority: { ActivateMana: { object: b, ability: 0 } } })
  })
})

describe('a tapped card', () => {
  it('has the box of an upright one and lies on its side inside it', () => {
    stacked()
    const row = screen.getByRole('list', { name: "Bo's creatures" })
    const turned = must(row.querySelector<HTMLElement>('button[data-tapped]'))
    const upright = must(row.querySelector<HTMLElement>('button[data-object-id]:not([data-tapped])'))
    expect(turned.className).toBe(turned.className.replace('w-(--card-h)', ''))
    expect(turned).toHaveClass('w-(--card-w)', 'h-(--card-h)')
    expect(upright).toHaveClass('w-(--card-w)', 'h-(--card-h)')
    expect(turned.querySelector('[data-slot="card-face"]')).toHaveAttribute('data-turned', 'true')
    expect(upright.querySelector('[data-slot="card-face"]')).not.toHaveAttribute('data-turned')
  })
})
