/** Goal: for a choice of one mode, of several, of up to some and of a repeating mode (a spell that can be taken back and a trigger that cannot), the prompt asks in the card's words, answers the indices of the modes (not their places) in the order on the card, closes the rest when the most are chosen, sends Cancel only when allowed, starts with focus on what the clock would do, takes number keys and leaves them to a text field. */
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ModeOption, ModeOptions } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { creaturesOf, fixtureOf, renderDecision } from '@/test-utils/combatTable'

// The indices differ from the places on purpose.
const MODES: ModeOption[] = [
  { index: 1, label: 'Deal 2 damage' },
  { index: 3, label: 'Counter a spell' },
  { index: 4, label: null },
  { index: 6, label: 'Draw a card' },
]

const fixture = fixtureOf(2)
const spell = must(fixture.view.objects.find((o) => o.zone === 'Stack' && o.controller === 0))
const creature = must(creaturesOf(fixture, 0)[0])

function mount(over: Partial<ModeOptions> = {}, source = spell.id) {
  const options: ModeOptions = {
    source,
    modes: MODES,
    min: 1,
    max: 1,
    repeat: false,
    can_cancel: true,
    ...over,
  }
  const table = renderDecision({ ChooseMode: options })
  const list = (): HTMLElement => screen.getByRole('group', { name: 'Modes' })
  const mode = (place: number): HTMLElement => must(within(list()).getAllByRole('button')[place])
  const confirm = (): HTMLElement | null => screen.queryByRole('button', { name: 'Confirm modes' })
  const sent = (): unknown => table.answer.mock.calls[0]?.[0]
  return { ...table, options, list, mode, confirm, sent }
}

describe('one mode', () => {
  it('asks in words, names a mode without a label by its place and gives each its key', () => {
    const { text, mode } = mount()
    expect(text()).toBe(`${spell.name}: choose one mode. Press 1 to 4 to choose.`)
    expect(within(mode(2)).getByText('Mode 5')).toBeInTheDocument()
    expect([0, 1, 2, 3].map((n) => mode(n).getAttribute('aria-keyshortcuts'))).toEqual(['1', '2', '3', '4'])
  })

  it.each([
    [0, 1],
    [1, 3],
    [2, 4],
    [3, 6],
  ])('a click on the mode at place %i answers its index %i at once', async (place, index) => {
    const { mode, user, answer, sent } = mount()
    await user.click(mode(place))
    expect(answer).toHaveBeenCalledTimes(1)
    expect(sent()).toEqual({ ChooseMode: { Modes: [index] } })
  })

  it('answers from a number key, once, and not from a key with no mode', async () => {
    const { user, answer, sent } = mount()
    await user.keyboard('9')
    expect(answer).not.toHaveBeenCalled()
    await user.keyboard('42')
    expect(answer).toHaveBeenCalledTimes(1)
    expect(sent()).toEqual({ ChooseMode: { Modes: [6] } })
  })

  it('has no Confirm and no Undo', () => {
    const { confirm } = mount()
    expect(confirm()).toBeNull()
    expect(screen.queryByRole('button', { name: 'Undo' })).toBeNull()
  })

  it('leaves the digits to a text field', async () => {
    const { user, answer } = mount()
    const field = document.createElement('input')
    document.body.append(field)
    field.focus()
    await user.keyboard('1')
    field.remove()
    expect(answer).not.toHaveBeenCalled()
  })

  it('marks no card on the table', () => {
    expect(mount().marks().size).toBe(0)
  })
})

describe('backing out', () => {
  it('Cancel the spell sends Cancel and takes focus first, as the clock would', async () => {
    const { user, answer, sent } = mount()
    const cancel = screen.getByRole('button', { name: 'Cancel the spell' })
    expect(cancel).toHaveFocus()
    await user.keyboard('{Enter}')
    expect(answer).toHaveBeenCalledTimes(1)
    expect(sent()).toEqual({ ChooseMode: 'Cancel' })
  })

  it('names the ability when a permanent asks', () => {
    mount({}, creature)
    expect(screen.getByRole('button', { name: 'Cancel the ability' })).toBeInTheDocument()
  })

  it('has no way out for a trigger, and focus starts on the first mode', async () => {
    const { mode, user, sent } = mount({ can_cancel: false }, creature)
    expect(screen.queryByRole('button', { name: /^Cancel/ })).toBeNull()
    expect(mode(0)).toHaveFocus()
    await user.keyboard('{Enter}')
    expect(sent()).toEqual({ ChooseMode: { Modes: [1] } })
  })
})

describe('several modes', () => {
  const two = { min: 2, max: 2 }

  it('asks for two, tells the progress and sends them in the order on the card', async () => {
    const { text, mode, confirm, user, answer, sent } = mount(two)
    expect(text()).toBe(`${spell.name}: choose two modes. Press 1 to 4 to choose a mode or take it back.`)
    expect(confirm()).toHaveAttribute('disabled')
    await user.click(mode(3))
    expect(text()).toContain('1 of 2 modes chosen. Choose 1 more.')
    expect(confirm()).toHaveAttribute('disabled')
    await user.click(mode(0))
    expect(text()).toContain('2 modes chosen. Confirm the modes, or take one back.')
    await user.click(must(confirm()))
    expect(answer).toHaveBeenCalledTimes(1)
    expect(sent()).toEqual({ ChooseMode: { Modes: [1, 6] } })
  })

  it('takes a chosen mode back with a second press, and closes the rest when the most are chosen', async () => {
    const { mode, user } = mount(two)
    await user.click(mode(0))
    await user.click(mode(1))
    const closed = (n: number): boolean => mode(n).getAttribute('aria-disabled') === 'true'
    expect([closed(2), closed(3), closed(0), closed(1)]).toEqual([true, true, false, false])
    expect([0, 1, 2, 3].map((n) => mode(n).getAttribute('aria-pressed'))).toEqual([
      'true',
      'true',
      'false',
      'false',
    ])
    await user.click(mode(1))
    expect([closed(2), closed(3)]).toEqual([false, false])
    expect(mode(1)).toHaveAttribute('aria-pressed', 'false')
  })

  it('toggles from the number keys', async () => {
    const { user, confirm, sent, answer } = mount(two)
    // The third key finds the most chosen already and does nothing.
    await user.keyboard('213')
    expect(sent()).toBeUndefined()
    await user.click(must(confirm()))
    expect(answer).toHaveBeenCalledTimes(1)
    expect(sent()).toEqual({ ChooseMode: { Modes: [1, 3] } })
  })

  it('confirms none when up to two are asked for', async () => {
    const { text, confirm, user, sent } = mount({ min: 0, max: 2 })
    expect(text()).toContain('choose up to two modes, or none.')
    expect(confirm()).not.toHaveAttribute('disabled')
    await user.click(must(confirm()))
    expect(sent()).toEqual({ ChooseMode: { Modes: [] } })
  })
})

describe('a mode that may repeat', () => {
  it('is chosen again by each press, counted, taken back by Undo and sent each time', async () => {
    const { mode, user, confirm, text, sent } = mount({
      min: 3,
      max: 3,
      repeat: true,
      modes: MODES.slice(0, 2),
    })
    expect(text()).toContain('choose three modes, a mode more than once if you like.')
    await user.click(mode(1))
    await user.click(mode(1))
    await user.click(mode(0))
    expect(mode(1)).toHaveTextContent('x2')
    expect(mode(0).getAttribute('aria-disabled')).toBe('true')
    await user.click(screen.getByRole('button', { name: 'Undo' }))
    expect(mode(0)).not.toHaveAttribute('aria-disabled', 'true')
    await user.click(mode(1))
    await user.click(must(confirm()))
    expect(sent()).toEqual({ ChooseMode: { Modes: [3, 3, 3] } })
  })
})
