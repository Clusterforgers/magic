/** Goal: for every kind of block the options can offer (one blocker, a blocker that may block several attackers, attackers a blocker may not block, attackers on a seat that is not drawn, 2 to 12 seats) the blockers are marked legal and the rest dimmed; choosing a blocker then marks the attackers it may block, a click, Enter or Space on a card or a button of the list pairs them, both cards, the summary and the answer agree, a pair can be changed, taken back or cancelled, and Confirm blocks, No blocks and a double click answer once. */
import { act, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { BlockerOption, BlockOptions, ObjectId } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { creaturesOf, fixtureOf, ownOf, renderCombat } from '@/test-utils/combatTable'
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
  // The attackers the chosen blocker may block, as the bar lists them for a seat whose cards are not drawn.
  const picker = (): HTMLElement => screen.getByRole('combobox', { name: /^Attacker .* blocks$/ })
  const choices = (): HTMLOptionElement[] =>
    within(picker())
      .getAllByRole<HTMLOptionElement>('option')
      .filter((option) => option.value !== '')
  return { ...table, ...given, picker, choices }
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
      // A blocker that may block one attacker blocks it with its click, so it is never chosen.
      for (const blocker of options.blockers.filter((b) => b.can_block.length > 1)) {
        await user.click(card(blocker.object))
        expect(marks().get(blocker.object)).toBe('selected')
        for (const attacker of shown) {
          if (!hasCard(attacker)) continue
          expect(marks().get(attacker), `attacker ${attacker}`).toBe(
            blocker.can_block.includes(attacker) ? 'legal' : 'dimmed',
          )
        }
        for (const other of options.blockers.filter((o) => o !== blocker && o.can_block.length > 1)) {
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
    const { card, hasCard, user, choices, picker, attackers, blockers, answer, button } = block(spec)
    expect(attackers.some(hasCard)).toBe(false)
    const [blocker] = blockers as [ObjectId]
    await user.click(card(blocker))
    expect(choices()).toHaveLength(3)
    for (const choice of choices()) expect(choice).toHaveTextContent(/\(Fay\)$/)
    await user.selectOptions(picker(), must(choices()[1]))
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
    expect(screen.queryByRole('combobox', { name: /^Attacker .* blocks$/ })).toBeNull()
    expect(button('Confirm blocks')).toBeDisabled()
  })

  it('takes every mark and note away once the decision is gone', async () => {
    const { card, user, marks, notes, update, blockers, attackers } = block(must(SPECS[1]))
    await user.click(card(must(blockers[0])))
    await user.click(card(must(attackers[0])))
    expect(card(must(attackers[0]))).toHaveAccessibleName(/blocked by/)
    update(() => ({ pending: null }))
    expect(marks().size).toBe(0)
    expect(notes().size).toBe(0)
    expect(card(must(attackers[0]))).not.toHaveAccessibleName(/blocked by/)
  })
})

describe('the words', () => {
  it('says what to do before a blocker is chosen and is one live region', () => {
    const { text, container } = block(must(SPECS[0]))
    expect(text()).toBe('Click a blocker, then the attacker it blocks.')
    const line = must(container.querySelector<HTMLElement>('[data-slot="decision-line"]'))
    expect(within(line).getByText('Your decision: Declare blockers')).toBeInTheDocument()
    expect(line.querySelectorAll('[aria-live]')).toHaveLength(1)
  })

  it('tells two attackers of one name apart in the list and the pairs', async () => {
    const { card, user, update, blockers, attackers, choices, picker, button, answer } = block({
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
    await user.selectOptions(picker(), must(choices()[1]))
    await user.click(button('Confirm blocks'))
    expect(answer.mock.calls).toEqual([[{ DeclareBlockers: [[blocker, b]] }]])
  })
})
