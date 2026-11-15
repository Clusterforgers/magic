/** Goal: a block declaration another player makes for a defending player (`BlockOptions.r6400_for`, "you choose which creatures block this combat and how those creatures block") says whose creatures it declares for, offers that player's creatures and answers with their blocks; a player's own declaration says nothing of the kind. */
import { describe, expect, it } from 'vitest'
import type { BlockOptions, ObjectId } from '@/protocol/generated'
import { creaturesOf, fixtureOf, renderCombat } from '@/test-utils/combatTable'

function options(chosenFor: number | undefined): BlockOptions {
  const [attacker] = creaturesOf(fixtureOf(3), 0) as [ObjectId]
  const [blocker] = creaturesOf(fixtureOf(3), 1) as [ObjectId]
  return {
    blockers: [{ object: blocker, can_block: [attacker] }],
    ...(chosenFor === undefined ? {} : { r6400_for: chosenFor }),
  }
}

describe('blocks chosen for another player', () => {
  it.each([1, 2])('names the seat whose creatures block (seat %i)', (seat) => {
    const t = renderCombat({ DeclareBlockers: options(seat) }, { seats: 3 })
    expect(t.text()).toMatch(/^You choose how .+'s creatures block\. /)
  })

  it('says nothing of it for a player declaring their own blocks', () => {
    const t = renderCombat({ DeclareBlockers: options(undefined) }, { seats: 3 })
    expect(t.text()).not.toMatch(/You choose how/)
  })

  it("answers with the blocks of the other player's creature", async () => {
    const t = renderCombat({ DeclareBlockers: options(1) }, { seats: 3 })
    const [attacker] = creaturesOf(fixtureOf(3), 0) as [ObjectId]
    const [blocker] = creaturesOf(fixtureOf(3), 1) as [ObjectId]
    await t.user.click(t.card(blocker))
    if (t.button('Confirm blocks').hasAttribute('disabled')) await t.user.click(t.card(attacker))
    await t.user.click(t.button('Confirm blocks'))
    expect(t.answer.mock.calls).toEqual([[{ DeclareBlockers: [[blocker, attacker]] }]])
  })
})
