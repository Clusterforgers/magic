/** Goal: a block declaration another player makes for a defending player ("you choose which creatures block this combat and how those creatures block": the creatures offered are not the deciding player's) says whose creatures it declares for and answers with their blocks; a player's own declaration says nothing of the kind. */
import { describe, expect, it } from 'vitest'
import type { BlockOptions, ObjectId } from '@/protocol/generated'
import { creaturesOf, fixtureOf, renderCombat } from '@/test-utils/combatTable'

/** Seat `blocking`'s first creature may block seat `attacking`'s first creature. */
function table(blocking: number, attacking: number) {
  const [attacker] = creaturesOf(fixtureOf(3), attacking) as [ObjectId]
  const [blocker] = creaturesOf(fixtureOf(3), blocking) as [ObjectId]
  const options: BlockOptions = { blockers: [{ object: blocker, can_block: [attacker] }] }
  return { ...renderCombat({ DeclareBlockers: options }, { seats: 3 }), attacker, blocker }
}

describe('blocks chosen for another player', () => {
  it.each([1, 2])('names the seat whose creatures block (seat %i)', (seat) => {
    const t = table(seat, seat === 1 ? 2 : 1)
    expect(t.text()).toMatch(/^You choose how .+'s creatures block\. /)
  })

  it('says nothing of it for a player declaring their own blocks', () => {
    const t = table(0, 1)
    expect(t.text()).not.toMatch(/You choose how/)
  })

  it("answers with the blocks of the other player's creature", async () => {
    const t = table(1, 2)
    await t.user.click(t.card(t.blocker))
    if (t.button('Confirm blocks').hasAttribute('disabled')) await t.user.click(t.card(t.attacker))
    await t.user.click(t.button('Confirm blocks'))
    expect(t.answer.mock.calls).toEqual([[{ DeclareBlockers: [[t.blocker, t.attacker]] }]])
  })
})
