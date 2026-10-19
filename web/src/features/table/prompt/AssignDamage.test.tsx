/** Goal: for attackers of power 0 to 10 facing one to four blockers (with toughness and damage already marked varying), the AssignDamage prompt shows one row per blocker with the default split (lethal damage in the offered order, the rest to the last), a stepper and a number box that never let the total pass the power, Confirm enabled only when everything is assigned and the reason in words when it is not, one Answer with the shares of the blockers that get damage, blockers marked on the table with the focused row selected and a click on a blocker's card moving focus to its row, all of it by keyboard alone, and nothing kept from a decision that was replaced. */
import { act, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { indexById } from '@/play/view'
import type { PlayState } from '@/play/store'
import type { Answer, DamageOptions, ObjectId } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from '../dev/fixture'

const fixture = buildFixture({ seats: 2 })
const creaturesOf = (seat: number) =>
  fixture.view.objects.filter(
    (object) =>
      object.zone === 'Battlefield' && object.controller === seat && object.types.includes('Creature'),
  )
const attacker = must(creaturesOf(0)[0])
const theirs = creaturesOf(1).map((object) => object.id)
const [b1, b2, b3, b4] = theirs as [ObjectId, ObjectId, ObjectId, ObjectId]
const BLOCKERS = [b1, b2, b3, b4]
const byId = new Map(fixture.view.objects.map((object) => [object.id, object]))
const nameOf = (id: ObjectId): string => must(byId.get(id)).name

/** What a test sets on a creature before the table is drawn. */
interface Stat {
  toughness?: number | null
  damage?: number
  name?: string
  keywords?: string[]
}

/** `state` with the creatures changed as `stats` says; the view and the by-id index stay in step. */
function withStats(state: PlayState, stats: Record<ObjectId, Stat>): PlayState {
  const view = clone(must(state.view))
  for (const object of view.objects) {
    const stat = stats[object.id]
    if (stat === undefined) continue
    if (stat.toughness !== undefined) object.toughness = stat.toughness
    if (stat.name !== undefined) object.name = stat.name
    if (stat.keywords !== undefined) object.keywords = stat.keywords
    if (stat.damage !== undefined && object.permanent !== null) object.permanent.damage = stat.damage
  }
  return { ...state, view, objectsById: indexById(view.objects) }
}

const options = (power: number, blockers: ObjectId[]): DamageOptions => ({
  attacker: attacker.id,
  power,
  blockers,
})
const statsOf = (shapes: [number, number][], blockers = BLOCKERS): Record<ObjectId, Stat> =>
  Object.fromEntries(shapes.map(([toughness, damage], index) => [blockers[index], { toughness, damage }]))

interface RenderOptions {
  stats?: Record<ObjectId, Stat>
  id?: number
}

/** The table of seat 0 facing `given`, with an `answer` spy that accepts everything. */
function renderAssign(given: DamageOptions, { stats = {}, id = 6 }: RenderOptions = {}) {
  const answer = vi.fn((_answer: Answer) => true)
  const table = renderTable({
    fixture,
    session: { answer },
    state: (state) => ({
      ...withStats(state, stats),
      pending: { id, decision: { player: 0, kind: { AssignDamage: given } } },
    }),
  })
  const card = (target: ObjectId): HTMLElement =>
    must(table.container.querySelector<HTMLElement>(`button[data-object-id="${target}"]`))
  const marks = (): Map<number, string> =>
    new Map(
      [...table.container.querySelectorAll<HTMLElement>('[data-highlight]')].map((element) => [
        Number(element.getAttribute('data-object-id')),
        must(element.getAttribute('data-highlight')),
      ]),
    )
  const input = (target: ObjectId): HTMLInputElement =>
    screen.getByRole<HTMLInputElement>('spinbutton', { name: `Damage to ${nameOf(target)}` })
  const less = (target: ObjectId): HTMLElement =>
    screen.getByRole('button', { name: `Less damage to ${nameOf(target)}` })
  const more = (target: ObjectId): HTMLElement =>
    screen.getByRole('button', { name: `More damage to ${nameOf(target)}` })
  const confirm = (): HTMLElement => screen.getByRole('button', { name: 'Confirm' })
  const amounts = (): number[] =>
    screen.getAllByRole<HTMLInputElement>('spinbutton').map((box) => Number(box.value))
  const status = (): string => must(screen.getByRole('status').textContent)
  const text = (): string => must(table.container.querySelector('[data-slot="prompt-text"]')?.textContent)
  const rows = (): HTMLElement[] => [
    ...table.container.querySelectorAll<HTMLElement>('[data-slot="damage-row"]'),
  ]
  return { ...table, answer, card, marks, input, less, more, confirm, amounts, status, text, rows }
}

const sharesOf = (blockers: ObjectId[], amounts: number[]): Answer => ({
  AssignDamage: blockers.flatMap((target, index): [ObjectId, number][] => {
    const amount = amounts[index] ?? 0
    return amount > 0 ? [[target, amount]] : []
  }),
})

/** [what, power, [toughness, damage marked] of each blocker, the default split]. */
const DEFAULTS: [string, number, [number, number][], number[]][] = [
  ['one blocker', 3, [[2, 0]], [3]],
  ['one blocker and no damage', 0, [[2, 0]], [0]],
  [
    'two blockers, the last takes the rest',
    5,
    [
      [2, 0],
      [2, 0],
    ],
    [2, 3],
  ],
  [
    'two blockers and exactly lethal',
    4,
    [
      [2, 0],
      [2, 0],
    ],
    [2, 2],
  ],
  [
    'two blockers and less than the first lethal',
    1,
    [
      [3, 0],
      [3, 0],
    ],
    [1, 0],
  ],
  [
    'two blockers and no damage',
    0,
    [
      [2, 0],
      [2, 0],
    ],
    [0, 0],
  ],
  [
    'two blockers, power 10',
    10,
    [
      [5, 0],
      [5, 0],
    ],
    [5, 5],
  ],
  [
    'two big blockers',
    6,
    [
      [7, 0],
      [7, 0],
    ],
    [6, 0],
  ],
  [
    'two blockers, damage already marked',
    3,
    [
      [4, 3],
      [4, 0],
    ],
    [1, 2],
  ],
  [
    'a blocker that already has lethal damage',
    4,
    [
      [3, 3],
      [3, 0],
    ],
    [0, 4],
  ],
  [
    'three blockers',
    7,
    [
      [2, 0],
      [3, 1],
      [4, 0],
    ],
    [2, 2, 3],
  ],
  [
    'three blockers and a power that ends in the middle',
    3,
    [
      [2, 0],
      [2, 0],
      [2, 0],
    ],
    [2, 1, 0],
  ],
  [
    'four blockers',
    10,
    [
      [1, 0],
      [1, 0],
      [1, 0],
      [1, 0],
    ],
    [1, 1, 1, 7],
  ],
  [
    'four blockers and lots of toughness',
    9,
    [
      [4, 0],
      [4, 0],
      [4, 0],
      [4, 0],
    ],
    [4, 4, 1, 0],
  ],
]

describe('the first split', () => {
  it.each(DEFAULTS)('with %s is the default, ready to send', async (_what, power, shapes, expected) => {
    const blockers = BLOCKERS.slice(0, shapes.length)
    const { amounts, confirm, status, answer, user } = renderAssign(options(power, blockers), {
      stats: statsOf(shapes),
    })
    expect(amounts()).toEqual(expected)
    expect(confirm()).toBeEnabled()
    expect(status()).toBe(
      power === 0 ? 'There is no damage to assign.' : `Assigned ${power} of ${power}. Ready to confirm.`,
    )
    await user.click(confirm())
    expect(answer.mock.calls).toEqual([[sharesOf(blockers, expected)]])
  })

  it.each([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10])(
    'adds up to a power of %i for one to four blockers of toughness 2',
    (power) => {
      for (const count of [1, 2, 3, 4]) {
        const { amounts, confirm, unmount } = renderAssign(options(power, BLOCKERS.slice(0, count)), {
          stats: statsOf(Array.from({ length: count }, (): [number, number] => [2, 0])),
        })
        expect(amounts()).toHaveLength(count)
        expect(amounts().reduce((sum, amount) => sum + amount, 0)).toBe(power)
        expect(confirm()).toBeEnabled()
        unmount()
      }
    },
  )

  it('has one row per blocker, in the offered order, each in a list', () => {
    const { rows } = renderAssign(options(5, [b3, b1, b2]))
    expect(rows().map((row) => row.getAttribute('data-object-id'))).toEqual([b3, b1, b2].map(String))
    expect(screen.getByRole('list', { name: 'Damage for each blocker' })).toBeInTheDocument()
  })
})

describe('the words', () => {
  it('say whose damage is divided and among whom', () => {
    const { text } = renderAssign(options(5, [b1, b2]))
    expect(text()).toBe(
      `Divide the 5 combat damage of ${attacker.name} among its blockers: ${nameOf(b1)} and ${nameOf(b2)}.`,
    )
  })

  it('list three blockers with commas', () => {
    const { text } = renderAssign(options(5, [b1, b2, b3]))
    expect(text()).toBe(
      `Divide the 5 combat damage of ${attacker.name} among its blockers: ${nameOf(b1)}, ${nameOf(b2)} and ${nameOf(b3)}.`,
    )
  })

  it('say "to its blocker" for one', () => {
    const { text } = renderAssign(options(3, [b1]))
    expect(text()).toBe(`Assign the 3 combat damage of ${attacker.name} to its blocker: ${nameOf(b1)}.`)
  })

  it('give each row its toughness, damage marked and lethal damage', () => {
    const { rows } = renderAssign(options(5, [b1, b2]), {
      stats: statsOf([
        [4, 1],
        [3, 0],
      ]),
    })
    expect(rows().map((row) => row.querySelector('[data-slot="damage-facts"]')?.textContent)).toEqual([
      'Toughness 4, 1 damage marked, lethal: 3',
      'Toughness 3, lethal: 3',
    ])
  })

  it('say how far each blocker is from lethal, and when it is lethal', async () => {
    const { rows, less, user } = renderAssign(options(5, [b1, b2]), {
      stats: statsOf([
        [2, 0],
        [4, 0],
      ]),
    })
    const notes = (): (string | undefined)[] =>
      rows().map((row) => row.querySelector('[data-slot="damage-lethal"]')?.textContent)
    expect(notes()).toEqual(['Lethal', '1 short of lethal'])
    await user.click(less(b1))
    expect(notes()).toEqual(['1 short of lethal', '1 short of lethal'])
  })

  it('tell two blockers of one name apart', () => {
    const { text } = renderAssign(options(4, [b1, b2]), {
      stats: { [b1]: { name: 'Grizzly Bears' }, [b2]: { name: 'Grizzly Bears' } },
    })
    expect(text()).toContain('Grizzly Bears (1 of 2) and Grizzly Bears (2 of 2)')
    expect(screen.getByRole('spinbutton', { name: 'Damage to Grizzly Bears (2 of 2)' })).toBeInTheDocument()
  })

  it('say there is nobody to assign to when there are no blockers, and Confirm cannot be pressed', () => {
    const { text, status, confirm } = renderAssign(options(3, []))
    expect(text()).toBe(`${attacker.name} has no blocker to assign its 3 combat damage to.`)
    expect(status()).toBe('There is nothing to assign this damage to.')
    expect(confirm()).toBeDisabled()
  })

  it('cope with creatures the table does not know', () => {
    const { text, amounts, rows } = renderAssign({ attacker: 9000, power: 4, blockers: [9001, 9002] })
    expect(text()).toBe(
      'Divide the 4 combat damage of the attacker among its blockers: Blocker (1 of 2) and Blocker (2 of 2).',
    )
    expect(amounts()).toEqual([0, 4])
    expect(rows().map((row) => row.querySelector('[data-slot="damage-facts"]')?.textContent)).toEqual([
      'No toughness',
      'No toughness',
    ])
  })

  it('are announced as one polite sentence that does not change while the numbers do', async () => {
    const { text, less, user, container } = renderAssign(options(5, [b1, b2]))
    const before = text()
    await user.click(less(b2))
    expect(text()).toBe(before)
    const line = must(container.querySelector<HTMLElement>('[data-slot="decision-line"]'))
    expect(within(line).getByText('Your decision: Assign combat damage')).toBeInTheDocument()
    expect(line.querySelector('[data-slot="prompt-text"]')).toHaveAttribute('aria-live', 'polite')
  })
})

describe('the stepper', () => {
  const given = options(5, [b1, b2])
  const shapes = statsOf([
    [2, 0],
    [2, 0],
  ])

  it('moves damage from one blocker to the other with a less and a more', async () => {
    const { amounts, less, more, confirm, status, user } = renderAssign(given, { stats: shapes })
    expect(amounts()).toEqual([2, 3])
    await user.click(less(b2))
    expect(amounts()).toEqual([2, 2])
    expect(confirm()).toBeDisabled()
    expect(status()).toBe('Assigned 4 of 5. Assign 1 more to confirm.')
    await user.click(more(b1))
    expect(amounts()).toEqual([3, 2])
    expect(confirm()).toBeEnabled()
    expect(status()).toBe('Assigned 5 of 5. Ready to confirm.')
  })

  it('cannot give more than the attacker has left, and says so on the button', async () => {
    const { amounts, more, user } = renderAssign(given, { stats: shapes })
    expect(more(b1)).toHaveAttribute('aria-disabled', 'true')
    await user.click(more(b1))
    await user.click(more(b2))
    expect(amounts()).toEqual([2, 3])
  })

  it('cannot go below zero', async () => {
    const { amounts, less, user } = renderAssign(options(2, [b1, b2]), { stats: shapes })
    expect(amounts()).toEqual([2, 0])
    expect(less(b2)).toHaveAttribute('aria-disabled', 'true')
    await user.click(less(b2))
    expect(amounts()).toEqual([2, 0])
  })

  it('shows how much is left to assign when everything is taken away', async () => {
    const { amounts, less, confirm, status, user } = renderAssign(given, { stats: shapes })
    for (const target of [b1, b1, b2, b2, b2]) await user.click(less(target))
    expect(amounts()).toEqual([0, 0])
    expect(status()).toBe('Assigned 0 of 5. Assign 5 more to confirm.')
    expect(confirm()).toBeDisabled()
  })

  it('keeps the button reachable by keyboard at its limit instead of disabling it', () => {
    const { more } = renderAssign(given, { stats: shapes })
    expect(more(b1)).not.toBeDisabled()
  })

  it('does not send an answer that is not legal, whatever is pressed', async () => {
    const { answer, less, confirm, user } = renderAssign(given, { stats: shapes })
    await user.click(less(b1))
    await user.click(confirm())
    act(() => confirm().focus())
    await user.keyboard('{Enter} ')
    expect(answer).not.toHaveBeenCalled()
  })
})

describe('the number box', () => {
  const given = options(5, [b1, b2])
  const shapes = statsOf([
    [2, 0],
    [2, 0],
  ])
  // [what is typed into the first row, the split that results]
  const TYPED: [string, number[]][] = [
    ['0', [0, 3]],
    ['1', [1, 3]],
    ['2', [2, 3]],
    ['3', [2, 3]],
    ['9', [2, 3]],
    ['', [0, 3]],
  ]

  it.each(TYPED)(
    'keeps what is typed in the first row (%j) within the power: %j',
    async (typed, expected) => {
      const { amounts, input, user } = renderAssign(given, { stats: shapes })
      await user.clear(input(b1))
      if (typed !== '') await user.type(input(b1), typed)
      expect(amounts()).toEqual(expected)
    },
  )

  it('takes a number that fits after the others are lowered', async () => {
    const { amounts, input, user, confirm, status } = renderAssign(given, { stats: shapes })
    await user.clear(input(b2))
    await user.type(input(b2), '1')
    expect(amounts()).toEqual([2, 1])
    expect(status()).toBe('Assigned 3 of 5. Assign 2 more to confirm.')
    await user.clear(input(b1))
    await user.type(input(b1), '4')
    expect(amounts()).toEqual([4, 1])
    expect(confirm()).toBeEnabled()
  })

  it('allows no more than the most the row can take, and no less than 0', () => {
    const { input } = renderAssign(given, { stats: shapes })
    expect(input(b1)).toHaveAttribute('min', '0')
    expect(input(b1)).toHaveAttribute('max', '2')
    expect(input(b2)).toHaveAttribute('max', '3')
  })

  it('selects the number when the box is focused, so typing replaces it', async () => {
    const { input, user } = renderAssign(given, { stats: shapes })
    const select = vi.spyOn(input(b1), 'select')
    await user.click(input(b1))
    expect(input(b1)).toHaveFocus()
    expect(select).toHaveBeenCalledTimes(1)
  })

  it.each([
    ['ArrowDown', '{ArrowDown}', b2, [2, 2]],
    ['ArrowUp when nothing is left', '{ArrowUp}', b2, [2, 3]],
    ['ArrowUp', '{ArrowUp}', b1, [2, 3]],
    ['ArrowDown on the first row', '{ArrowDown}', b1, [1, 3]],
  ])('%s in a box steps its number', async (_what, key, target, expected) => {
    const { amounts, input, user } = renderAssign(given, { stats: shapes })
    act(() => input(target).focus())
    await user.keyboard(key)
    expect(amounts()).toEqual(expected)
  })

  it('goes up by one after a step down, within the power', async () => {
    const { amounts, input, user } = renderAssign(given, { stats: shapes })
    act(() => input(b2).focus())
    await user.keyboard('{ArrowDown}{ArrowDown}{ArrowDown}{ArrowDown}{ArrowDown}')
    expect(amounts()).toEqual([2, 0])
    act(() => input(b1).focus())
    await user.keyboard('{ArrowUp}{ArrowUp}{ArrowUp}{ArrowUp}')
    expect(amounts()).toEqual([5, 0])
  })
})

describe('the keyboard', () => {
  const given = options(5, [b1, b2])
  const shapes = statsOf([
    [2, 0],
    [2, 0],
  ])

  it('reaches every control with Tab, a row at a time, and Confirm last', async () => {
    const { user, less } = renderAssign(given, { stats: shapes })
    act(() => less(b1).focus())
    const seen: string[] = []
    for (let step = 0; step < 6; step += 1) {
      await user.tab()
      const element = document.activeElement
      seen.push(element?.getAttribute('aria-label') ?? element?.textContent ?? '')
    }
    expect(seen).toEqual([
      `Damage to ${nameOf(b1)}`,
      `More damage to ${nameOf(b1)}`,
      `Less damage to ${nameOf(b2)}`,
      `Damage to ${nameOf(b2)}`,
      `More damage to ${nameOf(b2)}`,
      'Confirm',
    ])
  })

  it('answers without a mouse: move one point with the arrows, then Tab to Confirm and press Enter', async () => {
    const { answer, user, input, more } = renderAssign(given, { stats: shapes })
    act(() => input(b2).focus())
    await user.keyboard('{ArrowDown}')
    await user.tab({ shift: true })
    await user.tab({ shift: true })
    expect(more(b1)).toHaveFocus()
    await user.keyboard('{Enter}')
    expect(more(b1)).toHaveFocus()
    for (let step = 0; step < 4; step += 1) await user.tab()
    expect(screen.getByRole('button', { name: 'Confirm' })).toHaveFocus()
    await user.keyboard('{Enter}')
    expect(answer.mock.calls).toEqual([[sharesOf([b1, b2], [3, 2])]])
  })

  it('sends the default with Enter in a number box, when it is legal', async () => {
    const { answer, user, input } = renderAssign(given, { stats: shapes })
    act(() => input(b1).focus())
    await user.keyboard('{Enter}')
    expect(answer.mock.calls).toEqual([[sharesOf([b1, b2], [2, 3])]])
  })

  it('sends nothing with Enter in a number box while the split is not legal', async () => {
    const { answer, user, input } = renderAssign(given, { stats: shapes })
    await user.clear(input(b1))
    await user.keyboard('{Enter}')
    expect(answer).not.toHaveBeenCalled()
  })

  it('presses Confirm with Space too', async () => {
    const { answer, user, confirm } = renderAssign(given, { stats: shapes })
    act(() => confirm().focus())
    await user.keyboard(' ')
    expect(answer).toHaveBeenCalledTimes(1)
  })

  it('has the reason on Confirm for a screen reader', () => {
    const { confirm, status } = renderAssign(given, { stats: shapes })
    const described = must(confirm().getAttribute('aria-describedby'))
    expect(document.getElementById(described)).toHaveTextContent(status())
  })
})

describe('what is sent', () => {
  it.each([
    [
      'one click on Confirm',
      async (user: ReturnType<typeof renderAssign>['user'], el: HTMLElement) => user.click(el),
    ],
    [
      'a double click',
      async (user: ReturnType<typeof renderAssign>['user'], el: HTMLElement) => user.dblClick(el),
    ],
    [
      'three clicks',
      async (user: ReturnType<typeof renderAssign>['user'], el: HTMLElement) => user.tripleClick(el),
    ],
    [
      'Enter twice',
      async (_user: ReturnType<typeof renderAssign>['user'], el: HTMLElement) => {
        act(() => el.focus())
        await _user.keyboard('{Enter}{Enter}')
      },
    ],
  ])('is one Answer for %s', async (_how, gesture) => {
    const { answer, confirm, user } = renderAssign(options(5, [b1, b2]), {
      stats: statsOf([
        [2, 0],
        [2, 0],
      ]),
    })
    await gesture(user, confirm())
    expect(answer.mock.calls).toEqual([[sharesOf([b1, b2], [2, 3])]])
  })

  it('names only the blockers that get damage, with the amounts of the stepper', async () => {
    const { answer, less, user, confirm } = renderAssign(options(6, [b1, b2, b3]), {
      stats: statsOf([
        [2, 0],
        [2, 0],
        [2, 0],
      ]),
    })
    // The default is [2, 2, 2]; move all of the last one's damage to the first.
    await user.click(less(b3))
    await user.click(less(b3))
    expect(screen.getByRole('button', { name: 'Confirm' })).toBeDisabled()
    for (let times = 0; times < 2; times += 1)
      await user.click(screen.getByRole('button', { name: `More damage to ${nameOf(b1)}` }))
    await user.click(confirm())
    expect(answer.mock.calls).toEqual([
      [
        {
          AssignDamage: [
            [b1, 4],
            [b2, 2],
          ],
        },
      ],
    ])
  })

  it('sends an empty assignment for an attacker with no power', async () => {
    const { answer, user, confirm } = renderAssign(options(0, [b1, b2]))
    await user.click(confirm())
    expect(answer.mock.calls).toEqual([[{ AssignDamage: [] }]])
  })

  it('can be sent again when the connection refused it', async () => {
    const answer = vi.fn((_answer: Answer) => false)
    const table = renderTable({
      fixture,
      session: { answer },
      state: (state) => ({
        ...state,
        pending: { id: 6, decision: { player: 0, kind: { AssignDamage: options(4, [b1, b2]) } } },
      }),
    })
    await table.user.click(screen.getByRole('button', { name: 'Confirm' }))
    await table.user.click(screen.getByRole('button', { name: 'Confirm' }))
    expect(answer).toHaveBeenCalledTimes(2)
  })
})

describe('the table while deciding', () => {
  const given = options(5, [b1, b2, b3])
  const shapes = statsOf([
    [2, 0],
    [2, 0],
    [2, 0],
  ])

  it('marks the blockers legal and nothing else, not even the attacker or the hand', () => {
    const { marks } = renderAssign(given, { stats: shapes })
    expect(marks()).toEqual(
      new Map([
        [b1, 'legal'],
        [b2, 'legal'],
        [b3, 'legal'],
      ]),
    )
  })

  it('marks the blocker of the row in use as selected, and only that one', async () => {
    const { marks, input, user, card } = renderAssign(given, { stats: shapes })
    await user.click(input(b2))
    expect(marks().get(b2)).toBe('selected')
    expect(marks().get(b1)).toBe('legal')
    expect(card(b2)).toHaveAttribute('aria-pressed', 'true')
    expect(card(b1)).toHaveAttribute('aria-pressed', 'false')
    await user.click(input(b3))
    expect(marks().get(b2)).toBe('legal')
    expect(marks().get(b3)).toBe('selected')
  })

  it('marks a row as in use while the focus is in it, by its buttons too', async () => {
    const { rows, less, marks, user } = renderAssign(given, { stats: shapes })
    expect(rows().map((row) => row.getAttribute('data-active'))).toEqual(['false', 'false', 'false'])
    await user.click(less(b2))
    expect(rows().map((row) => row.getAttribute('data-active'))).toEqual(['false', 'true', 'false'])
    expect(marks().get(b2)).toBe('selected')
  })

  it('takes the selected mark off when the focus leaves the rows, and not when it moves inside one', async () => {
    const { marks, input, more, confirm, user } = renderAssign(given, { stats: shapes })
    await user.click(input(b3))
    await user.tab()
    expect(more(b3)).toHaveFocus()
    expect(marks().get(b3)).toBe('selected')
    await user.tab()
    expect(confirm()).toHaveFocus()
    expect(marks().get(b3)).toBe('legal')
  })

  it.each([b1, b2, b3])('moves focus to the row of blocker %i when its card is clicked', async (target) => {
    const { card, input, user, marks, answer } = renderAssign(given, { stats: shapes })
    await user.click(card(target))
    expect(input(target)).toHaveFocus()
    expect(marks().get(target)).toBe('selected')
    expect(answer).not.toHaveBeenCalled()
  })

  it.each([
    ['Enter', '{Enter}'],
    ['Space', ' '],
  ])('moves focus to the row on %s on the card', async (_key, keys) => {
    const { card, input, user } = renderAssign(given, { stats: shapes })
    act(() => card(b3).focus())
    await user.keyboard(keys)
    expect(input(b3)).toHaveFocus()
  })

  it('lets a click on a second card take the focus from the first row', async () => {
    const { card, input, user, marks } = renderAssign(given, { stats: shapes })
    await user.click(card(b1))
    await user.click(card(b2))
    expect(input(b2)).toHaveFocus()
    expect(marks().get(b1)).toBe('legal')
    expect(marks().get(b2)).toBe('selected')
  })

  it('does not change the split when a card is clicked', async () => {
    const { card, amounts, user } = renderAssign(given, { stats: shapes })
    const before = amounts()
    await user.click(card(b1))
    await user.click(card(b1))
    expect(amounts()).toEqual(before)
  })

  it('does nothing when a card that is not a blocker is clicked', async () => {
    const { card, answer, user, marks } = renderAssign(given, { stats: shapes })
    await user.click(card(must(creaturesOf(1)[5]?.id)))
    expect(answer).not.toHaveBeenCalled()
    expect(marks().size).toBe(3)
  })

  it('takes the marks away with the prompt', () => {
    const { marks, update } = renderAssign(given, { stats: shapes })
    expect(marks().size).toBe(3)
    update(() => ({ pending: null }))
    expect(marks().size).toBe(0)
  })
})

describe('an attacker with trample', () => {
  const trample = (power: number, blockers: ObjectId[], lethal?: number[]): DamageOptions => ({
    ...options(power, blockers),
    excess: { Player: 1 },
    ...(lethal === undefined ? {} : { lethal }),
  })
  const toughTwo = statsOf([
    [2, 0],
    [2, 0],
  ])

  it('starts at lethal damage to each blocker and names who takes the rest', async () => {
    const { amounts, confirm, status, answer, user, text, container } = renderAssign(
      trample(6, [b1, b2], [2, 2]),
      { stats: toughTwo },
    )
    expect(amounts()).toEqual([2, 2])
    expect(confirm()).toBeEnabled()
    expect(status()).toBe('Assigned 4 of 6. The other 2 goes to Bo. Ready to confirm.')
    expect(container.querySelector('[data-slot="damage-excess"]')).toHaveTextContent('Bo takes the rest: 2.')
    expect(text()).toBe(
      `Divide the 6 combat damage of ${attacker.name} among its blockers: ${nameOf(b1)} and ${nameOf(b2)}. Damage not given to a blocker goes to Bo, once every blocker has lethal damage.`,
    )
    await user.click(confirm())
    expect(answer.mock.calls).toEqual([[sharesOf([b1, b2], [2, 2])]])
  })

  it('does not let the rest go past a blocker that is short of lethal damage, and says what to do', async () => {
    const { amounts, confirm, status, less, user, answer, container } = renderAssign(
      trample(6, [b1, b2], [2, 2]),
      { stats: toughTwo },
    )
    await user.click(less(b1))
    expect(amounts()).toEqual([1, 2])
    expect(confirm()).toBeDisabled()
    expect(status()).toBe(
      'Assigned 3 of 6. Give every blocker its lethal damage to send the rest to Bo, or assign 3 more.',
    )
    expect(container.querySelector('[data-slot="damage-excess"]')).toHaveTextContent('Bo takes the rest: 3.')
    await user.click(confirm())
    expect(answer).not.toHaveBeenCalled()
  })

  it('may give everything to the blockers, and then there is no rest', async () => {
    const { amounts, more, confirm, status, user, container } = renderAssign(trample(6, [b1, b2], [2, 2]), {
      stats: toughTwo,
    })
    await user.click(more(b1))
    await user.click(more(b2))
    expect(amounts()).toEqual([3, 3])
    expect(status()).toBe('Assigned 6 of 6. Ready to confirm.')
    expect(confirm()).toBeEnabled()
    expect(container.querySelector('[data-slot="damage-excess"]')).toHaveTextContent('Bo takes the rest: 0.')
  })

  it('names a planeswalker that is attacked', () => {
    const { container, status } = renderAssign(
      { ...trample(5, [b1], [2]), excess: { Object: b3 } },
      {
        stats: statsOf([[2, 0]]),
      },
    )
    expect(container.querySelector('[data-slot="damage-excess"]')).toHaveTextContent(
      `${nameOf(b3)} takes the rest: 3.`,
    )
    expect(status()).toBe(`Assigned 2 of 5. The other 3 goes to ${nameOf(b3)}. Ready to confirm.`)
  })

  it('takes the lethal damage the engine states over what the creature shows', () => {
    const { amounts, rows, status } = renderAssign(trample(8, [b1, b2], [1, 1]), {
      stats: statsOf([
        [5, 0],
        [5, 0],
      ]),
    })
    expect(amounts()).toEqual([1, 1])
    expect(rows().map((row) => row.querySelector('[data-slot="damage-facts"]')?.textContent)).toEqual([
      'Toughness 5, lethal: 1',
      'Toughness 5, lethal: 1',
    ])
    expect(status()).toBe('Assigned 2 of 8. The other 6 goes to Bo. Ready to confirm.')
  })

  it('works lethal damage out from deathtouch when the engine does not state it, and names the keyword', () => {
    const { amounts, rows, status } = renderAssign(trample(8, [b1, b2]), {
      stats: {
        ...statsOf([
          [5, 0],
          [5, 1],
        ]),
        [attacker.id]: { keywords: ['Trample', 'Deathtouch'] },
      },
    })
    expect(amounts()).toEqual([1, 1])
    expect(rows().map((row) => row.querySelector('[data-slot="damage-facts"]')?.textContent)).toEqual([
      'Toughness 5, lethal: 1 (deathtouch)',
      'Toughness 5, 1 damage marked, lethal: 1 (deathtouch)',
    ])
    expect(status()).toBe('Assigned 2 of 8. The other 6 goes to Bo. Ready to confirm.')
  })

  it('sends only the blockers’ shares: what is left for the player is not in the answer', async () => {
    const { answer, user, confirm } = renderAssign(trample(5, [b1], [2]), { stats: statsOf([[2, 0]]) })
    await user.click(confirm())
    expect(answer.mock.calls).toEqual([[sharesOf([b1], [2])]])
  })

  it('draws no line for the rest when the attacker has no trample', () => {
    const { container, text } = renderAssign(options(5, [b1, b2]), { stats: toughTwo })
    expect(container.querySelector('[data-slot="damage-excess"]')).toBeNull()
    expect(text()).not.toContain('Damage not given to a blocker')
  })
})

describe('a decision that is replaced', () => {
  it('starts again from the default of the new options, with no selection and no edits', async () => {
    const { update, amounts, less, user, marks, rows } = renderAssign(options(5, [b1, b2]), {
      stats: statsOf([
        [2, 0],
        [2, 0],
      ]),
    })
    await user.click(less(b2))
    expect(amounts()).toEqual([2, 2])
    expect(marks().get(b2)).toBe('selected')
    update(() => ({
      pending: { id: 7, decision: { player: 0, kind: { AssignDamage: options(4, [b1, b2, b3]) } } },
    }))
    expect(amounts()).toEqual([2, 2, 0])
    expect(rows().map((row) => row.getAttribute('data-active'))).toEqual(['false', 'false', 'false'])
    expect(marks()).toEqual(
      new Map([
        [b1, 'legal'],
        [b2, 'legal'],
        [b3, 'legal'],
      ]),
    )
  })

  it('can be answered once again, as it is a new decision', async () => {
    const { update, answer, user } = renderAssign(options(4, [b1, b2]), {
      stats: statsOf([
        [2, 0],
        [2, 0],
      ]),
    })
    await user.click(screen.getByRole('button', { name: 'Confirm' }))
    expect(answer).toHaveBeenCalledTimes(1)
    update(() => ({
      pending: { id: 8, decision: { player: 0, kind: { AssignDamage: options(3, [b1, b2]) } } },
    }))
    await user.click(screen.getByRole('button', { name: 'Confirm' }))
    expect(answer.mock.calls.at(-1)).toEqual([sharesOf([b1, b2], [2, 1])])
  })

  it('shows no prompt for the decision of another seat', () => {
    const { container, update, marks } = renderAssign(options(4, [b1, b2]))
    update(() => ({
      pending: { id: 9, decision: { player: 1, kind: { AssignDamage: options(4, [b1, b2]) } } },
    }))
    expect(container.querySelector('[data-decision="AssignDamage"]')).toBeNull()
    expect(marks().size).toBe(0)
  })
})

describe('the creatures change while the prompt is open', () => {
  const given = options(5, [b1, b2])

  it('follows the default until a number has been changed', () => {
    const { amounts, store } = renderAssign(given, {
      stats: statsOf([
        [2, 0],
        [2, 0],
      ]),
    })
    expect(amounts()).toEqual([2, 3])
    act(() =>
      store.setState((state) =>
        withStats(
          state,
          statsOf([
            [4, 1],
            [2, 0],
          ]),
        ),
      ),
    )
    expect(amounts()).toEqual([3, 2])
  })

  it('keeps what the player chose after that', async () => {
    const { amounts, store, less, user } = renderAssign(given, {
      stats: statsOf([
        [2, 0],
        [2, 0],
      ]),
    })
    await user.click(less(b2))
    act(() =>
      store.setState((state) =>
        withStats(
          state,
          statsOf([
            [4, 1],
            [2, 0],
          ]),
        ),
      ),
    )
    expect(amounts()).toEqual([2, 2])
  })
})
