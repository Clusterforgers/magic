/** Goal: a source's mana draws as symbols (one per mana, a count past three, the cost before a colon, a slash between ways, a five-colour disc for any colour, "sacrifice" in words) with one spoken name, and the mana pool shows each kind with its amount and its restricted mana apart, marked and in words. */
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ManaOption, ManaPool as PoolValue } from '@/protocol/generated'
import { cost, manaOfLetter, poolOf } from '@/test-utils/mana'
import { givesOfOptions } from './manaMade'
import { ManaMade } from './ManaMade'
import { ManaPool } from './ManaPool'

const option = (letter: string, extra: Partial<ManaOption> = {}, ability = 0): ManaOption => ({
  source: { object: 1, ability },
  produces: manaOfLetter(letter),
  ...extra,
})
const anyOf = (extra: Partial<ManaOption> = {}): ManaOption[] =>
  [...'WUBRG'].map((letter, ability) => option(letter, extra, ability))

describe('ManaMade', () => {
  it.each<[string, ManaOption[], string[], string]>([
    ['a basic land', [option('G')], ['G'], 'green'],
    ['Sol Ring', [option('C', { count: 2 })], ['C', 'C'], '2 colorless'],
    ['three of a kind', [option('C', { count: 3 })], ['C', 'C', 'C'], '3 colorless'],
    ['five of a kind is one symbol with a count', [option('G', { count: 5 })], ['G'], '5 green'],
    ['a bounce land', [option('W', { also: ['Blue'] })], ['W', 'U'], 'white and blue'],
    ['a choice of two colours', [option('R', {}, 0), option('W', {}, 1)], ['R', 'W'], 'red or white'],
    [
      'a Signet',
      [option('W', { also: ['Black'], cost: cost('{1}') })],
      ['1', 'W', 'B'],
      'pay 1 generic for white and black',
    ],
    [
      'a filter land',
      [option('W', { cost: cost('{W/U}') }), option('U', { cost: cost('{W/U}') }, 1)],
      ['W/U', 'W', 'U'],
      'pay 1 white or blue for white or blue',
    ],
    ['a source of any colour', anyOf(), ['any'], 'one mana of any color'],
    ['a Treasure', anyOf({ sacrifice: true }), ['any'], 'one mana of any color, sacrificing it'],
  ])('%s', (_name, options, symbols, spoken) => {
    const { container } = render(<ManaMade gives={givesOfOptions(options)} sizeClass="size-4" words />)
    expect([...container.querySelectorAll('[data-pip]')].map((pip) => pip.getAttribute('data-pip'))).toEqual(
      symbols,
    )
    expect(screen.getByRole('img')).toHaveAccessibleName(spoken)
  })

  it('says sacrifice beside a Treasure when asked to and not otherwise', () => {
    const gives = givesOfOptions(anyOf({ sacrifice: true }))
    const { container, rerender } = render(<ManaMade gives={gives} sizeClass="size-4" words />)
    expect(container).toHaveTextContent('sacrifice')
    rerender(<ManaMade gives={gives} sizeClass="size-4" />)
    expect(container).not.toHaveTextContent('sacrifice')
  })

  it('writes the count of a long run beside its symbol', () => {
    const { container } = render(
      <ManaMade gives={givesOfOptions([option('G', { count: 5 })])} sizeClass="size-4" />,
    )
    expect(container.querySelector('[data-count]')).toHaveTextContent('×5')
  })
})

describe('ManaPool', () => {
  const kinds = (container: HTMLElement) =>
    [...container.querySelectorAll('[data-mana]')].map((el) => [
      el.getAttribute('data-mana'),
      el.querySelector('.font-semibold')?.textContent,
      el.querySelector('.sr-only')?.textContent,
      el.hasAttribute('data-restricted'),
    ])

  const only = ['CostWithX' as const]
  it.each<[string, PoolValue, Array<[string, string, string, boolean]>]>([
    [
      'a green and two red',
      poolOf('RRG'),
      [
        ['Red', '×2', '2 red', false],
        ['Green', '×1', '1 green', false],
      ],
    ],
    ['colourless mana', poolOf('CCC'), [['Colorless', '×3', '3 colorless', false]]],
    [
      'restricted mana apart from the rest',
      {
        ...poolOf('G'),
        restricted: [
          { mana: 'Colorless', only },
          { mana: 'Colorless', only },
        ],
      },
      [
        ['Green', '×1', '1 green', false],
        ['Colorless', '×2', '2 colorless, restricted', true],
      ],
    ],
    [
      'restricted mana alone',
      { ...poolOf(''), restricted: [{ mana: 'Red', only }] },
      [['Red', '×1', '1 red, restricted', true]],
    ],
  ])('%s', (_name, pool, expected) => {
    const { container } = render(<ManaPool pool={pool} />)
    expect(kinds(container)).toEqual(expected)
  })

  it('draws nothing for an empty pool', () => {
    const { container } = render(<ManaPool pool={poolOf('')} />)
    expect(container).toBeEmptyDOMElement()
  })

  it('names the amounts for a screen reader', () => {
    render(<ManaPool pool={poolOf('RRG')} />)
    expect(screen.getByRole('group', { name: 'Mana pool' })).toHaveTextContent('2 red')
  })
})
