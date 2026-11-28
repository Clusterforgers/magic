/** Goal: in every table layout (grid, focus, the boss table) at two to four seats, the sector prompt lists the viewer's creatures with one toggle per sector, starts them spread over the sectors in turn with Confirm focused, moves a creature to the sector pressed, shows the summary and answers the sectors; the helpers spread, move and word the sectors. */
import { screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ObjectId, P170aSectorsOptions, Sector } from '@/protocol/generated'
import type { TableMode } from '../layout'
import { must } from '@/test-utils/batches'
import { creaturesOf, fixtureOf, renderDecision } from '@/test-utils/combatTable'
import { moveToSector, sectorWords, spreadSectors } from './sectorsText'

function optionsOf(seats: number): { options: P170aSectorsOptions; mine: ObjectId[] } {
  const mine = creaturesOf(fixtureOf(seats), 0).slice(0, 2)
  return {
    mine,
    options: { creatures: mine, text: 'Assign each of your creatures to the alpha, beta or gamma sector.' },
  }
}

function nameOf(seats: number, id: ObjectId): string {
  return must(fixtureOf(seats).view.objects.find((object) => object.id === id)).name
}

function mount(options: P170aSectorsOptions, seats = 2, mode?: TableMode) {
  return renderDecision(
    { P170aAssignSectors: options },
    { seats, options: mode === undefined ? {} : { mode } },
  )
}

describe.each<[number, TableMode | undefined]>([
  [2, undefined],
  [3, 'grid'],
  [4, 'focus'],
  [4, 'boss'],
])('at %i seats in the %s layout', (seats, mode) => {
  it('starts spread with Confirm focused and answers the sectors chosen', async () => {
    const { options, mine } = optionsOf(seats)
    const { answer, user, text } = mount(options, seats, mode)
    const [first, second] = [must(mine[0]), must(mine[1])]
    expect(text()).toContain(options.text)
    expect(screen.getByRole('button', { name: `${nameOf(seats, first)} in alpha` })).toHaveAttribute(
      'aria-pressed',
      'true',
    )
    expect(screen.getByRole('button', { name: `${nameOf(seats, second)} in beta` })).toHaveAttribute(
      'aria-pressed',
      'true',
    )
    expect(screen.getByRole('button', { name: 'Confirm sectors' })).toHaveFocus()
    await user.click(screen.getByRole('button', { name: `${nameOf(seats, second)} in gamma` }))
    expect(screen.getByRole('button', { name: `${nameOf(seats, second)} in beta` })).toHaveAttribute(
      'aria-pressed',
      'false',
    )
    expect(screen.getByText(new RegExp(`Gamma: ${nameOf(seats, second)}`))).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Confirm sectors' }))
    expect(answer).toHaveBeenCalledWith({
      P170aAssignSectors: [
        [first, 'Alpha'],
        [second, 'Gamma'],
      ],
    })
  })
})

describe('the sectors', () => {
  it.each<[ObjectId[], [ObjectId, Sector][]]>([
    [[], []],
    [[7], [[7, 'Alpha']]],
    [
      [7, 8, 9, 10],
      [
        [7, 'Alpha'],
        [8, 'Beta'],
        [9, 'Gamma'],
        [10, 'Alpha'],
      ],
    ],
  ])('spreads %j', (creatures, expected) => {
    expect(spreadSectors(creatures)).toEqual(expected)
  })

  it.each<[[ObjectId, Sector][], ObjectId, Sector, [ObjectId, Sector][]]>([
    [[[7, 'Alpha']], 7, 'Gamma', [[7, 'Gamma']]],
    [
      [
        [7, 'Alpha'],
        [8, 'Beta'],
      ],
      8,
      'Alpha',
      [
        [7, 'Alpha'],
        [8, 'Alpha'],
      ],
    ],
    [[[7, 'Alpha']], 9, 'Beta', [[7, 'Alpha']]],
  ])('moves in %j creature %i to %s', (sectors, creature, sector, expected) => {
    expect(moveToSector(sectors, creature, sector)).toEqual(expected)
  })

  it.each<[Sector, string[], string]>([
    ['Alpha', [], 'Alpha: none'],
    ['Beta', ['Bear', 'Ogre'], 'Beta: Bear, Ogre'],
  ])('words %s with %j', (sector, names, words) => {
    expect(sectorWords(sector, names)).toBe(words)
  })
})
