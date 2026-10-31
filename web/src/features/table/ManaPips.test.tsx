/** Goal: a row of mana symbols draws every kind of cost symbol the rules know (plain, hybrid split in two halves with no letters under the 12 px floor, Phyrexian with the phi mark), hides the drawing from a screen reader and names the whole cost, symbol by symbol, in words. */
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { cost } from '@/test-utils/mana'
import { costPips } from './cardModel'
import { ManaPips } from './ManaPips'

const SIZE = 'size-5 text-xs'

function draw(text: string) {
  render(<ManaPips pips={costPips(cost(text))} sizeClass={SIZE} label="Mana cost" />)
  const group = screen.getByRole('img')
  const pips = [...group.querySelectorAll<HTMLElement>('[data-pip]')]
  return { group, pips }
}

describe('ManaPips', () => {
  it('names the whole cost for a screen reader and leaves the drawing out of the tree', () => {
    const { group, pips } = draw('{2}{W/P}{G/U/P}')
    expect(group).toHaveAccessibleName(
      'Mana cost: 2 generic, white mana or 2 life, green or blue mana or 2 life',
    )
    for (const pip of pips) expect(pip).toHaveAttribute('aria-hidden', 'true')
  })

  it.each([
    ['{3}', ['3']],
    ['{G}{G}{U}', ['U', 'G', 'G']],
    ['{C}', ['C']],
    ['', ['0']],
  ])('draws the plain cost %j with one disc and one letter each', (text, shown) => {
    const { pips } = draw(text)
    expect(pips.map((pip) => pip.textContent)).toEqual(shown)
    for (const pip of pips) expect(pip.className).not.toMatch(/bg-linear/)
  })

  it.each([
    ['{W/U}', 'from-mana-white', 'to-mana-blue'],
    ['{G/W}', 'from-mana-green', 'to-mana-white'],
    ['{R/G}', 'from-mana-red', 'to-mana-green'],
    ['{C/W}', 'from-mana-colorless', 'to-mana-white'],
    ['{2/W}', 'from-muted', 'to-mana-white'],
    ['{2/B}', 'from-muted', 'to-mana-black'],
  ])('draws %s as a disc split in two halves with no letter, whose name tells the colours', (text, from, to) => {
    const [pip] = draw(text).pips
    expect(pip?.className).toContain('bg-linear-to-br')
    expect(pip?.className).toContain(from)
    expect(pip?.className).toContain(to)
    expect(pip?.textContent).toBe('')
  })

  it.each([
    ['{W/P}', 'bg-mana-white'],
    ['{R/P}', 'bg-mana-red'],
    ['{C/P}', 'bg-mana-colorless'],
  ])('draws %s as a disc of its colour with the phi mark', (text, fill) => {
    const [pip] = draw(text).pips
    expect(pip?.className).toContain(fill)
    expect(pip?.textContent).toBe('ϕ')
  })

  it('draws a hybrid Phyrexian symbol split in two with the phi mark between', () => {
    const [pip] = draw('{G/U/P}').pips
    expect(pip?.className).toContain('bg-linear-to-br')
    expect(pip?.className).toContain('from-mana-green')
    expect(pip?.className).toContain('to-mana-blue')
    expect(pip?.textContent).toBe('ϕ')
  })

  it('gives each symbol its printed form to find it by', () => {
    const { pips } = draw('{1}{R/W}{2/G}{B/P}{G/U/P}{C/W}{C/P}')
    expect(pips.map((pip) => pip.getAttribute('data-pip'))).toEqual([
      '1',
      'C/W',
      'R/W',
      '2/G',
      'B/P',
      'C/P',
      'G/U/P',
    ])
  })
})
