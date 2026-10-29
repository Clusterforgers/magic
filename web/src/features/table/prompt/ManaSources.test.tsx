/** Goal: the payment prompt lists each mana source with the symbols of what a tap gives (Sol Ring two colourless, a Signet with its cost, a Treasure and a Command Tower of any colour, a bounce land both mana), equal sources share a chip with a count, a click adds or takes back like the card does, the colour of a source of several is picked in a small switch, and the running total says what the payment has made so far. */
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { GameContext } from '@/features/game/gameContext'
import { GameTable } from '@/features/table/GameTable'
import { TableOptionsContext } from '@/features/table/tableOptions'
import type { Answer, ManaOption } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { source } from '@/test-utils/mana'
import { fixtureGame } from '../dev/devGame'
import { ManaSources, groupSources } from './ManaSources'

function open() {
  const game = fixtureGame({ seats: 2, prompt: 'pay-sources' })
  const answer = vi.spyOn(game.session, 'answer')
  render(
    <GameContext value={game}>
      <TableOptionsContext value={{}}>
        <GameTable />
      </TableOptionsContext>
    </GameContext>,
  )
  return { answer, user: userEvent.setup() }
}
const chips = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>('[data-slot="pay-source"]')]
const chip = (label: string): HTMLElement =>
  must(chips().find((el) => el.getAttribute('aria-label')?.startsWith(label)))
const pips = (el: HTMLElement): (string | null)[] =>
  [...el.querySelectorAll('[data-pip]')].map((pip) => pip.getAttribute('data-pip'))
const prompt = (): HTMLElement => must(document.querySelector<HTMLElement>('[data-decision="PayMana"]'))
const row = (name: string): HTMLElement => within(prompt()).getByRole('group', { name })

describe('the sources of ?prompt=pay-sources', () => {
  it('shows what a tap gives on each chip: Sol Ring, a Signet, a Treasure, a Command Tower and a bounce land', () => {
    open()
    const expected: [string, string, string[], boolean][] = [
      ['Tap Sol Ring for', '2 colorless', ['C', 'C'], false],
      ['Tap Orzhov Signet for', 'pay 1 generic for white and black', ['1', 'W', 'B'], false],
      ['Tap Treasure for', 'one mana of any color, sacrificing it', ['any'], true],
      ['Tap Command Tower for', 'one mana of any color', ['any'], false],
      ['Tap Azorius Chancery for', 'white and blue', ['W', 'U'], false],
    ]
    for (const [start, spoken, symbols, sacrifice] of expected) {
      const el = chip(start)
      expect(el).toHaveAccessibleName(`${start} ${spoken}`)
      expect(pips(el)).toEqual(symbols)
      expect(el.textContent?.includes('sacrifice')).toBe(sacrifice)
    }
  })

  it('adds a source with a click, shows the total made against what is still owed, and takes it back', async () => {
    const { user } = open()
    expect(within(row('Made so far')).getByText('nothing yet')).toBeInTheDocument()
    await user.click(chip('Tap Sol Ring for'))
    expect(chip('Tap Sol Ring for')).toHaveAttribute('aria-pressed', 'true')
    expect(within(row('Made so far')).getByRole('img')).toHaveAccessibleName(
      'Mana made: colorless, colorless',
    )
    expect(within(row('Still owed')).getByRole('img')).toHaveAccessibleName('Mana still owed: white, green')
    await user.click(chip('Tap Sol Ring for'))
    expect(chip('Tap Sol Ring for')).toHaveAttribute('aria-pressed', 'false')
    expect(within(row('Made so far')).getByText('nothing yet')).toBeInTheDocument()
  })

  it('picks the colour of a source of any colour in the switch and shows what it will give', async () => {
    const { user } = open()
    await user.click(chip('Tap Command Tower for'))
    const choice = within(prompt()).getByRole('group', { name: 'Mana from Command Tower' })
    expect(
      within(choice)
        .getAllByRole('button', { name: /^Add / })
        .map((button) => button.getAttribute('aria-label')),
    ).toEqual(['Add white mana', 'Add blue mana', 'Add black mana', 'Add red mana', 'Add green mana'])
    await user.click(within(choice).getByRole('button', { name: 'Add blue mana' }))
    expect(within(prompt()).queryByRole('group', { name: 'Mana from Command Tower' })).toBeNull()
    expect(
      within(prompt()).getByRole('button', { name: 'Remove blue from Command Tower' }),
    ).toBeInTheDocument()
    expect(within(row('Made so far')).getByRole('img')).toHaveAccessibleName('Mana made: blue')
    expect(chip('Tap Command Tower for')).toHaveAttribute('aria-pressed', 'true')
  })

  it('pays {2}{W}{G} with Sol Ring, a Treasure for white and a Command Tower for green, and sends three taps', async () => {
    const { user, answer } = open()
    await user.click(chip('Tap Sol Ring for'))
    await user.click(chip('Tap Treasure for'))
    await user.click(within(prompt()).getByRole('button', { name: 'Add white mana' }))
    await user.click(chip('Tap Command Tower for'))
    // Only green is still owed, so the Tower adds it without asking.
    expect(within(row('Still owed')).getByText('nothing, the cost is covered')).toBeInTheDocument()
    await user.click(within(prompt()).getByRole('button', { name: 'Confirm payment' }))
    const [sent] = answer.mock.calls[0] as [Answer]
    const payment =
      'PayMana' in sent && typeof sent.PayMana === 'object' && 'Manual' in sent.PayMana
        ? sent.PayMana.Manual
        : null
    expect(payment?.taps).toHaveLength(3)
  })
})

describe('ManaSources', () => {
  const forest = (object: number): ManaOption[] => [source(object, 'G')]
  const sources = new Map<number, readonly ManaOption[]>([
    [1, forest(1)],
    [2, forest(2)],
    [3, [{ ...source(3, 'C'), count: 2 }]],
  ])
  const names: Record<number, string> = { 1: 'Forest', 2: 'Forest', 3: 'Sol Ring' }
  const nameOf = (id: number): string => names[id] ?? 'a permanent'

  it('puts equal sources on one chip with their number', () => {
    expect(groupSources(sources, nameOf).map((group) => [group.name, group.ids])).toEqual([
      ['Forest', [1, 2]],
      ['Sol Ring', [3]],
    ])
  })

  it.each<[string, number[], number[], number | undefined, string]>([
    ['adds the first free one', [1, 2, 3], [], 1, 'Tap a Forest for green, 0 of 2 used'],
    ['adds the next free one', [1, 2, 3], [1], 2, 'Tap a Forest for green, 1 of 2 used'],
    ['takes the last one back when none is free', [3], [1, 2], 2, 'Tap a Forest for green, 2 of 2 used'],
    ['does nothing when none is free or used', [3], [], undefined, 'Tap a Forest for green, 0 of 2 used'],
  ])('a click %s', async (_name, legal, taken, called, label) => {
    const onAct = vi.fn()
    render(<ManaSources sources={sources} nameOf={nameOf} legal={legal} taken={taken} onAct={onAct} />)
    const forestChip = screen.getByRole('button', { name: label })
    expect(forestChip).toHaveAttribute('aria-disabled', String(called === undefined))
    await userEvent.setup().click(forestChip)
    expect(onAct.mock.calls).toEqual(called === undefined ? [] : [[called]])
  })
})
