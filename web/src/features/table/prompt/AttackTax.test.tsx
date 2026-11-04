/** Goal: when an effect taxes attacking, each creature that can attack is selectable and the creatures that cannot are dimmed, the bar adds the chosen costs up in words against the mana the player has, and the payment that follows an attack says it is for the attack and goes back to the attackers. */
import { describe, expect, it } from 'vitest'
import type { AttackerOption, AttackTarget, ManaCost, ObjectId } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { creaturesOf, fixtureOf, renderCombat } from '@/test-utils/combatTable'
import { attackLifeOf, attackTaxOf, costLine } from './combatText'
import { payText } from './payText'

const GENERIC_2: ManaCost = [{ Generic: 2 }]
const BO: AttackTarget = { Player: 1 }
const CY: AttackTarget = { Player: 2 }

describe('costLine', () => {
  const two = { cost: GENERIC_2, life: 0 }
  const paid = 'You pay after you confirm.'
  type Row = [string, { cost: ManaCost; life: number }[], boolean, number, string]
  it.each<Row>([
    ['nothing taxed', [two], false, 4, ''],
    [
      'nothing chosen',
      [],
      true,
      3,
      'Some creatures cost mana to attack. Choose attackers to see the total. You have 3 mana available.',
    ],
    ['one creature', [two], true, 3, `The attack costs 2 generic. You have 3 mana available. ${paid}`],
    [
      'several creatures',
      [two, two],
      true,
      4,
      `The attack costs 4 generic. You have 4 mana available. ${paid}`,
    ],
    [
      'more than available',
      [two, two],
      true,
      3,
      `The attack costs 4 generic. You have 3 mana available. That is more than you surely have. ${paid}`,
    ],
    ['life', [{ cost: [], life: 2 }], true, 0, `The attack costs 2 life. You have 0 mana available. ${paid}`],
    ['free ones', [{ cost: [], life: 0 }], true, 2, 'These attackers cost nothing.'],
  ])('%s', (_name, declared, taxed, mana, line) => {
    expect(costLine(declared, taxed, mana)).toBe(line)
  })

  it('reads the cost of each defender from the option', () => {
    const option: AttackerOption = {
      object: 1 as ObjectId,
      defenders: [BO, CY],
      tax: [[BO, GENERIC_2]],
      life: [[CY, 3]],
    }
    expect(attackTaxOf(option, BO)).toEqual(GENERIC_2)
    expect(attackTaxOf(option, CY)).toEqual([])
    expect(attackLifeOf(option, CY)).toBe(3)
    expect(attackLifeOf(option, BO)).toBe(0)
  })
})

describe('payText for an attack', () => {
  const base = {
    spell: 'Elf',
    cost: GENERIC_2,
    owed: GENERIC_2,
    chosen: 0,
    canAdd: true,
    auto: true,
    cancel: true,
    note: '',
    covered: false,
  }
  it.each([
    [{}, 'Pay 2 generic for Elf.'],
    [{ attack: true }, 'Pay 2 generic for the attack.'],
    [
      { attack: true, canAdd: false, auto: false },
      'You cannot pay 2 generic for the attack with the mana you have. Add the mana by hand in the Commands panel, or go back to choose other attackers.',
    ],
    [{ attack: true, covered: true, chosen: 2 }, 'The cost is covered. Confirm to attack.'],
  ])('%j', (extra, start) => {
    expect(payText({ ...base, ...extra })).toContain(start)
  })
})

describe('the attack screen with a tax', () => {
  const seats = 3
  const fixture = fixtureOf(seats)
  const [a, b, c] = creaturesOf(fixture, 0) as [ObjectId, ObjectId, ObjectId]
  const taxed = (object: ObjectId): AttackerOption => ({ object, defenders: [BO], tax: [[BO, GENERIC_2]] })
  const render = () =>
    renderCombat({ DeclareAttackers: { attackers: [taxed(a), taxed(b)], mana: 3 } }, { seats, fixture })

  it('shows the total in words and updates it with each creature', async () => {
    const t = render()
    const line = () => must(t.container.querySelector('[data-slot="attack-cost"]')).textContent
    expect(line()).toContain('Choose attackers to see the total. You have 3 mana available.')
    await t.user.click(t.card(a))
    expect(line()).toBe('The attack costs 2 generic. You have 3 mana available. You pay after you confirm.')
    await t.user.click(t.card(b))
    expect(line()).toContain('costs 4 generic')
    expect(line()).toContain('That is more than you surely have.')
  })

  it('writes the cost on the card before and after it is chosen', async () => {
    const t = render()
    expect(t.card(a)).toHaveTextContent('Pay 2')
    expect(t.card(c)).not.toHaveTextContent('Pay 2')
    await t.user.click(t.card(a))
    expect(t.card(a)).toHaveTextContent('Pay 2')
  })

  it('answers with the attackers once confirmed', async () => {
    const t = render()
    await t.user.click(t.card(a))
    await t.user.click(t.button('Confirm attack'))
    expect(t.answer.mock.calls).toEqual([[{ DeclareAttackers: [[a, BO]] }]])
  })

  it('selects the creatures that may attack and leaves the one that may not', async () => {
    const t = render()
    await t.user.click(t.card(c))
    expect(t.button('Confirm attack').hasAttribute('disabled')).toBe(true)
    await t.user.click(t.card(a))
    expect(t.button('Confirm attack').hasAttribute('disabled')).toBe(false)
  })

  it('leaves the cost line out while nothing is taxed', () => {
    const t = renderCombat(
      { DeclareAttackers: { attackers: [{ object: a, defenders: [BO] }] } },
      { seats, fixture },
    )
    expect(t.container.querySelector('[data-slot="attack-cost"]')).toBeNull()
  })
})
