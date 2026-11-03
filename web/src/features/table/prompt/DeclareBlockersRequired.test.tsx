/** Goal: a creature a rule requires to block (`BlockOptions.required`) reads "Must block", starts on the attacker it can block alone, cannot take its block back, keeps Confirm and No blocks off until it blocks, and moves to another attacker only by choosing it. */
import { describe, expect, it } from 'vitest'
import type { BlockOptions, ObjectId } from '@/protocol/generated'
import { creaturesOf, fixtureOf, renderCombat } from '@/test-utils/combatTable'

/** Two of the viewer's creatures may block; the first must. It may block `choices` attackers. */
function table(choices: 1 | 2) {
  const attackers = creaturesOf(fixtureOf(2), 1).slice(0, 2) as [ObjectId, ObjectId]
  const [a, b] = creaturesOf(fixtureOf(2), 0) as [ObjectId, ObjectId]
  const can = choices === 1 ? [attackers[0]] : [...attackers]
  const options: BlockOptions = {
    blockers: [
      { object: a, can_block: can },
      { object: b, can_block: [...attackers] },
    ],
    required: [a],
  }
  return { ...renderCombat({ DeclareBlockers: options }, { seats: 2 }), a, b, attackers }
}

describe('a creature a rule requires to block', () => {
  it('starts on the only attacker it can block and keeps its block when clicked', async () => {
    const t = table(1)
    expect(t.notes().get(t.a)).toBe('Must block')
    expect(t.button('Confirm blocks')).toBeEnabled()
    expect(t.button('No blocks')).toBeDisabled()
    await t.user.click(t.card(t.a))
    expect(t.marks().get(t.a)).toBe('selected')
    await t.user.click(t.button('Confirm blocks'))
    expect(t.answer.mock.calls).toEqual([[{ DeclareBlockers: [[t.a, t.attackers[0]]] }]])
  })

  it('waits for a block when it can block several attackers, then keeps it', async () => {
    const t = table(2)
    expect(t.notes().get(t.a)).toBe('Must block')
    expect(t.text()).toMatch(/must block if able\.$/)
    await t.user.click(t.card(t.b))
    await t.user.click(t.card(t.attackers[0]))
    expect(t.button('Confirm blocks')).toBeDisabled()
    await t.user.click(t.card(t.a))
    await t.user.click(t.card(t.attackers[1]))
    expect(t.button('Confirm blocks')).toBeEnabled()
    await t.user.click(t.card(t.a))
    expect(t.marks().get(t.a)).toBe('selected')
    expect(t.button('Confirm blocks')).toBeEnabled()
  })
})
