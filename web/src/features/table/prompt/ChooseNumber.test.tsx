/** Goal: for ranges with and without a limit, left-out numbers and a cancel, the ChooseNumber prompt names its field by the card's words, starts in the field on the smallest allowed number, steps by buttons, arrows and slider over left-out numbers, takes a typed number, keeps Confirm off with the reason for a number not allowed, answers once with the number (Enter or Confirm) or Cancel, and shows the total cost for an X. */
import { act, fireEvent, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { NumberOptions } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { creaturesOf, fixtureOf, renderDecision } from '@/test-utils/combatTable'
import { cost } from '@/test-utils/mana'

const source = must(creaturesOf(fixtureOf(2), 0)[0])
const sourceName = must(fixtureOf(2).view.objects.find((object) => object.id === source)).name
const X = 'Choose a value for X.'

function mount(given: Partial<NumberOptions> = {}) {
  const options: NumberOptions = { source, min: 0, max: 5, text: X, ...given }
  const table = renderDecision({ ChooseNumber: options })
  const field = (): HTMLInputElement => screen.getByRole('spinbutton', { name: options.text || 'Number' }) as HTMLInputElement
  const confirm = (): HTMLElement => must(table.container.querySelector<HTMLElement>('[data-slot="number-confirm"]'))
  return { ...table, options, field, confirm }
}

describe('the field', () => {
  it.each<[string, Partial<NumberOptions>, string]>([
    ['from 0', {}, '0'],
    ['from 2', { min: 2 }, '2'],
    ['past a left-out smallest number', { except: [0] }, '1'],
  ])('starts %s, focused, on the smallest allowed number', (_what, given, start) => {
    const { field } = mount(given)
    expect(field().value).toBe(start)
    expect(field()).toHaveFocus()
  })

  it.each([
    [X, X],
    ['Choose how many times to pay Multikicker {G}.', 'Choose how many times to pay Multikicker {G}.'],
    ['', 'Number'],
  ])('is named by the words "%s"', (text, name) => {
    mount({ text })
    expect(screen.getByRole('spinbutton', { name })).toBeInTheDocument()
  })

  it('asks in the card words and reads the range to a screen reader', () => {
    expect(mount({ except: [2] }).text()).toBe(`${sourceName}: ${X} From 0 to 5, except 2.`)
  })
})

describe('stepping and typing', () => {
  it.each<[string, Partial<NumberOptions>, string[], string]>([
    ['up twice', {}, ['More', 'More'], '2'],
    ['up over a left-out number', { except: [1] }, ['More'], '2'],
    ['down again', {}, ['More', 'More', 'Less'], '1'],
    ['not past the top', { max: 1 }, ['More', 'More'], '1'],
  ])('%s', async (_what, given, clicks, value) => {
    const { field, user } = mount(given)
    for (const name of clicks) {
      const button = screen.getByRole('button', { name })
      if (button.hasAttribute('disabled')) continue
      await user.click(button)
    }
    expect(field().value).toBe(value)
  })

  it('steps with the arrow keys over a left-out number', async () => {
    const { field, user } = mount({ except: [1, 2] })
    await user.keyboard('{ArrowUp}')
    expect(field().value).toBe('3')
    await user.keyboard('{ArrowDown}')
    expect(field().value).toBe('0')
  })

  it('moves with the slider, which is left out without a limit', () => {
    mount({ except: [3] })
    const slider = screen.getByRole('slider', { name: `${X} (slider)` })
    act(() => {
      fireEvent.change(slider, { target: { value: '3' } })
    })
    expect((screen.getByRole('spinbutton') as HTMLInputElement).value).toBe('4')
  })

  it('has no slider without a limit', () => {
    mount({ max: null })
    expect(screen.queryByRole('slider')).toBeNull()
  })
})

describe('answers', () => {
  it.each<[string, Partial<NumberOptions>, string, string | undefined]>([
    ['a number in range', {}, '3', undefined],
    ['above the top', {}, '6', 'Choose 5 or less.'],
    ['a left-out number', { except: [2] }, '2', '2 is not allowed here.'],
    ['nothing typed', {}, '', 'Enter a whole number.'],
  ])('typed: %s', async (_what, given, typed, problem) => {
    const { field, confirm, user, answer } = mount(given)
    await user.clear(field())
    if (typed !== '') await user.type(field(), typed)
    if (problem === undefined) {
      expect(confirm()).toBeEnabled()
      await user.keyboard('{Enter}')
      expect(answer.mock.calls).toEqual([[{ ChooseNumber: { Number: Number(typed) } }]])
    } else {
      expect(confirm()).toBeDisabled()
      expect(field()).toHaveAccessibleDescription(problem)
      await user.keyboard('{Enter}')
      expect(answer).not.toHaveBeenCalled()
    }
  })

  it('confirms by the button, once', async () => {
    const { confirm, user, answer } = mount()
    await user.click(screen.getByRole('button', { name: 'More' }))
    await user.click(confirm())
    expect(answer.mock.calls).toEqual([[{ ChooseNumber: { Number: 1 } }]])
  })

  it.each([
    [true, 1],
    [false, 0],
  ])('offers Cancel only when the options allow it (%s)', async (canCancel, buttons) => {
    const { user, answer } = mount({ can_cancel: canCancel })
    const cancel = screen.queryAllByRole('button', { name: 'Cancel' })
    expect(cancel).toHaveLength(buttons)
    if (canCancel) {
      await user.click(must(cancel[0]))
      expect(answer.mock.calls).toEqual([[{ ChooseNumber: 'Cancel' }]])
    }
  })
})

describe('the total cost of an X', () => {
  it.each<[string, number, string[], string]>([
    ['{R}', 1, [], 'Total cost: red'],
    ['{R}', 1, ['More', 'More', 'More'], 'Total cost: 3 generic, red'],
    ['{1}{G}', 2, ['More'], 'Total cost: 3 generic, green'],
  ])('%s with %i per X after %j reads "%s"', async (at, perX, clicks, label) => {
    const { user, container } = mount({ cost: cost(at), per_x: perX })
    for (const name of clicks) await user.click(screen.getByRole('button', { name }))
    const total = must(container.querySelector('[data-slot="number-total"] [role="img"]'))
    expect(total.getAttribute('aria-label')).toBe(label)
  })

  it('shows no total for a number that is not an X of a mana cost', () => {
    const { container } = mount()
    expect(container.querySelector('[data-slot="number-total"]')).toBeNull()
  })
})
