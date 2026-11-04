/** Goal: an attacker a rule says must be blocked (`BlockOptions.must_be_blocked`) is marked "Must be blocked" while a free creature could block it, Confirm and No blocks wait until something blocks it, and the marks and the wait are gone once it is blocked. */
import { describe, expect, it } from 'vitest'
import type { BlockOptions, ObjectId } from '@/protocol/generated'
import { creaturesOf, fixtureOf, renderCombat } from '@/test-utils/combatTable'

function table() {
  const attackers = creaturesOf(fixtureOf(2), 1).slice(0, 2) as [ObjectId, ObjectId]
  const [a, b] = creaturesOf(fixtureOf(2), 0) as [ObjectId, ObjectId]
  const options: BlockOptions = {
    blockers: [
      { object: a, can_block: [...attackers] },
      { object: b, can_block: [...attackers] },
    ],
    must_be_blocked: [attackers[0]],
  }
  return { ...renderCombat({ DeclareBlockers: options }, { seats: 2 }), a, b, attackers }
}

describe('an attacker a rule says must be blocked', () => {
  it('is marked and keeps No blocks off until a creature blocks it', async () => {
    const t = table()
    expect(t.notes().get(t.attackers[0])).toBe('Must be blocked')
    expect(t.button('No blocks')).toBeDisabled()
    expect(t.button('Confirm blocks')).toBeDisabled()
    expect(t.text()).toMatch(/must be blocked if able\.$/)
  })

  it('lets Confirm go once a creature blocks it, and not before when another one is blocked', async () => {
    const t = table()
    await t.user.click(t.card(t.a))
    await t.user.click(t.card(t.attackers[1]))
    expect(t.button('Confirm blocks')).toBeDisabled()
    await t.user.click(t.card(t.b))
    await t.user.click(t.card(t.attackers[0]))
    expect(t.button('Confirm blocks')).toBeEnabled()
    await t.user.click(t.button('Confirm blocks'))
    expect(t.answer.mock.calls).toEqual([[{ DeclareBlockers: [[t.a, t.attackers[1]], [t.b, t.attackers[0]]] }]])
  })
})
