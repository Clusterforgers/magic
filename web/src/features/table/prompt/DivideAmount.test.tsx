/** Goal: for totals, target counts and kinds of target, the DivideAmount prompt starts at the even split with focus on the first number, steps and types within what the other targets leave, counts what is left to give, keeps Confirm off until the split adds up to the total, answers once with every target and its share in the engine's order or Cancel when the options allow it, draws a picture for each card among the targets and says why nothing can be sent when the total is too small. */
import { fireEvent, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { DivideOptions, Target } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { creaturesOf, fixtureOf, renderDecision } from '@/test-utils/combatTable'

const fixture = fixtureOf(2)
const source = must(creaturesOf(fixture, 0)[0])
const foeA = must(creaturesOf(fixture, 1)[0])
const foeB = must(creaturesOf(fixture, 1)[1])
const object = (id: number): Target => ({ Object: { id, counter: 0 } })
const ME: Target = { Player: 0 }
const FOE: Target = { Player: 1 }
const TEXT = 'Divide 3 damage among the targets.'

function mount(given: Partial<DivideOptions> = {}) {
  const options: DivideOptions = { source, total: 3, among: [FOE, object(foeA)], text: TEXT, ...given }
  const table = renderDecision({ DivideAmount: options })
  const root = table.container
  const fields = (): HTMLInputElement[] => screen.getAllByRole('spinbutton') as HTMLInputElement[]
  const shares = (): number[] => fields().map((field) => Number(field.value))
  const confirm = (): HTMLElement => screen.getByRole('button', { name: 'Confirm' })
  const status = (): string => must(root.querySelector('[data-slot="divide-status"]')).textContent ?? ''
  const rows = (): HTMLElement[] => [...root.querySelectorAll<HTMLElement>('[data-slot="divide-row"]')]
  // One press of a stepper button of a row; a button the rules hold off (aria-disabled) does nothing.
  const click = (direction: 'Less' | 'More', row: number): void => {
    const button = within(must(rows()[row])).getByRole('button', { name: new RegExp(`^${direction} for `) })
    if (button.getAttribute('aria-disabled') !== 'true') fireEvent.click(button)
  }
  return { ...table, options, fields, shares, confirm, status, rows, click }
}

describe('the first split', () => {
  it.each<[string, number, Target[], number[]]>([
    ['3 among 2', 3, [FOE, ME], [2, 1]],
    ['4 among 2', 4, [FOE, ME], [2, 2]],
    ['7 among 3', 7, [FOE, ME, object(foeA)], [3, 2, 2]],
    ['2 among 2', 2, [FOE, ME], [1, 1]],
    ['5 among 1', 5, [FOE], [5]],
  ])('%s starts at %j', (_what, total, among, expected) => {
    const { shares, confirm, status } = mount({ total, among })
    expect(shares()).toEqual(expected)
    expect(confirm()).toBeEnabled()
    expect(status()).toBe(`All ${total} given out.`)
  })

  it('starts with focus on the first number', () => {
    expect(mount().fields()[0]).toHaveFocus()
  })

  it('asks in the card words, with the least each gets', () => {
    const name = must(fixture.view.objects.find((o) => o.id === source)).name
    expect(mount().text()).toBe(`${name}: ${TEXT} Each target gets at least 1.`)
  })
})

describe('stepping and typing', () => {
  it.each<[string, number, Array<['Less' | 'More', number]>, number[]]>([
    [
      'less on the first, more on the second',
      3,
      [
        ['Less', 0],
        ['More', 1],
      ],
      [1, 2],
    ],
    ['more with nothing left stays', 3, [['More', 0]], [2, 1]],
    ['less below one stays', 2, [['Less', 0]], [1, 1]],
    [
      'less three times',
      5,
      [
        ['Less', 0],
        ['Less', 0],
        ['Less', 0],
      ],
      [1, 2],
    ],
    [
      'back and forth',
      5,
      [
        ['Less', 0],
        ['More', 1],
        ['More', 1],
      ],
      [2, 3],
    ],
  ])('buttons: %s', (_what, total, clicks, expected) => {
    const { shares, click } = mount({ total })
    for (const [direction, row] of clicks) click(direction, row)
    expect(shares()).toEqual(expected)
  })

  it('shows a button that cannot move as unavailable', () => {
    const { rows } = mount({ total: 3 })
    const more = within(must(rows()[0])).getByRole('button', { name: /^More for / })
    expect(more).toHaveAttribute('aria-disabled', 'true')
    const less = within(must(rows()[1])).getByRole('button', { name: /^Less for / })
    expect(less).toHaveAttribute('aria-disabled', 'true')
  })

  it.each<[string, string, number[]]>([
    ['arrow up has nothing to take', '{ArrowUp}', [2, 1]],
    ['arrow down gives one back', '{ArrowDown}', [1, 1]],
    ['arrow down twice stops at one', '{ArrowDown}{ArrowDown}', [1, 1]],
  ])('keyboard: %s', async (_what, keys, expected) => {
    const { user, shares } = mount()
    await user.keyboard(keys)
    expect(shares()).toEqual(expected)
  })

  it('goes up again with the arrow keys once some is left', async () => {
    const { user, shares, status } = mount({ total: 4 })
    await user.keyboard('{ArrowDown}{ArrowDown}')
    expect(shares()).toEqual([1, 2])
    expect(status()).toBe('1 left to give.')
    await user.keyboard('{ArrowUp}')
    expect(shares()).toEqual([2, 2])
  })

  it.each<[string, string, number[]]>([
    ['a number that fits', '2', [2, 1]],
    ['the most that is left', '3', [3, 1]],
    ['more than is left', '9', [3, 1]],
    ['zero', '0', [1, 1]],
    ['nothing', '', [1, 1]],
  ])('typed in the first number, with 1 left of 4: %s', (_what, typed, expected) => {
    const { fields, shares, click } = mount({ total: 4 })
    click('Less', 1)
    fireEvent.change(must(fields()[0]), { target: { value: typed } })
    expect(shares()).toEqual(expected)
  })
})

describe('the remainder and Confirm', () => {
  it.each<[number, boolean, string]>([
    [0, true, 'All 5 given out.'],
    [1, false, '1 left to give.'],
    [2, false, '2 left to give.'],
  ])('after %i steps down of 5 among 2: legal %s, "%s"', (steps, legal, status) => {
    const { confirm, status: say, click } = mount({ total: 5 })
    for (let step = 0; step < steps; step += 1) click('Less', 0)
    expect(say()).toBe(status)
    expect(confirm().hasAttribute('disabled')).toBe(!legal)
  })

  it('does not send while something is left, by the button or by Enter', async () => {
    const { user, confirm, answer, click } = mount({ total: 3 })
    click('Less', 0)
    expect(confirm()).toBeDisabled()
    await user.keyboard('{Enter}')
    expect(answer).not.toHaveBeenCalled()
  })

  it('is described by the remainder', () => {
    expect(mount().confirm()).toHaveAccessibleDescription('All 3 given out.')
  })
})

describe('answers', () => {
  it.each<[string, Partial<DivideOptions>, number[]]>([
    ['the even split', {}, [2, 1]],
    ['one target', { among: [FOE], total: 4 }, [4]],
    ['three targets', { total: 7, among: [FOE, ME, object(foeA)] }, [3, 2, 2]],
  ])('sends %s in the order of the targets', async (_what, given, expected) => {
    const { user, confirm, answer, options } = mount(given)
    await user.click(confirm())
    expect(answer.mock.calls).toEqual([
      [{ DivideAmount: { Division: options.among.map((target, at) => [target, expected[at]]) } }],
    ])
  })

  it('sends a changed split with Enter, once', async () => {
    const { user, answer, options, click } = mount({ total: 4 })
    click('Less', 1)
    click('More', 0)
    await user.keyboard('{Enter}')
    await user.keyboard('{Enter}')
    expect(answer.mock.calls).toEqual([
      [
        {
          DivideAmount: {
            Division: [
              [options.among[0], 3],
              [options.among[1], 1],
            ],
          },
        },
      ],
    ])
  })

  it.each([
    [true, 1],
    [false, 0],
    [undefined, 0],
  ])('offers Cancel only when the options allow it (%s)', async (canCancel, buttons) => {
    const { user, answer } = mount(canCancel === undefined ? {} : { can_cancel: canCancel })
    const cancel = screen.queryAllByRole('button', { name: /^Cancel the / })
    expect(cancel).toHaveLength(buttons)
    if (canCancel === true) {
      await user.click(must(cancel[0]))
      expect(answer.mock.calls).toEqual([[{ DivideAmount: 'Cancel' }]])
    }
  })
})

describe('the targets', () => {
  it('draws a picture for each card among the targets and none for a player', () => {
    const { rows } = mount({ total: 5, among: [FOE, object(foeA), object(foeB)] })
    expect(rows().map((row) => row.querySelector('[data-slot="target-face"]') !== null)).toEqual([
      false,
      true,
      true,
    ])
  })

  it('marks the cards among the targets on the table and moves to a row on a click', async () => {
    const { user, card, fields, marks } = mount()
    expect(marks().get(foeA)).toBe('legal')
    await user.click(card(foeA))
    expect(fields()[1]).toHaveFocus()
    expect(marks().get(foeA)).toBe('selected')
  })

  it('names the viewer among the players', () => {
    mount({ among: [ME, FOE], total: 2 })
    expect(screen.getByRole('group', { name: /\(you\)$/ })).toBeInTheDocument()
  })
})

describe('a total too small for the targets', () => {
  it('cannot be sent and says why', () => {
    const { confirm, status } = mount({ total: 1, among: [FOE, ME] })
    expect(confirm()).toBeDisabled()
    expect(status()).toBe('There is not enough to give each target 1.')
  })

  it('still offers Cancel when the options allow it', () => {
    mount({ total: 1, among: [FOE, ME], can_cancel: true })
    expect(screen.getByRole('button', { name: /^Cancel the / })).toBeEnabled()
  })
})
