/** Goal: for every kind of block the options can offer (one blocker, a blocker that may block several attackers, attackers a blocker may not block, attackers on a seat that is not drawn, 2 to 12 seats) the blockers are marked legal and the rest dimmed; choosing a blocker then marks the attackers it may block, a click, Enter or Space on a card or a button of the list pairs them, both cards, the summary and the answer agree, a pair can be changed, taken back or cancelled, and Confirm blocks, No blocks and a double click answer once. */
import { act, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { BlockerOption, BlockOptions, ObjectId } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { creaturesOf, fixtureOf, ownOf, renderCombat } from '@/test-utils/combatTable'
import { powerToughness } from '../cardModel'

const objectOf = (seats: number, id: ObjectId) =>
  must(fixtureOf(seats).view.objects.find((object) => object.id === id))

interface Spec {
  name: string
  seats: number
  /** For each blocker (the viewer's n-th creature), the indices of the attackers it may block. */
  blocks: number[][]
  /** The seat the attackers belong to. */
  from?: number
}

/** Attackers are creatures of `from` (seat 1, the active seat of the fixture, by default); blockers are the viewer's. */
function optionsOf({ seats, blocks, from = 1 }: Spec): {
  options: BlockOptions
  attackers: ObjectId[]
  blockers: ObjectId[]
} {
  const attackers = creaturesOf(fixtureOf(seats), from).slice(0, 5)
  const mine = creaturesOf(fixtureOf(seats), 0)
  const blockers = mine.slice(0, blocks.length)
  return {
    attackers,
    blockers,
    options: {
      blockers: blocks.map((indices, index): BlockerOption => ({
        object: must(blockers[index]),
        can_block: indices.map((i) => must(attackers[i])),
      })),
    },
  }
}

const SPECS: Spec[] = [
  { name: 'one blocker and one attacker', seats: 2, blocks: [[0]] },
  {
    name: 'three blockers that may block all of three attackers',
    seats: 2,
    blocks: [
      [0, 1, 2],
      [0, 1, 2],
      [0, 1, 2],
    ],
  },
  {
    name: 'a blocker that may block several attackers and one that may block one',
    seats: 3,
    blocks: [[0, 1, 2, 3], [2]],
  },
  {
    name: 'blockers that may not block some attackers',
    seats: 4,
    blocks: [
      [0, 1],
      [2, 3],
      [1, 3],
    ],
  },
  { name: 'five blockers at 8 seats', seats: 8, blocks: [[0, 1], [0, 1], [2], [3, 4], [4]] },
  {
    name: 'attackers of a seat that is not drawn, at 12 seats',
    seats: 12,
    blocks: [[0, 1, 2], [1]],
    from: 5,
  },
  { name: 'attackers of the next seat, at 12 seats', seats: 12, blocks: [[0, 1, 2, 3, 4], [0], [4], [2, 3]] },
]

const block = (spec: Spec) => {
  const given = optionsOf(spec)
  const table = renderCombat({ DeclareBlockers: given.options }, { seats: spec.seats })
  const picker = (): HTMLElement => screen.getByRole('group', { name: /^Attackers .* may block$/ })
  const choices = (): HTMLElement[] =>
    within(picker())
      .getAllByRole('button')
      .filter((b) => !['Cancel', 'Remove block'].includes(b.textContent ?? ''))
  const pairs = (): HTMLElement[] => {
    const list = screen.queryByRole('list', { name: 'Your blocks' })
    return list === null ? [] : within(list).getAllByRole('listitem')
  }
  return { ...table, ...given, picker, choices, pairs }
}

describe('what is marked', () => {
  it.each(SPECS)('with $name the blockers are legal and the rest of the viewer dimmed', (spec) => {
    const { marks, options, fixture } = block(spec)
    const legal = new Set(options.blockers.map((b) => b.object))
    expect(marks()).toEqual(new Map(ownOf(fixture).map((id) => [id, legal.has(id) ? 'legal' : 'dimmed'])))
  })

  it.each(SPECS)(
    'with $name choosing a blocker marks it chosen, the attackers it may block legal and the others dimmed',
    async (spec) => {
      const { marks, card, hasCard, user, options, fixture } = block(spec)
      // The attackers of the options, and every other attacker in the combat, which are dimmed.
      const shown = new Set([
        ...options.blockers.flatMap((b) => b.can_block),
        ...fixture.view.combat.attackers.map((a) => a.object),
      ])
      for (const blocker of options.blockers) {
        await user.click(card(blocker.object))
        expect(marks().get(blocker.object)).toBe('selected')
        for (const attacker of shown) {
          if (!hasCard(attacker)) continue
          expect(marks().get(attacker), `attacker ${attacker}`).toBe(
            blocker.can_block.includes(attacker) ? 'legal' : 'dimmed',
          )
        }
        for (const other of options.blockers.filter((o) => o !== blocker)) {
          expect(marks().get(other.object)).toBe('legal')
        }
      }
    },
  )

  it('marks a blocker that may block nothing as not available', () => {
    const [first, second] = creaturesOf(fixtureOf(2), 0) as [ObjectId, ObjectId]
    const [attacker] = creaturesOf(fixtureOf(2), 1) as [ObjectId]
    const { marks } = renderCombat(
      {
        DeclareBlockers: {
          blockers: [
            { object: first, can_block: [attacker] },
            { object: second, can_block: [] },
          ],
        },
      },
      { seats: 2 },
    )
    expect(marks().get(first)).toBe('legal')
    expect(marks().get(second)).toBe('dimmed')
  })

  it('marks nothing before a blocker is chosen on the attackers', () => {
    const { marks, attackers } = block(must(SPECS[1]))
    for (const attacker of attackers) expect(marks().has(attacker)).toBe(false)
  })
})

describe('the first blocker of a table of two', () => {
  const spec = must(SPECS[1])
  const given = optionsOf(spec)
  const [b1, b2, b3] = given.blockers as [ObjectId, ObjectId, ObjectId]
  const [a1, a2, a3] = given.attackers as [ObjectId, ObjectId, ObjectId]

  it('is chosen by a click, which asks for an attacker and moves focus into the list', async () => {
    const { card, user, text, picker, choices } = block(spec)
    await user.click(card(b1))
    expect(text()).toBe(`Choose the attacker ${objectOf(2, b1).name} blocks.`)
    expect(picker()).toBeInTheDocument()
    expect(choices()).toHaveLength(3)
    expect(choices()[0]).toHaveFocus()
  })

  it('lists each attacker with its power, toughness and controller', async () => {
    const { card, user, choices } = block(spec)
    await user.click(card(b1))
    const attacker = objectOf(2, a1)
    expect(choices()[0]).toHaveTextContent(`${attacker.name} ${powerToughness(attacker)} (Bo)`)
  })

  it('is chosen by Enter or Space and cancelled by choosing it again', async () => {
    const { card, user, marks } = block(spec)
    act(() => card(b1).focus())
    await user.keyboard('{Enter}')
    expect(marks().get(b1)).toBe('selected')
    act(() => card(b1).focus())
    await user.keyboard(' ')
    expect(marks().get(b1)).toBe('legal')
    expect(screen.queryByRole('group', { name: /may block$/ })).toBeNull()
  })

  it('is cancelled by Escape and by Cancel, and focus goes back to its card', async () => {
    const { card, user, marks } = block(spec)
    await user.click(card(b1))
    await user.keyboard('{Escape}')
    expect(marks().get(b1)).toBe('legal')
    expect(card(b1)).toHaveFocus()
    await user.click(card(b1))
    await user.click(screen.getByRole('button', { name: 'Cancel' }))
    expect(marks().get(b1)).toBe('legal')
    expect(card(b1)).toHaveFocus()
  })

  it('gives the choice to another blocker when that one is chosen', async () => {
    const { card, user, marks } = block(spec)
    await user.click(card(b1))
    await user.click(card(b2))
    expect(marks().get(b1)).toBe('legal')
    expect(marks().get(b2)).toBe('selected')
    expect(screen.getAllByRole('group', { name: /may block$/ })).toHaveLength(1)
  })

  it('writes "Pick attacker" on the blocker being placed', async () => {
    const { card, user, notes } = block(spec)
    await user.click(card(b1))
    expect(notes()).toEqual(new Map([[b1, 'Pick attacker']]))
  })

  it('blocks an attacker when its card is clicked, and both cards say so', async () => {
    const { card, user, notes, marks, answer, text, pairs } = block(spec)
    await user.click(card(b1))
    await user.click(card(a2))
    expect(answer).not.toHaveBeenCalled()
    expect(marks().get(b1)).toBe('selected')
    expect(marks().get(a2)).toBeUndefined()
    expect(notes()).toEqual(
      new Map([
        [b1, `Blocks ${objectOf(2, a2).name}`],
        [a2, `Blocked by ${objectOf(2, b1).name}`],
      ]),
    )
    expect(card(b1)).toHaveAccessibleName(new RegExp(`, blocking ${objectOf(2, a2).name}, Bo's$`))
    expect(card(a2)).toHaveAccessibleName(new RegExp(`, blocked by ${objectOf(2, b1).name}$`))
    expect(text()).toBe('1 blocker on 1 attacker. Confirm the blocks, or choose another creature.')
    expect(pairs()).toHaveLength(1)
    expect(pairs()[0]).toHaveTextContent(`${objectOf(2, b1).name} blocks ${objectOf(2, a2).name} (Bo)`)
  })

  it('blocks an attacker when its button in the list is pressed, and moves focus back to the blocker', async () => {
    const { card, user, notes, choices } = block(spec)
    await user.click(card(b1))
    await user.click(must(choices()[2]))
    expect(notes().get(b1)).toBe(`Blocks ${objectOf(2, a3).name}`)
    expect(screen.queryByRole('group', { name: /may block$/ })).toBeNull()
    expect(card(b1)).toHaveFocus()
  })

  it('answers from the keyboard alone: Enter, Tab to an attacker, Enter, then Confirm blocks', async () => {
    const { card, user, answer, button } = block(spec)
    act(() => card(b2).focus())
    await user.keyboard('{Enter}')
    await user.keyboard('{Tab}{Enter}')
    expect(card(b2)).toHaveFocus()
    act(() => button('Confirm blocks').focus())
    await user.keyboard('{Enter}')
    expect(answer.mock.calls).toEqual([[{ DeclareBlockers: [[b2, a2]] }]])
  })

  it('ignores a click on an attacker before any blocker is chosen, and on a card it may not block', async () => {
    const small = block({ name: '', seats: 2, blocks: [[0], [0, 1]] })
    const [only, wide] = small.blockers as [ObjectId, ObjectId]
    const [x, y] = small.attackers as [ObjectId, ObjectId]
    await small.user.click(small.card(y))
    expect(small.notes().size).toBe(0)
    await small.user.click(small.card(only))
    await small.user.click(small.card(y))
    expect(small.notes().get(only)).toBe('Pick attacker')
    await small.user.click(small.card(x))
    expect(small.notes().get(only)).toBe(`Blocks ${objectOf(2, x).name}`)
    await small.user.click(small.card(wide))
    expect(small.notes().get(wide)).toBe('Pick attacker')
  })

  it('takes a block back when the same attacker is chosen again', async () => {
    const { card, user, notes, pairs } = block(spec)
    await user.click(card(b1))
    await user.click(card(a1))
    await user.click(card(b1))
    expect(screen.getByRole('button', { name: 'Remove block' })).toBeInTheDocument()
    await user.click(card(a1))
    expect(notes().size).toBe(0)
    expect(pairs()).toEqual([])
  })

  it('moves a block to another attacker, and the blocker is in the answer once', async () => {
    const { card, user, notes, button, answer } = block(spec)
    await user.click(card(b1))
    await user.click(card(a1))
    await user.click(card(b1))
    await user.click(card(a3))
    expect(notes().get(a1)).toBeUndefined()
    expect(notes().get(a3)).toBe(`Blocked by ${objectOf(2, b1).name}`)
    await user.click(button('Confirm blocks'))
    expect(answer.mock.calls).toEqual([[{ DeclareBlockers: [[b1, a3]] }]])
  })

  it('shows the chosen attacker as pressed in the list when the blocker is chosen again', async () => {
    const { card, user, choices } = block(spec)
    await user.click(card(b1))
    await user.click(card(a2))
    await user.click(card(b1))
    expect(choices().map((c) => c.getAttribute('aria-pressed'))).toEqual(['false', 'true', 'false'])
  })

  it('changes a block with Change, and takes it back with Remove block or Remove', async () => {
    const { card, user, notes, pairs, choices } = block(spec)
    await user.click(card(b1))
    await user.click(card(a1))
    await user.click(card(b2))
    await user.click(card(a1))
    expect(notes().get(a1)).toBe('Blocked by 2')
    await user.click(
      within(must(pairs()[0])).getByRole('button', { name: `Change the block of ${objectOf(2, b1).name}` }),
    )
    expect(choices()[0]).toHaveFocus()
    await user.click(must(choices()[1]))
    expect(notes().get(b1)).toBe(`Blocks ${objectOf(2, a2).name}`)
    expect(notes().get(a1)).toBe(`Blocked by ${objectOf(2, b2).name}`)
    await user.click(
      within(must(pairs()[1])).getByRole('button', { name: `Remove the block of ${objectOf(2, b2).name}` }),
    )
    expect(notes().has(b2)).toBe(false)
    expect(notes().has(a1)).toBe(false)
    expect(pairs()).toHaveLength(1)
    await user.click(card(b1))
    await user.click(screen.getByRole('button', { name: 'Remove block' }))
    expect(pairs()).toHaveLength(0)
    expect(b3).toBeDefined()
  })

  it('says it is a double block when two blockers take one attacker', async () => {
    const { card, user, notes, answer, button, text } = block(spec)
    await user.click(card(b1))
    await user.click(card(a1))
    await user.click(card(b2))
    await user.click(card(a1))
    expect(notes().get(a1)).toBe('Blocked by 2')
    expect(card(a1)).toHaveAccessibleName(
      new RegExp(`, blocked by ${objectOf(2, b1).name} and ${objectOf(2, b2).name}$`),
    )
    expect(text()).toBe('2 blockers on 1 attacker. Confirm the blocks, or choose another creature.')
    await user.click(button('Confirm blocks'))
    expect(answer.mock.calls).toEqual([
      [
        {
          DeclareBlockers: [
            [b1, a1],
            [b2, a1],
          ],
        },
      ],
    ])
  })

  it('sends the pairs in the order they were made', async () => {
    const { card, user, answer, button } = block(spec)
    for (const [blocker, attacker] of [
      [b3, a1],
      [b1, a3],
      [b2, a2],
    ] as const) {
      await user.click(card(blocker))
      await user.click(card(attacker))
    }
    await user.click(button('Confirm blocks'))
    expect(answer.mock.calls).toEqual([
      [
        {
          DeclareBlockers: [
            [b3, a1],
            [b1, a3],
            [b2, a2],
          ],
        },
      ],
    ])
  })
})

describe('Confirm blocks and No blocks', () => {
  const spec = must(SPECS[0])
  const { blockers, attackers } = optionsOf(spec)
  const [blocker] = blockers as [ObjectId]
  const [attacker] = attackers as [ObjectId]

  it('cannot confirm without a block, and answers an empty block with No blocks', async () => {
    const { button, user, answer } = block(spec)
    expect(button('Confirm blocks')).toBeDisabled()
    await user.click(button('No blocks'))
    expect(answer.mock.calls).toEqual([[{ DeclareBlockers: [] }]])
  })

  it('answers no blocks with No blocks even when a block was made', async () => {
    const { card, button, user, answer } = block(spec)
    await user.click(card(blocker))
    await user.click(card(attacker))
    await user.click(button('No blocks'))
    expect(answer.mock.calls).toEqual([[{ DeclareBlockers: [] }]])
  })

  it.each([
    ['Confirm blocks', async (t: ReturnType<typeof block>) => t.user.dblClick(t.button('Confirm blocks'))],
    ['No blocks', async (t: ReturnType<typeof block>) => t.user.dblClick(t.button('No blocks'))],
    [
      'Enter on Confirm blocks',
      async (t: ReturnType<typeof block>) => {
        act(() => t.button('Confirm blocks').focus())
        await t.user.keyboard('{Enter}{Enter}')
      },
    ],
  ])('sends one answer for %s pressed twice', async (_name, gesture) => {
    const table = block(spec)
    await table.user.click(table.card(blocker))
    await table.user.click(table.card(attacker))
    await gesture(table)
    expect(table.answer).toHaveBeenCalledTimes(1)
  })

  it('says so when nothing can block, and offers No blocks only', async () => {
    const { text, button, user, answer } = renderCombat({ DeclareBlockers: { blockers: [] } })
    expect(text()).toBe('None of your creatures can block. Choose No blocks to go on.')
    expect(button('Confirm blocks')).toBeDisabled()
    await user.click(button('No blocks'))
    expect(answer.mock.calls).toEqual([[{ DeclareBlockers: [] }]])
  })
})

describe('attackers of a seat that is not drawn', () => {
  const spec = must(SPECS[5])

  it('can still be blocked from the list, which names their controller', async () => {
    const { card, hasCard, user, choices, attackers, blockers, answer, button } = block(spec)
    expect(attackers.some(hasCard)).toBe(false)
    const [blocker] = blockers as [ObjectId]
    await user.click(card(blocker))
    expect(choices()).toHaveLength(3)
    for (const choice of choices()) expect(choice).toHaveTextContent(/\(Fay\)$/)
    await user.click(must(choices()[1]))
    await user.click(button('Confirm blocks'))
    expect(answer.mock.calls).toEqual([[{ DeclareBlockers: [[blocker, must(attackers[1])]] }]])
  })
})

describe('a replaced decision', () => {
  it('forgets the blocks and the chosen blocker, and marks the new options', async () => {
    const spec = must(SPECS[1])
    const { card, user, marks, notes, update, button, blockers, attackers } = block(spec)
    const [b1, b2] = blockers as [ObjectId, ObjectId]
    const [a1] = attackers as [ObjectId]
    await user.click(card(b1))
    await user.click(card(a1))
    await user.click(card(b2))
    update(() => ({
      pending: {
        id: 9,
        decision: { player: 0, kind: { DeclareBlockers: { blockers: [{ object: b2, can_block: [a1] }] } } },
      },
    }))
    expect(notes().size).toBe(0)
    expect(marks().get(b1)).toBe('dimmed')
    expect(marks().get(b2)).toBe('legal')
    expect(screen.queryByRole('group', { name: /may block$/ })).toBeNull()
    expect(screen.queryByRole('list', { name: 'Your blocks' })).toBeNull()
    expect(button('Confirm blocks')).toBeDisabled()
  })

  it('takes every mark and note away once the decision is gone', async () => {
    const { card, user, marks, notes, update, blockers, attackers } = block(must(SPECS[1]))
    await user.click(card(must(blockers[0])))
    await user.click(card(must(attackers[0])))
    expect(notes().size).toBe(2)
    update(() => ({ pending: null }))
    expect(marks().size).toBe(0)
    expect(notes().size).toBe(0)
  })
})

describe('the words', () => {
  it('says what to do before a blocker is chosen and is one live region', () => {
    const { text, container } = block(must(SPECS[0]))
    expect(text()).toBe('Choose a creature to block with, then the attacker it blocks, or choose No blocks.')
    const line = must(container.querySelector<HTMLElement>('[data-slot="decision-line"]'))
    expect(within(line).getByText('Your decision: Declare blockers')).toBeInTheDocument()
    expect(line.querySelectorAll('[aria-live]')).toHaveLength(1)
  })

  it('tells two attackers of one name apart in the list and the pairs', async () => {
    const { card, user, update, blockers, attackers, choices, pairs } = block({
      name: '',
      seats: 2,
      blocks: [[0, 1]],
    })
    const [blocker] = blockers as [ObjectId]
    const [a, b] = attackers as [ObjectId, ObjectId]
    update((state) => ({
      objectsById: {
        ...state.objectsById,
        [a]: { ...must(state.objectsById[a]), name: 'Goblin' },
        [b]: { ...must(state.objectsById[b]), name: 'Goblin' },
      },
    }))
    await user.click(card(blocker))
    expect(choices().map((c) => c.textContent?.split(' (')[0]?.replace(/ \d+\/\d+$/, ''))).toEqual([
      'Goblin 1',
      'Goblin 2',
    ])
    await user.click(must(choices()[1]))
    expect(pairs()[0]).toHaveTextContent(/blocks Goblin 2 \(Bo\)/)
  })
})
