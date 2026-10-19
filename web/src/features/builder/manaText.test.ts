// Goal: a mana cost written as text becomes the same symbols the table draws for every kind of symbol (generic, coloured, colourless, hybrid, Phyrexian), a symbol the game has no shape for (X, snow) still gets a named disc, and text without braces gives no symbols.
import { describe, expect, it } from 'vitest'
import { faceCosts, pipsOf } from './manaText'

describe('pipsOf', () => {
  it.each([
    ['{R}', [{ tone: 'red', text: 'R', name: 'red' }]],
    [
      '{2}{U}',
      [
        { tone: 'generic', text: '2', name: '2 generic' },
        { tone: 'blue', text: 'U', name: 'blue' },
      ],
    ],
    ['{0}', [{ tone: 'generic', text: '0' }]],
    ['{12}', [{ tone: 'generic', text: '12' }]],
    ['{W/U}', [{ tone: 'white', split: 'blue', text: 'W', text2: 'U' }]],
    ['{2/G}', [{ tone: 'generic', split: 'green', text: '2', text2: 'G' }]],
    ['{B/P}', [{ tone: 'black', text: 'ϕ', phyrexian: true }]],
    ['{R/G/P}', [{ tone: 'red', split: 'green', text: 'ϕ', phyrexian: true }]],
    ['{C}', [{ tone: 'colorless', text: 'C' }]],
    ['{C/W}', [{ tone: 'colorless', split: 'white', text: 'C', text2: 'W' }]],
    ['{C/P}', [{ tone: 'colorless', text: 'ϕ', phyrexian: true }]],
    ['{X}{R}{R}', [{ tone: 'generic', text: 'X', name: 'X' }, { tone: 'red' }, { tone: 'red' }]],
    ['{S}', [{ tone: 'generic', text: 'S', name: 'snow' }]],
    ['{HW}', [{ tone: 'generic', text: 'HW', name: 'HW' }]],
    ['{W/X}', [{ tone: 'generic', symbol: 'W/X' }]],
    ['{A/B/C/D}', [{ tone: 'generic', text: 'A/' }]],
  ])('reads %s', (cost, expected) => {
    expect(pipsOf(cost)).toMatchObject(expected)
  })

  it.each(['', 'no braces', '{}', '{', 'W'])('finds no symbols in %j', (cost) => {
    expect(pipsOf(cost)).toEqual([])
  })

  it('keeps the symbols in card order with a key each, even when two are equal', () => {
    const pips = pipsOf('{X}{X}{R}{R}')
    expect(pips.map((pip) => pip.symbol)).toEqual(['X', 'X', 'R', 'R'])
    expect(new Set(pips.map((pip) => pip.key)).size).toBe(4)
  })
})

describe('faceCosts', () => {
  it('splits a double-faced card at its two slashes', () => {
    const faces = faceCosts('{1}{W} // {2}{U}')
    expect(faces.map((face) => face.map((pip) => pip.symbol))).toEqual([
      ['1', 'W'],
      ['2', 'U'],
    ])
  })

  it('leaves out a face without a cost', () => {
    expect(faceCosts(' // {R}')).toHaveLength(1)
    expect(faceCosts('{R} // ')).toHaveLength(1)
  })

  it('gives nothing for a card without a cost', () => {
    expect(faceCosts('')).toEqual([])
  })
})
