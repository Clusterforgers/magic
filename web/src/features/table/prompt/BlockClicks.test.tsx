/** Goal: blocking is done by clicking alone: a blocker that can block one attacker blocks it with one click, any other is chosen and then the attacker is clicked, a blocker that blocks is clicked again to take its block back, two blockers on one attacker make a double block, no label is written on a card, and the answers sent have the exact shape the server reads. */
import { act, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { BlockerOption, ObjectId } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { creaturesOf, fixtureOf, renderCombat } from '@/test-utils/combatTable'

const mine = creaturesOf(fixtureOf(2), 0)
const theirs = creaturesOf(fixtureOf(2), 1)
const [b1, b2, b3] = mine as [ObjectId, ObjectId, ObjectId]
const [a1, a2, a3] = theirs as [ObjectId, ObjectId, ObjectId]

const table = (blockers: BlockerOption[], extra: { min_blockers?: [ObjectId, number][] } = {}) =>
  renderCombat({ DeclareBlockers: { blockers, ...extra } })

/** Both blockers may block both attackers. */
const BOTH: BlockerOption[] = [
  { object: b1, can_block: [a1, a2] },
  { object: b2, can_block: [a1, a2] },
]

const confirmed = async (t: ReturnType<typeof table>) => {
  await t.user.click(t.button('Confirm blocks'))
  return t.answer.mock.calls.map(([sent]) => sent)
}

describe('the clicks of a block', () => {
  it.each([
    {
      name: 'a blocker that can block one attacker blocks it with one click',
      blockers: [{ object: b1, can_block: [a1] }],
      clicks: [b1],
      sent: [[b1, a1]],
    },
    {
      name: 'a blocker is chosen and then the attacker is clicked',
      blockers: BOTH,
      clicks: [b1, a2],
      sent: [[b1, a2]],
    },
    {
      name: 'a double block is a second blocker chosen and the same attacker clicked',
      blockers: BOTH,
      clicks: [b1, a1, b2, a1],
      sent: [
        [b1, a1],
        [b2, a1],
      ],
    },
    {
      name: 'two blockers on two attackers',
      blockers: BOTH,
      clicks: [b1, a1, b2, a2],
      sent: [
        [b1, a1],
        [b2, a2],
      ],
    },
    {
      name: 'the chosen blocker is changed by choosing another one',
      blockers: BOTH,
      clicks: [b1, b2, a2],
      sent: [[b2, a2]],
    },
    {
      name: 'a click on an attacker with no blocker chosen does nothing, and so does a click on one it may not block',
      blockers: [
        { object: b1, can_block: [a1, a2] },
        { object: b2, can_block: [a1] },
      ],
      clicks: [a1, b1, a3, a2],
      sent: [[b1, a2]],
    },
  ])('$name', async ({ blockers, clicks, sent }) => {
    const t = table(blockers)
    for (const click of clicks) await t.user.click(t.card(click))
    expect(await confirmed(t)).toEqual([{ DeclareBlockers: sent }])
    expect(t.notes().size).toBe(0)
  })

  it('takes a block back with a second click on the blocker, and Confirm waits again', async () => {
    const t = table(BOTH)
    await t.user.click(t.card(b1))
    await t.user.click(t.card(a1))
    expect(t.marks().get(b1)).toBe('selected')
    expect(t.button('Confirm blocks')).toBeEnabled()
    await t.user.click(t.card(b1))
    expect(t.marks().get(b1)).toBe('legal')
    expect(t.button('Confirm blocks')).toBeDisabled()
  })

  it('takes one blocker of a double block back and keeps the other', async () => {
    const t = table(BOTH)
    for (const click of [b1, a1, b2, a1, b2]) await t.user.click(t.card(click))
    expect(await confirmed(t)).toEqual([{ DeclareBlockers: [[b1, a1]] }])
  })

  it('takes a block of a single-attacker blocker back and makes it again', async () => {
    const t = table([{ object: b1, can_block: [a1] }])
    await t.user.click(t.card(b1))
    await t.user.click(t.card(b1))
    expect(t.button('Confirm blocks')).toBeDisabled()
    await t.user.click(t.card(b1))
    expect(await confirmed(t)).toEqual([{ DeclareBlockers: [[b1, a1]] }])
  })

  it('shows the blocker that waits for its attacker, and the attackers it may block, and says what to click', async () => {
    const t = table([
      { object: b1, can_block: [a1, a2] },
      { object: b2, can_block: [a1, a2, a3] },
    ])
    await t.user.click(t.card(b1))
    expect(t.card(b1)).toHaveAttribute('data-cue', 'active')
    expect(t.card(b2)).not.toHaveAttribute('data-cue')
    expect(t.marks().get(a1)).toBe('legal')
    expect(t.marks().get(a3)).toBe('dimmed')
    expect(t.text()).toMatch(/^Click the attacker .* blocks\.$/)
    await t.user.click(t.card(b1))
    expect(t.card(b1)).not.toHaveAttribute('data-cue')
  })

  it('lets go of the chosen blocker with Escape', async () => {
    const t = table(BOTH)
    await t.user.click(t.card(b1))
    await t.user.keyboard('{Escape}')
    expect(t.marks().get(b1)).toBe('legal')
    expect(screen.queryByRole('combobox')).toBeNull()
  })

  it('writes no label on any card, and says the block in the spoken name', async () => {
    const t = table(BOTH)
    await t.user.click(t.card(b1))
    await t.user.click(t.card(a1))
    expect(t.container.querySelector('[data-slot="card-note"]')).toBeNull()
    expect(t.card(b1)).toHaveAccessibleName(/blocking/)
    expect(t.card(a1)).toHaveAccessibleName(/blocked by/)
  })

  it('has one row of two buttons, plus the list of attackers while a blocker is chosen, and no panel', async () => {
    const t = table(BOTH)
    const bar = must(t.container.querySelector('[data-slot="combat-bar"]'))
    expect(screen.getAllByRole('button', { name: /^(Confirm blocks|No blocks)$/ })).toHaveLength(2)
    expect(screen.queryByRole('list', { name: 'Your blocks' })).toBeNull()
    expect(screen.queryByRole('combobox')).toBeNull()
    await t.user.click(t.card(b1))
    expect(screen.getByRole('combobox', { name: /^Attacker .* blocks$/ })).toBeInTheDocument()
    expect(bar.querySelectorAll('button')).toHaveLength(2)
  })

  it('works from the keyboard: Enter on a blocker, Enter on its attacker, Enter on Confirm blocks', async () => {
    const t = table(BOTH)
    act(() => t.card(b1).focus())
    await t.user.keyboard('{Enter}')
    expect(t.marks().get(b1)).toBe('selected')
    act(() => t.card(a2).focus())
    await t.user.keyboard('{Enter}')
    act(() => t.button('Confirm blocks').focus())
    await t.user.keyboard('{Enter}')
    expect(t.answer.mock.calls).toEqual([[{ DeclareBlockers: [[b1, a2]] }]])
  })

  it('confirms with Enter when nothing is focused', async () => {
    const t = table([{ object: b1, can_block: [a1] }])
    await t.user.click(t.card(b1))
    act(() => (document.activeElement as HTMLElement).blur())
    await t.user.keyboard('{Enter}')
    expect(t.answer.mock.calls).toEqual([[{ DeclareBlockers: [[b1, a1]] }]])
  })

  it('answers No blocks even with a block made', async () => {
    const t = table([{ object: b3, can_block: [a1] }])
    await t.user.click(t.card(b3))
    await t.user.click(t.button('No blocks'))
    expect(t.answer.mock.calls).toEqual([[{ DeclareBlockers: [] }]])
  })

  it('keeps Confirm off while an attacker that needs two blockers has one, and labels it on its card', async () => {
    const t = table(BOTH, { min_blockers: [[a1, 2]] })
    expect(t.container.querySelector(`[data-object-id="${a1}"] [data-slot="card-note"]`)).toHaveTextContent(
      'Needs 2+ blockers',
    )
    await t.user.click(t.card(b1))
    await t.user.click(t.card(a1))
    expect(t.button('Confirm blocks')).toBeDisabled()
    expect(t.container.querySelector(`[data-object-id="${a1}"] [data-slot="card-note"]`)).toHaveTextContent(
      'Needs 1 more',
    )
    await t.user.click(t.card(b2))
    await t.user.click(t.card(a1))
    expect(await confirmed(t)).toEqual([
      {
        DeclareBlockers: [
          [b1, a1],
          [b2, a1],
        ],
      },
    ])
  })
})
