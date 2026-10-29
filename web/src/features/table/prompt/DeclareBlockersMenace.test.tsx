/** Goal: an attacker that needs several blockers (menace, `BlockOptions.min_blockers`) says so before anyone is put on it, on its card and in the prompt; Confirm waits while it has some blockers but too few and sends once it has enough or none; a prompt without such an attacker is as it was. */
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { BlockOptions, ObjectId } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { creaturesOf, fixtureOf, renderCombat } from '@/test-utils/combatTable'
import { blockersNeeded } from './combatText'

/** Three of the viewer's creatures may block either of two attackers; the first attacker needs `needed` blockers. */
function menaceTable(needed: number | null) {
  const attackers = creaturesOf(fixtureOf(2), 1).slice(0, 2) as [ObjectId, ObjectId]
  const blockers = creaturesOf(fixtureOf(2), 0).slice(0, 3) as [ObjectId, ObjectId, ObjectId]
  const options: BlockOptions = {
    blockers: blockers.map((object) => ({ object, can_block: [...attackers] })),
    ...(needed === null ? {} : { min_blockers: [[attackers[0], needed]] as [ObjectId, number][] }),
  }
  const table = renderCombat({ DeclareBlockers: options }, { seats: 2 })
  const confirm = (): HTMLElement => table.button('Confirm blocks')
  /** Chooses `blocker`, then the attacker at `at` in its list. */
  const block = async (blocker: ObjectId, at: 0 | 1): Promise<void> => {
    await table.user.click(table.card(blocker))
    await table.user.click(table.card(must(attackers[at])))
  }
  const list = () => screen.queryByRole('list', { name: 'Attackers that need several blockers' })
  return { ...table, attackers, blockers, confirm, block, list }
}

describe('an attacker that needs several blockers', () => {
  it('says so in the prompt and on its card before anyone is put on it', () => {
    const { list, notes, attackers, text, confirm } = menaceTable(2)
    expect(must(list())).toHaveTextContent(/needs two or more blockers/)
    expect(notes().get(attackers[0])).toBe('Needs 2+ blockers')
    expect(notes().has(attackers[1])).toBe(false)
    expect(text()).not.toMatch(/needs/)
    expect(confirm()).toBeDisabled()
  })

  it('keeps Confirm off while it has one blocker and sends once it has two', async () => {
    const { block, confirm, list, notes, text, attackers, blockers, user, answer } = menaceTable(2)
    await block(blockers[0], 0)
    expect(confirm()).toBeDisabled()
    expect(notes().get(attackers[0])).toBe('Needs 1 more')
    expect(text()).toMatch(/needs two or more blockers\. Choose another creature to block it/)
    expect(must(list())).toHaveTextContent(/1 of 2 chosen/)
    await block(blockers[1], 0)
    expect(confirm()).toBeEnabled()
    expect(text()).not.toMatch(/needs/)
    expect(notes().has(attackers[0])).toBe(false)
    await user.click(confirm())
    expect(answer.mock.calls).toEqual([
      [
        {
          DeclareBlockers: [
            [blockers[0], attackers[0]],
            [blockers[1], attackers[0]],
          ],
        },
      ],
    ])
  })

  it('lets another attacker be blocked by one while it has none, and waits again once one blocker joins it', async () => {
    const { block, card, confirm, attackers, blockers, user, answer } = menaceTable(2)
    await block(blockers[0], 1)
    expect(confirm()).toBeEnabled()
    // The second blocker goes to the one that needs two: it has one, so Confirm waits.
    await block(blockers[1], 0)
    expect(confirm()).toBeDisabled()
    // Taking that block back leaves the first block alone, which can be sent.
    await user.click(card(must(blockers[1])))
    expect(confirm()).toBeEnabled()
    await user.click(confirm())
    expect(answer.mock.calls).toEqual([[{ DeclareBlockers: [[blockers[0], attackers[1]]] }]])
  })

  it('names the shortfall of a larger number', async () => {
    const { block, notes, text, attackers, blockers, list } = menaceTable(3)
    await block(blockers[0], 0)
    expect(notes().get(attackers[0])).toBe('Needs 2 more')
    expect(text()).toMatch(/needs three or more blockers/)
    expect(must(list())).toHaveTextContent(/1 of 3 chosen/)
  })
})

describe('a prompt without an attacker that needs several blockers', () => {
  it('draws no list and sends one blocker on one attacker', async () => {
    const { block, confirm, list, notes, attackers, blockers, user, answer } = menaceTable(null)
    expect(list()).toBeNull()
    expect(notes().size).toBe(0)
    await block(blockers[0], 0)
    expect(confirm()).toBeEnabled()
    await user.click(confirm())
    expect(answer.mock.calls).toEqual([[{ DeclareBlockers: [[blockers[0], attackers[0]]] }]])
  })

  it('ignores a minimum of one', () => {
    const { list } = menaceTable(1)
    expect(list()).toBeNull()
  })
})

describe('the words for the number of blockers', () => {
  it.each([
    [2, 'two or more blockers'],
    [3, 'three or more blockers'],
    [10, 'ten or more blockers'],
    [11, '11 or more blockers'],
  ])('%i reads "%s"', (count, words) => {
    expect(blockersNeeded(count)).toBe(words)
  })
})
