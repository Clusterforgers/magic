/** Goal: for 2 to 6 triggers the list starts in the order the engine would take (the last to trigger on top, resolving first), Up and Down move a trigger one place and say where it went, the ends of the list have no move past them, focus stays on the button pressed, Default order puts the list back, Confirm order sends every trigger once in the order they go on the stack (the top of the list last) and sends once, and each row names its source and what it waits for and tells equal triggers apart. */
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { TriggerEntry } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { creaturesOf, fixtureOf, renderDecision } from '@/test-utils/combatTable'

const WHENS = [
  'When it enters the battlefield',
  'Whenever it attacks',
  'At the beginning of your upkeep',
  'When it dies',
  'Whenever you cast a spell',
  'When it becomes the target of a spell or ability',
]

const sources = creaturesOf(fixtureOf(2), 0)
const nameOf = (id: number): string => must(fixtureOf(2).view.objects.find((o) => o.id === id)).name

const triggers = (count: number): TriggerEntry[] =>
  Array.from({ length: count }, (_, i) => ({
    source: { id: must(sources[i]), counter: 0 },
    ability: 0,
    when: must(WHENS[i]),
  }))

const labelOf = (entry: TriggerEntry): string => `${nameOf(entry.source.id)}: ${entry.when}`

function mount(entries: TriggerEntry[]) {
  const table = renderDecision({ OrderTriggers: { triggers: entries } })
  const list = (): HTMLElement => screen.getByRole('list', { name: /^Your triggers/ })
  /** The trigger at each place of the list, top first, by its index among the options. */
  const shown = (): number[] =>
    within(list())
      .getAllByRole('listitem')
      .map((row) => Number(must(row.querySelector('button')).getAttribute('data-entry')))
  const rowOf = (entry: number): HTMLElement =>
    must(list().querySelector<HTMLElement>(`button[data-entry="${entry}"]`)).closest('li') as HTMLElement
  const move = (entry: number, way: 'up' | 'down'): HTMLElement =>
    must(list().querySelector<HTMLElement>(`button[data-entry="${entry}"][data-move="${way}"]`))
  const confirm = (): HTMLElement => screen.getByRole('button', { name: 'Confirm order' })
  const reset = (): HTMLElement => screen.getByRole('button', { name: 'Default order' })
  return { ...table, entries, list, shown, rowOf, move, confirm, reset }
}

describe('the list at first', () => {
  it.each([2, 3, 4, 6])(
    'with %i triggers the last to trigger is on top, and Confirm order sends the order they triggered',
    async (count) => {
      const { shown, rowOf, entries, confirm, answer, user, marks } = mount(triggers(count))
      const reversed = Array.from({ length: count }, (_, i) => count - 1 - i)
      expect(shown()).toEqual(reversed)
      for (const [entry, trigger] of entries.entries()) {
        expect(rowOf(entry)).toHaveTextContent(labelOf(trigger))
      }
      // Nothing on the table is clickable while the triggers are ordered.
      expect([...marks().values()]).toEqual([])
      await user.click(confirm())
      expect(answer).toHaveBeenCalledTimes(1)
      expect(answer).toHaveBeenCalledWith({ OrderTriggers: Array.from({ length: count }, (_, i) => i) })
    },
  )

  it('says in words that the top one resolves first, and marks the two ends', () => {
    const { text, rowOf } = mount(triggers(3))
    expect(text()).toBe('Order your 3 triggers: the top one resolves first. Confirm when the order is right.')
    expect(rowOf(2)).toHaveTextContent('1.')
    expect(rowOf(2)).toHaveTextContent('(resolves first)')
    expect(rowOf(0)).toHaveTextContent('3.')
    expect(rowOf(0)).toHaveTextContent('(resolves last)')
    expect(rowOf(1)).not.toHaveTextContent('(resolves')
  })

  it('has Default order disabled while the list is in the default order', () => {
    expect(mount(triggers(2)).reset()).toBeDisabled()
  })
})

describe('moving a trigger', () => {
  // The list starts [n-1 ... 0]; each case presses Up or Down on a trigger by its index among the options.
  it.each([
    ['2 triggers, the first Down', 2, [['down', 1]], [0, 1], [1, 0]],
    ['3 triggers, trigger 0 Up', 3, [['up', 0]], [2, 0, 1], [1, 0, 2]],
    ['3 triggers, trigger 2 Down', 3, [['down', 2]], [1, 2, 0], [0, 2, 1]],
    [
      '3 triggers, trigger 0 Up twice',
      3,
      [
        ['up', 0],
        ['up', 0],
      ],
      [0, 2, 1],
      [1, 2, 0],
    ],
    [
      '4 triggers, trigger 3 Down twice',
      4,
      [
        ['down', 3],
        ['down', 3],
      ],
      [2, 1, 3, 0],
      [0, 3, 1, 2],
    ],
    [
      '4 triggers, trigger 3 Down then trigger 0 Up',
      4,
      [
        ['down', 3],
        ['up', 0],
      ],
      [2, 3, 0, 1],
      [1, 0, 3, 2],
    ],
    [
      '6 triggers, the bottom one all the way up',
      6,
      [
        ['up', 0],
        ['up', 0],
        ['up', 0],
        ['up', 0],
        ['up', 0],
      ],
      [0, 5, 4, 3, 2, 1],
      [1, 2, 3, 4, 5, 0],
    ],
  ] as const)('with %s the list and the answer follow', async (_what, count, presses, list, sent) => {
    const { shown, move, confirm, answer, user } = mount(triggers(count))
    for (const [way, entry] of presses) await user.click(move(entry, way))
    expect(shown()).toEqual(list)
    await user.click(confirm())
    expect(answer).toHaveBeenCalledWith({ OrderTriggers: sent })
  })

  it('leaves no move past either end of the list, and the button there only reads as unavailable', async () => {
    const { move, user } = mount(triggers(3))
    const unavailable = (way: 'up' | 'down', entry: number): boolean =>
      move(entry, way).getAttribute('aria-disabled') === 'true'
    expect([unavailable('up', 2), unavailable('down', 0)]).toEqual([true, true])
    expect([unavailable('down', 2), unavailable('up', 0)]).toEqual([false, false])
    await user.click(move(0, 'up'))
    expect([unavailable('down', 0), unavailable('down', 1)]).toEqual([false, true])
    await user.click(move(0, 'up'))
    expect([unavailable('up', 0), unavailable('up', 2)]).toEqual([true, false])
  })

  it('says where the trigger went', async () => {
    const entries = triggers(3)
    const { text, move, user } = mount(entries)
    await user.click(move(0, 'up'))
    expect(text()).toBe(
      `Order your 3 triggers: the top one resolves first. ${labelOf(must(entries[0]))} is now 2nd of 3.`,
    )
    await user.click(move(0, 'up'))
    expect(text()).toContain(`${labelOf(must(entries[0]))} is now 1st of 3, resolves first.`)
    await user.click(move(0, 'down'))
    await user.click(move(0, 'down'))
    expect(text()).toContain('is now 3rd of 3, resolves last.')
  })

  it('keeps focus on the button pressed, also when the trigger reaches an end and the button stops working', async () => {
    const { shown, move, user } = mount(triggers(3))
    await user.click(move(0, 'up'))
    expect(move(0, 'up')).toHaveFocus()
    await user.click(move(0, 'up'))
    expect(shown()).toEqual([0, 2, 1])
    expect(move(0, 'up')).toHaveFocus()
    expect(move(0, 'up')).toHaveAttribute('aria-disabled', 'true')
    // Pressing it again changes nothing: the trigger does not bounce back the other way.
    await user.click(move(0, 'up'))
    expect(shown()).toEqual([0, 2, 1])
    expect(move(0, 'up')).toHaveFocus()
  })

  it('can be done from the keyboard: Enter on a focused Up button moves, and at the top does nothing', async () => {
    const { shown, move, user } = mount(triggers(3))
    move(1, 'up').focus()
    await user.keyboard('{Enter}')
    expect(shown()).toEqual([1, 2, 0])
    await user.keyboard('{Enter}')
    expect(shown()).toEqual([1, 2, 0])
    expect(move(1, 'up')).toHaveFocus()
  })

  it('names each button for the trigger it moves', () => {
    const entries = triggers(2)
    mount(entries)
    const first = labelOf(must(entries[0]))
    expect(screen.getByRole('button', { name: `Move ${first} up` })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: `Move ${first} down` })).toBeInTheDocument()
  })
})

describe('Default order', () => {
  it('puts the list back as the engine would take it, and the sentence with it', async () => {
    const { shown, move, reset, text, user } = mount(triggers(4))
    await user.click(move(0, 'up'))
    await user.click(move(3, 'down'))
    expect(shown()).not.toEqual([3, 2, 1, 0])
    expect(reset()).toBeEnabled()
    await user.click(reset())
    expect(shown()).toEqual([3, 2, 1, 0])
    expect(reset()).toBeDisabled()
    expect(text()).toContain('Confirm when the order is right.')
  })

  it('is what Confirm order sends after a move and a move back', async () => {
    const { move, confirm, reset, answer, user } = mount(triggers(3))
    await user.click(move(1, 'up'))
    await user.click(move(1, 'down'))
    expect(reset()).toBeDisabled()
    await user.click(confirm())
    expect(answer).toHaveBeenCalledWith({ OrderTriggers: [0, 1, 2] })
  })
})

describe('the rows', () => {
  it('number triggers that read the same in the order they triggered, so a label stays with its trigger', async () => {
    const byName = new Map<string, number[]>()
    for (const object of fixtureOf(2).view.objects) {
      if (object.zone !== 'Battlefield' || object.controller !== 0) continue
      byName.set(object.name, [...(byName.get(object.name) ?? []), object.id])
    }
    const [name, ids] = must([...byName].find(([, list]) => list.length >= 3))
    const same: TriggerEntry[] = ids
      .slice(0, 3)
      .map((id) => ({ source: { id, counter: 0 }, ability: 0, when: 'When it dies' }))
    const { shown, rowOf, move, user } = mount(same)
    for (const [entry, n] of [
      [0, 1],
      [1, 2],
      [2, 3],
    ] as const) {
      expect(rowOf(entry)).toHaveTextContent(`${name}: When it dies (${n})`)
    }
    await user.click(move(0, 'up'))
    expect(shown()).toEqual([2, 0, 1])
    expect(rowOf(0)).toHaveTextContent(`${name}: When it dies (1)`)
  })

  it('call a source the view does not hold "A card", and a trigger without words by its ability number', () => {
    const { rowOf } = mount([
      { source: { id: 999_999, counter: 0 }, ability: 1 },
      { source: { id: must(sources[0]), counter: 0 }, ability: 0, when: 'When it dies' },
    ])
    expect(rowOf(0)).toHaveTextContent('A card: ability 2')
    expect(rowOf(1)).toHaveTextContent(`${nameOf(must(sources[0]))}: When it dies`)
  })

  it('send a double click on Confirm order as one answer', async () => {
    const { confirm, answer, user } = mount(triggers(2))
    await user.dblClick(confirm())
    expect(answer).toHaveBeenCalledTimes(1)
  })
})
