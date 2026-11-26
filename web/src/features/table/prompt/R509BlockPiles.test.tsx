/** Goal: in every table layout (grid, focus, the boss table) at two to four seats, the pile prompt lists the viewer's creatures with a toggle for each pile and the attackers by name; toggling builds the piles the summary shows and Confirm answers them; a creature joins no more piles than it may and one that can block additional creatures joins as many; No blocks and N answer with every pile empty, No blocks having the focus. */
import { screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ObjectId, R509PilesOptions } from '@/protocol/generated'
import type { TableMode } from '../layout'
import { must } from '@/test-utils/batches'
import { creaturesOf, fixtureOf, renderDecision } from '@/test-utils/combatTable'
import { confirmPilesLabel, togglePile } from './pilesText'

function optionsOf(seats: number, extra = false): { options: R509PilesOptions; mine: ObjectId[] } {
  const attackers = creaturesOf(fixtureOf(seats), 1).slice(0, 2)
  const mine = creaturesOf(fixtureOf(seats), 0).slice(0, 2)
  return {
    mine,
    options: {
      attackers,
      creatures: mine.map((object, at) => ({ object, piles: extra && at === 0 ? 2 : 1 })),
      text: 'Divide your creatures into 2 piles.',
    },
  }
}

function nameOf(seats: number, id: ObjectId): string {
  return must(fixtureOf(seats).view.objects.find((object) => object.id === id)).name
}

function mount(options: R509PilesOptions, seats = 2, mode?: TableMode) {
  return renderDecision({ R509BlockPiles: options }, { seats, options: mode === undefined ? {} : { mode } })
}

describe.each<[number, TableMode | undefined]>([
  [2, undefined],
  [3, 'grid'],
  [4, 'focus'],
  [4, 'boss'],
])('at %i seats in the %s layout', (seats, mode) => {
  it('names the attackers and builds the piles it confirms', async () => {
    const { options, mine } = optionsOf(seats)
    const { answer, user, text } = mount(options, seats, mode)
    const [first, second] = [must(mine[0]), must(mine[1])]
    expect(text()).toContain(`Attacking you: ${options.attackers.map((id) => nameOf(seats, id)).join(', ')}.`)
    await user.click(screen.getByRole('button', { name: `${nameOf(seats, first)} in pile 1` }))
    await user.click(screen.getByRole('button', { name: `${nameOf(seats, second)} in pile 2` }))
    expect(screen.getByText(new RegExp(`Pile 1: ${nameOf(seats, first)}`))).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Confirm piles (2 creatures)' }))
    expect(answer).toHaveBeenCalledWith({ R509BlockPiles: [[first], [second]] })
  })

  it('keeps a creature to the piles it may join', async () => {
    const { options, mine } = optionsOf(seats, true)
    const { user } = mount(options, seats, mode)
    const [many, one] = [nameOf(seats, must(mine[0])), nameOf(seats, must(mine[1]))]
    await user.click(screen.getByRole('button', { name: `${one} in pile 1` }))
    expect(screen.getByRole('button', { name: `${one} in pile 2` })).toBeDisabled()
    await user.click(screen.getByRole('button', { name: `${many} in pile 1` }))
    expect(screen.getByRole('button', { name: `${many} in pile 2` })).toBeEnabled()
  })
})

describe('no blocks', () => {
  it('answers every pile empty with the button, which has the focus, and with N', async () => {
    const { options } = optionsOf(2)
    const { answer, user } = mount(options)
    expect(screen.getByRole('button', { name: 'No blocks: every pile empty' })).toHaveFocus()
    await user.keyboard('n')
    expect(answer).toHaveBeenCalledWith({ R509BlockPiles: [[], []] })
  })
})

describe('the piles', () => {
  it.each<[ObjectId[][], number, number, number, ObjectId[][]]>([
    [[[], []], 0, 7, 1, [[7], []]],
    [[[7], []], 0, 7, 1, [[], []]],
    [[[7], []], 1, 7, 1, [[7], []]],
    [[[7], []], 1, 7, 2, [[7], [7]]],
  ])('toggles in %j pile %i creature %i (at most %i piles)', (piles, at, creature, most, expected) => {
    expect(togglePile(piles, at, creature, most)).toEqual(expected)
  })

  it.each<[ObjectId[][], string]>([
    [[[], []], 'Confirm empty piles'],
    [[[7], [7]], 'Confirm piles (1 creature)'],
    [[[7], [8]], 'Confirm piles (2 creatures)'],
  ])('labels %j', (piles, label) => {
    expect(confirmPilesLabel(piles)).toBe(label)
  })
})
