/** Goal: for lists of 1 to 11 options (colors with their symbol, players with the viewer's own seat named, labels, equal labels) the prompt asks in the card's words, draws a button per option with its number key for the first ten, answers the index from a click or its key exactly once, starts with focus on the first option (the one the clock takes), and leaves the keys to a text field. */
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ChoiceOption } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { creaturesOf, fixtureOf, renderDecision } from '@/test-utils/combatTable'

const source = must(creaturesOf(fixtureOf(2), 0)[0])
const sourceName = must(fixtureOf(2).view.objects.find((object) => object.id === source)).name

const labels = (count: number): ChoiceOption[] =>
  Array.from({ length: count }, (_, n) => ({ Label: `Option ${n + 1}` }))

function mount(options: ChoiceOption[], text = 'Choose a creature type.', seats = 2) {
  const table = renderDecision({ ChooseOption: { source, options, text } }, { seats })
  const list = (): HTMLElement => screen.getByRole('group', { name: 'Options' })
  const buttons = (): HTMLElement[] => within(list()).getAllByRole('button')
  return { ...table, list, buttons }
}

describe('the list', () => {
  it.each([1, 2, 5, 9, 10, 11])('of %i options has a button each and a key for the first ten', (count) => {
    const { buttons } = mount(labels(count))
    expect(buttons().map((b) => b.getAttribute('aria-label'))).toEqual(
      labels(count).map((o) => ('Label' in o ? o.Label : '')),
    )
    const keys = buttons().map((b) => b.getAttribute('aria-keyshortcuts'))
    expect(keys).toEqual(
      ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0', undefined].slice(0, count).map((k) => k ?? null),
    )
    expect(
      buttons()
        .flatMap((b) => [...b.querySelectorAll('[data-slot="key-hint"]')])
        .map((k) => k.textContent),
    ).toEqual(['1', '2', '3', '4', '5', '6', '7', '8', '9', '0'].slice(0, count))
  })

  it.each([
    [3, 'Choose a creature type.', `${sourceName}: Choose a creature type. Press 1 to 3 to choose.`],
    [2, '', `${sourceName}: choose one. Press 1 or 2 to choose.`],
    [1, 'Name a card.', `${sourceName}: Name a card. Press 1 to choose it.`],
  ])('of %i with the words "%s" reads "%s"', (count, text, sentence) => {
    expect(mount(labels(count), text).text()).toBe(sentence)
  })

  it('draws a color with its symbol and a label without one', () => {
    const { buttons } = mount([{ Color: 'Red' }, { Label: 'Goblin' }])
    const dots = (b: HTMLElement): number => b.querySelectorAll('span[aria-hidden="true"]').length
    expect(buttons().map(dots)).toEqual([1, 0])
    expect(buttons().map((b) => b.getAttribute('aria-label'))).toEqual(['Red', 'Goblin'])
  })

  it("names the players, the viewer's own seat among them", () => {
    const { buttons } = mount([{ Player: 0 }, { Player: 1 }, { Player: 2 }], 'Choose a player.', 3)
    const names = buttons().map((b) => b.getAttribute('aria-label'))
    expect(names[0]).toMatch(/ \(you\)$/)
    expect(names[1]).not.toMatch(/\(you\)/)
    expect(new Set(names).size).toBe(3)
  })

  it('marks no card on the table', () => {
    expect(mount(labels(3)).marks().size).toBe(0)
  })
})

describe('answering', () => {
  it.each([
    [1, 0],
    [5, 4],
    [10, 9],
  ])('a click on button %i of 11 sends the index %i', async (place, index) => {
    const { buttons, user, answer } = mount(labels(11))
    await user.click(must(buttons()[place - 1]))
    expect(answer).toHaveBeenCalledTimes(1)
    expect(answer.mock.calls[0]?.[0]).toEqual({ ChooseOption: index })
  })

  it('answers the eleventh, which has no key, from a click', async () => {
    const { buttons, user, answer } = mount(labels(11))
    await user.click(must(buttons()[10]))
    expect(answer.mock.calls[0]?.[0]).toEqual({ ChooseOption: 10 })
  })

  it.each([
    ['1', 0],
    ['3', 2],
    ['9', 8],
    ['0', 9],
  ])('the key %s sends the index %i', async (key, index) => {
    const { user, answer } = mount(labels(11))
    await user.keyboard(key)
    expect(answer).toHaveBeenCalledTimes(1)
    expect(answer.mock.calls[0]?.[0]).toEqual({ ChooseOption: index })
  })

  it('ignores a key with no option, and answers only once', async () => {
    const { user, answer } = mount(labels(3))
    await user.keyboard('7')
    expect(answer).not.toHaveBeenCalled()
    await user.keyboard('21')
    expect(answer).toHaveBeenCalledTimes(1)
    expect(answer.mock.calls[0]?.[0]).toEqual({ ChooseOption: 1 })
  })

  it('tells equal labels apart by their place', async () => {
    const { buttons, user, answer } = mount([{ Label: 'Goblin' }, { Label: 'Goblin' }])
    await user.click(must(buttons()[1]))
    expect(answer.mock.calls[0]?.[0]).toEqual({ ChooseOption: 1 })
  })

  it('leaves the digits to a text field', async () => {
    const { user, answer } = mount(labels(3))
    const field = document.createElement('input')
    document.body.append(field)
    field.focus()
    await user.keyboard('12')
    field.remove()
    expect(answer).not.toHaveBeenCalled()
  })
})

describe('focus', () => {
  it('starts on the first option, so Enter at once takes what the clock takes', async () => {
    const { buttons, user, answer } = mount(labels(4))
    expect(buttons()[0]).toHaveFocus()
    await user.keyboard('{Enter}')
    expect(answer.mock.calls[0]?.[0]).toEqual({ ChooseOption: 0 })
  })
})
