/** Goal: the payment in the Stack column groups the mana sources by what a tap gives (Sol Ring two colourless, a Signet with its cost, a Treasure and a Command Tower of any colour, a bounce land both mana) with their marks (restricted mana, life, energy, another permanent), equal permanents share a row with a count, a row has a button for each kind its source makes, a row in use says what it pays and offers Untap and its other kinds, the circles count what is paid, and floating mana the pool keeps says so. */
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
import { groupByMade, groupWords } from './PayColumn'

function open(name = 'pay-sources') {
  const game = fixtureGame({ seats: 2, prompt: name })
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
const column = (): HTMLElement => must(document.querySelector<HTMLElement>('[data-slot="pay-column"]'))
const rows = (): HTMLElement[] => [...column().querySelectorAll<HTMLElement>('[data-slot="pay-source-row"]')]
const row = (name: string): HTMLElement =>
  must(rows().find((el) => el.textContent?.startsWith(name.slice(0, 1) + name)))
const labels = (): (string | null)[] =>
  [...column().querySelectorAll('[data-slot="pay-group-label"]')].map((label) =>
    label.getAttribute('aria-label'),
  )
const tap = (name: RegExp): HTMLElement => within(column()).getByRole('button', { name })
const paid = (): string | null =>
  must(document.querySelector('[data-slot="pay-meter"]')).getAttribute('aria-label')
const captions = (): string[] =>
  [...column().querySelectorAll('[data-slot="pay-caption"]')].map((caption) => caption.textContent ?? '')

describe('the sources of ?prompt=pay-sources', () => {
  it('groups them by what a tap gives: basic lands, Sol Ring, a Signet, a Treasure, a Command Tower and a bounce land', () => {
    open()
    expect(labels()).toEqual(
      expect.arrayContaining([
        'Makes 2 colorless',
        'Makes white and black, costs 1 generic',
        'Makes one mana of any color, you choose, sacrifice',
        'Makes one mana of any color, you choose',
        'Makes white and blue',
      ]),
    )
    // A source of one kind has one button; a source of any colour one for each.
    expect(within(row('Sol Ring')).getAllByRole('button')).toHaveLength(1)
    expect(
      within(row('Command Tower'))
        .getAllByRole('button')
        .map((button) => button.getAttribute('aria-label')),
    ).toEqual(['white', 'blue', 'black', 'red', 'green'].map((colour) => `Tap Command Tower for ${colour}`))
  })

  it('adds a source with a press, counts it in the circles and takes it back with Untap', async () => {
    const { user } = open()
    expect(paid()).toBe('0 of 4 paid')
    await user.click(tap(/^Tap Sol Ring for 2 colorless/))
    expect(row('Sol Ring')).toHaveAttribute('data-used')
    expect(row('Sol Ring')).toHaveTextContent('paying')
    expect(paid()).toBe('2 of 4 paid')
    expect(captions().filter((caption) => caption === 'Sol Ring')).toHaveLength(2)
    await user.click(tap(/^Untap Sol Ring/))
    expect(row('Sol Ring')).not.toHaveAttribute('data-used')
    expect(paid()).toBe('0 of 4 paid')
  })

  it('takes the colour of a source of any colour from its row, and changes it there without untapping', async () => {
    const { user } = open()
    await user.click(tap(/^Tap Command Tower for blue/))
    expect(
      within(column()).getByRole('button', { name: 'Remove blue from Command Tower' }),
    ).toBeInTheDocument()
    expect(row('Command Tower')).toHaveTextContent('paying')
    await user.click(tap(/^Switch Command Tower to green/))
    expect(
      within(column()).getByRole('button', { name: 'Remove green from Command Tower' }),
    ).toBeInTheDocument()
    expect(within(column()).queryByRole('button', { name: 'Remove blue from Command Tower' })).toBeNull()
    // A source of one kind has no other kind to switch to.
    await user.click(tap(/^Tap Sol Ring for/))
    expect(within(row('Sol Ring')).queryByRole('button', { name: /^Switch/ })).toBeNull()
  })

  it('pays {2}{W}{G} with Sol Ring, a Treasure for white and a Command Tower for green, and sends three taps', async () => {
    const { user, answer } = open()
    await user.click(tap(/^Tap Sol Ring for/))
    await user.click(tap(/^Tap Treasure for white/))
    await user.click(tap(/^Tap Command Tower for green/))
    expect(paid()).toBe('4 of 4 paid')
    await user.click(screen.getByRole('button', { name: 'Confirm payment' }))
    const [sent] = answer.mock.calls[0] as [Answer]
    const payment =
      typeof sent === 'object' &&
      'PayMana' in sent &&
      typeof sent.PayMana === 'object' &&
      'Manual' in sent.PayMana
        ? sent.PayMana.Manual
        : null
    expect(payment?.taps).toHaveLength(3)
  })

  it('says why a row is off: a Signet waits for the mana of its own cost', async () => {
    const { user } = open()
    expect(row('Orzhov Signet')).toHaveAttribute('data-off')
    expect(row('Orzhov Signet')).toHaveTextContent('Tap another source for its cost first')
    await user.click(tap(/^Tap Sol Ring for/))
    expect(row('Orzhov Signet')).not.toHaveAttribute('data-off')
  })
})

/** The viewer's untapped permanent on the board whose spoken name starts with `name`, marked for the payment. */
const onBoard = (name: RegExp): HTMLElement =>
  must(
    [...document.querySelectorAll<HTMLElement>('button[data-object-id][data-highlight]')].find((card) =>
      name.test(card.getAttribute('aria-label') ?? ''),
    ),
  )

describe('a press on a card of the board pays with it', () => {
  it.each<[string, RegExp, string | null, string]>([
    ['a basic land', /^(Plains|Forest), Basic Land/, null, '1 of 4 paid'],
    ['a land of any colour', /^Command Tower,/, 'green', '1 of 4 paid'],
    ['a land of two colours', /^Azorius Chancery,/, null, '2 of 4 paid'],
    ['Sol Ring', /^Sol Ring,/, null, '2 of 4 paid'],
  ])('%s: the card is pressed, its row lit or used, and the circles fill', async (_, name, colour, after) => {
    const { user } = open()
    const card = onBoard(name)
    expect(card).toHaveAttribute('data-highlight', 'legal')
    await user.click(card)
    if (colour !== null) {
      // A source of several kinds lights its row in the column, which asks for the kind.
      const lit = must(document.querySelector<HTMLElement>('[data-slot="pay-source-row"][data-lit]'))
      await user.click(within(lit).getByRole('button', { name: `Add ${colour} mana` }))
    }
    expect(card).toHaveAttribute('aria-pressed', 'true')
    expect(paid()).toBe(after)
    // A second press takes it back.
    await user.click(card)
    expect(card).toHaveAttribute('aria-pressed', 'false')
    expect(paid()).toBe('0 of 4 paid')
  })

  it('takes a Signet once its own cost is paid by a land pressed before it', async () => {
    const { user } = open()
    await user.click(onBoard(/^Sol Ring,/))
    const signet = onBoard(/^Orzhov Signet,/)
    await user.click(signet)
    expect(signet).toHaveAttribute('aria-pressed', 'true')
    expect(row('Orzhov Signet')).toHaveAttribute('data-used')
  })

  it('still lets the game pay with "Pay for me"', async () => {
    const { user, answer } = open()
    await user.click(screen.getByRole('button', { name: 'Pay for me' }))
    expect(answer).toHaveBeenCalledWith({ PayMana: 'Auto' })
  })
})

describe('the marks of ?prompt=pay-marks', () => {
  it.each<[string, string, string]>([
    ['Ancient Ziggurat', 'only', 'mana only for creature spells'],
    ['Mana Confluence', 'life', 'pays 1 life'],
    ['Aether Hub', 'energy', 'pays 1 energy for some mana'],
    ['Springleaf Drum', 'tap', 'taps 1 more permanent'],
  ])('marks the group of %s', (name, mark, words) => {
    open('pay-marks')
    const group = must(row(name).closest('[data-slot="pay-group"]'))
    const label = must(group.querySelector('[data-slot="pay-group-label"]'))
    expect([...label.querySelectorAll('[data-mark]')].map((el) => el.getAttribute('data-mark'))).toEqual([
      mark,
    ])
    expect(label.getAttribute('aria-label')).toContain(words)
  })

  it('marks the green that the pool keeps to the end of the turn', () => {
    open('pay-marks')
    expect(
      within(within(column()).getByRole('group', { name: 'Floating mana' })).getByRole('button', {
        name: /^Spend green from your pool/,
      }),
    ).toHaveAccessibleName(/kept: 1 until end of turn$/)
  })
})

describe('the groups', () => {
  const forest = (object: number): ManaOption[] => [source(object, 'G')]
  const sources = new Map<number, readonly ManaOption[]>([
    [1, forest(1)],
    [2, forest(2)],
    [3, [{ ...source(3, 'C'), count: 2 }]],
    [4, [source(4, 'W', 0), source(4, 'U', 1)]],
    [5, [{ ...source(5, 'W'), also: ['Blue'], cost: [{ Generic: 1 }] }]],
  ])
  const names: Record<number, string> = {
    1: 'Forest',
    2: 'Forest',
    3: 'Sol Ring',
    4: 'Fountain',
    5: 'Signet',
  }
  const nameOf = (id: number): string => names[id] ?? 'a permanent'

  it('puts equal permanents on one row and every kind of source in its own group', () => {
    const groups = groupByMade(sources, nameOf)
    expect(groups.map((group) => group.rows.map((one) => [one.name, one.ids]))).toEqual([
      [['Forest', [1, 2]]],
      [['Sol Ring', [3]]],
      [['Fountain', [4]]],
      [['Signet', [5]]],
    ])
    expect(groups.map((group) => groupWords(group.gives, group.marks))).toEqual([
      'Makes green',
      'Makes 2 colorless',
      'Makes white or blue, you choose',
      'Makes white and blue, costs 1 generic',
    ])
  })
})
