/** Goal: in every table layout (grid, focus, the boss table) at two to four seats, the dice prompt states the question and the roll; a reroll toggles any dice, confirms them with the cost on the button, keeps the results with Keep or K, which has the focus; the roll to ignore answers with the die pressed or its number key; and the words of the roll read right for one die and several. */
import { screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { R706DiceOptions } from '@/protocol/generated'
import type { TableMode } from '../layout'
import { renderDecision } from '@/test-utils/combatTable'
import { rerollLabel, resultsWords, rollWords } from './diceText'

const reroll: R706DiceOptions = {
  roller: 0,
  sides: 6,
  results: [2, 5, 1],
  min: 0,
  max: 3,
  purpose: { Reroll: '{1}' },
  text: 'Pay {1} to reroll any of these dice?',
}

const ignore: R706DiceOptions = {
  roller: 0,
  sides: 6,
  results: [6, 3],
  min: 1,
  max: 1,
  purpose: 'Ignore',
  text: 'Choose one of the rolls to ignore.',
}

function mount(options: R706DiceOptions, seats = 2, mode?: TableMode) {
  return renderDecision(
    { R706ChooseDice: options },
    { seats, options: mode === undefined ? {} : { mode } },
  )
}

describe.each<[number, TableMode | undefined]>([
  [2, undefined],
  [3, 'grid'],
  [4, 'focus'],
  [4, 'boss'],
])('at %i seats in the %s layout', (seats, mode) => {
  it('states the question, and the roll when another player chooses', () => {
    const first = mount(reroll, seats, mode)
    expect(first.text()).toContain('Pay {1} to reroll any of these dice?')
    first.unmount()
    expect(mount(ignore, seats, mode).text()).toContain('You rolled 6 and 3 on six-sided dice.')
  })

  it('rerolls the dice toggled, the cost on the button', async () => {
    const { answer, user } = mount(reroll, seats, mode)
    const confirm = screen.getByRole('button', { name: 'Choose dice to reroll' })
    expect(confirm).toBeDisabled()
    await user.click(screen.getByRole('button', { name: 'Die 3: 1' }))
    await user.click(screen.getByRole('button', { name: 'Die 1: 2' }))
    expect(screen.getByRole('button', { name: 'Die 1: 2' })).toHaveAttribute('aria-pressed', 'true')
    await user.click(screen.getByRole('button', { name: 'Reroll 2 dice (pay {1})' }))
    expect(answer).toHaveBeenCalledWith({ R706ChooseDice: [0, 2] })
  })

  it('ignores the die pressed', async () => {
    const { answer, user } = mount(ignore, seats, mode)
    expect(screen.getByRole('button', { name: 'Die 1: 6' })).toHaveFocus()
    await user.click(screen.getByRole('button', { name: 'Die 2: 3' }))
    expect(answer).toHaveBeenCalledWith({ R706ChooseDice: [1] })
  })
})

describe('the keyboard', () => {
  it('keeps the results with K, Keep having the focus', async () => {
    const { answer, user } = mount(reroll)
    expect(screen.getByRole('button', { name: 'Keep the results' })).toHaveFocus()
    await user.keyboard('k')
    expect(answer).toHaveBeenCalledWith({ R706ChooseDice: [] })
  })

  it('toggles a die with its number and ignores one with its number', async () => {
    const first = mount(reroll)
    await first.user.keyboard('2')
    expect(screen.getByRole('button', { name: 'Die 2: 5' })).toHaveAttribute('aria-pressed', 'true')
    first.unmount()
    const second = mount(ignore)
    await second.user.keyboard('1')
    expect(second.answer).toHaveBeenCalledWith({ R706ChooseDice: [0] })
  })
})

describe('the words', () => {
  it.each<[number[], string]>([
    [[4], '4'],
    [[4, 2], '4 and 2'],
    [[1, 2, 3], '1, 2 and 3'],
  ])('lists %j as %s', (results, words) => {
    expect(resultsWords(results)).toBe(words)
  })

  it.each<[number, number[], string]>([
    [6, [4], 'Ana rolled a 4 on a six-sided die.'],
    [20, [17, 3], 'Ana rolled 17 and 3 on 20-sided dice.'],
  ])('says a d%i roll of %j', (sides, results, words) => {
    expect(rollWords('Ana', sides, results)).toBe(words)
  })

  it.each<[number, string, string]>([
    [0, '{1}', 'Choose dice to reroll'],
    [1, '{1}', 'Reroll 1 die (pay {1})'],
    [3, '', 'Reroll 3 dice'],
  ])('labels %i dice chosen', (chosen, cost, label) => {
    expect(rerollLabel(chosen, cost)).toBe(label)
  })
})
