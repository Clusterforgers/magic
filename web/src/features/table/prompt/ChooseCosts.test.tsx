/** Goal: for lists of optional costs with and without a gift and a cancel, the ChooseCosts prompt shows each cost as the card prints it as a toggle, adds up the extra cost of the chosen ones, toggles from the number keys, refuses a second gift with the reason, starts with focus on Confirm so a press pays nothing extra, answers once with the chosen set in ascending order (none included) or Cancel when the options allow it. */
import { fireEvent, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { CostsOptions, OptionalCost } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { creaturesOf, fixtureOf, renderDecision } from '@/test-utils/combatTable'
import { SEAT_BASE } from './costsText'

const fixture = fixtureOf(2)
const source = must(creaturesOf(fixture, 0)[0])
const sourceName = must(fixture.view.objects.find((o) => o.id === source)).name

const KICKER: OptionalCost = { index: 0, label: 'Kicker {2}{R}' }
const BUYBACK: OptionalCost = { index: 1, label: 'Buyback {3}' }
const GIFT_A: OptionalCost = { index: SEAT_BASE + 1, label: 'Gift to player 2' }
const GIFT_B: OptionalCost = { index: SEAT_BASE + 2, label: 'Gift to player 3' }

function mount(optional: OptionalCost[], given: Partial<CostsOptions> = {}) {
  const options: CostsOptions = { source, optional, ...given }
  const table = renderDecision({ ChooseCosts: options })
  const root = table.container
  const toggle = (label: string): HTMLElement => screen.getByRole('button', { name: label })
  const confirm = (): HTMLElement => screen.getByRole('button', { name: 'Confirm costs' })
  const total = (): string => must(root.querySelector('[data-slot="costs-total"]')).textContent ?? ''
  const problem = (): string | null => root.querySelector('[data-slot="costs-problem"]')?.textContent ?? null
  const pressed = (): string[] =>
    within(must(root.querySelector<HTMLElement>('[data-slot="cost-list"]')))
      .queryAllByRole('button', { pressed: true })
      .map((button) => button.getAttribute('aria-label') ?? '')
  return { ...table, options, toggle, confirm, total, problem, pressed }
}

describe('the toggles', () => {
  it.each<[string, OptionalCost[]]>([
    ['one cost', [KICKER]],
    ['two costs', [KICKER, BUYBACK]],
    ['costs and gifts', [KICKER, GIFT_A, GIFT_B]],
  ])('lists %s, none taken', (_what, optional) => {
    const { toggle, pressed } = mount(optional)
    for (const cost of optional) expect(toggle(cost.label)).toHaveAttribute('aria-pressed', 'false')
    expect(pressed()).toEqual([])
  })

  it('asks in the card words and reads the keys to a screen reader', () => {
    expect(mount([KICKER, BUYBACK]).text()).toBe(
      `${sourceName}: choose the optional costs to pay, or none. Press 1 to 2 to take a cost or leave it.`,
    )
  })

  it('draws the mana of a cost as symbols', () => {
    const { toggle } = mount([KICKER])
    const symbols = [...toggle(KICKER.label).querySelectorAll('[data-pip]')].map((pip) =>
      pip.getAttribute('data-pip'),
    )
    expect(symbols).toEqual(['2', 'R'])
  })

  it('starts with focus on Confirm', () => {
    expect(mount([KICKER]).confirm()).toHaveFocus()
  })

  it('toggles a cost on and off by click', async () => {
    const { user, toggle } = mount([KICKER, BUYBACK])
    await user.click(toggle(KICKER.label))
    expect(toggle(KICKER.label)).toHaveAttribute('aria-pressed', 'true')
    await user.click(toggle(KICKER.label))
    expect(toggle(KICKER.label)).toHaveAttribute('aria-pressed', 'false')
  })

  it.each<[string, string, string[]]>([
    ['the first', '1', ['Kicker {2}{R}']],
    ['the second', '2', ['Buyback {3}']],
    ['both', '12', ['Kicker {2}{R}', 'Buyback {3}']],
    ['the first twice', '11', []],
    ['a key with no cost', '3', []],
  ])('keys: %s', async (_what, keys, expected) => {
    const { user, pressed } = mount([KICKER, BUYBACK])
    await user.keyboard(keys)
    expect(pressed()).toEqual(expected)
  })
})

describe('the extra cost', () => {
  // The line shows the mana as symbols and the other parts as words; a screen reader hears the whole cost after a colon.
  it.each<[string, OptionalCost[], string[], string, string, string]>([
    ['nothing taken', [KICKER, BUYBACK], [], '', '', 'No extra cost'],
    ['the kicker', [KICKER, BUYBACK], [KICKER.label], 'Extra mana: 2 generic, red', '', ': {2}{R}'],
    [
      'both added up',
      [KICKER, BUYBACK],
      [KICKER.label, BUYBACK.label],
      'Extra mana: 5 generic, red',
      '',
      ': {5}{R}',
    ],
    [
      'a gift beside mana',
      [KICKER, GIFT_A],
      [KICKER.label, GIFT_A.label],
      'Extra mana: 2 generic, red',
      'Gift to player 2',
      ': {2}{R} and Gift to player 2',
    ],
    ['a gift alone', [KICKER, GIFT_A], [GIFT_A.label], '', 'Gift to player 2', ': Gift to player 2'],
  ])('%s', async (_what, optional, taken, pips, others, spoken) => {
    const { user, toggle, total, container } = mount(optional)
    for (const label of taken) await user.click(toggle(label))
    const line = must(container.querySelector('[data-slot="costs-total"]'))
    expect(line.querySelector('[role="img"]')?.getAttribute('aria-label') ?? '').toBe(pips)
    expect(line.querySelector(':scope > span:not(.sr-only):not([role])')?.textContent ?? '').toBe(others)
    expect(total()).toContain(spoken)
  })
})

describe('a second gift', () => {
  it('is refused with the reason, and nothing changes', async () => {
    const { user, toggle, problem, pressed } = mount([KICKER, GIFT_A, GIFT_B])
    await user.click(toggle(GIFT_A.label))
    expect(problem()).toBeNull()
    await user.click(toggle(GIFT_B.label))
    expect(problem()).toBe(
      'Only one gift can be promised: take back "Gift to player 2" to choose "Gift to player 3".',
    )
    expect(pressed()).toEqual([GIFT_A.label])
    expect(toggle(GIFT_B.label)).toHaveAttribute('aria-disabled', 'true')
  })

  it('is allowed once the first is taken back, and the reason goes', async () => {
    const { user, toggle, problem, pressed } = mount([GIFT_A, GIFT_B])
    await user.click(toggle(GIFT_A.label))
    await user.click(toggle(GIFT_B.label))
    await user.click(toggle(GIFT_A.label))
    expect(problem()).toBeNull()
    await user.click(toggle(GIFT_B.label))
    expect(pressed()).toEqual([GIFT_B.label])
  })

  it('does not hold back a kicker', async () => {
    const { user, toggle, pressed } = mount([KICKER, GIFT_A])
    await user.click(toggle(GIFT_A.label))
    await user.click(toggle(KICKER.label))
    expect(pressed()).toEqual([KICKER.label, GIFT_A.label])
  })

  it('is refused from the number key too', async () => {
    const { user, problem, pressed } = mount([GIFT_A, GIFT_B])
    await user.keyboard('12')
    expect(problem()).not.toBeNull()
    expect(pressed()).toEqual([GIFT_A.label])
  })
})

describe('answers', () => {
  it.each<[string, OptionalCost[], string[], number[]]>([
    ['no cost', [KICKER, BUYBACK], [], []],
    ['the second only', [KICKER, BUYBACK], [BUYBACK.label], [1]],
    ['both, in index order', [KICKER, BUYBACK], [BUYBACK.label, KICKER.label], [0, 1]],
    ['a kicker and a gift', [KICKER, GIFT_B], [GIFT_B.label, KICKER.label], [0, SEAT_BASE + 2]],
  ])('sends %s', async (_what, optional, taken, indices) => {
    const { user, toggle, confirm, answer } = mount(optional)
    for (const label of taken) await user.click(toggle(label))
    await user.click(confirm())
    expect(answer.mock.calls).toEqual([[{ ChooseCosts: { Costs: indices } }]])
  })

  it('sends with Enter on the focused Confirm, once', async () => {
    const { user, answer } = mount([KICKER])
    await user.keyboard('{Enter}')
    await user.keyboard('{Enter}')
    expect(answer.mock.calls).toEqual([[{ ChooseCosts: { Costs: [] } }]])
  })

  it.each([
    [true, 1],
    [false, 0],
    [undefined, 0],
  ])('offers Cancel only when the options allow it (%s)', async (canCancel, buttons) => {
    const { user, answer } = mount([KICKER], canCancel === undefined ? {} : { can_cancel: canCancel })
    const cancel = screen.queryAllByRole('button', { name: /^Cancel the / })
    expect(cancel).toHaveLength(buttons)
    if (canCancel === true) {
      await user.click(must(cancel[0]))
      expect(answer.mock.calls).toEqual([[{ ChooseCosts: 'Cancel' }]])
    }
  })

  it('has no toggle for a cost that is not offered', () => {
    const { container, pressed } = mount([BUYBACK])
    expect(within(container).queryByRole('button', { name: KICKER.label })).toBeNull()
    fireEvent.keyDown(document, { key: '2' })
    expect(pressed()).toEqual([])
  })
})
