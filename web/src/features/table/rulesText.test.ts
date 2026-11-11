// Goal: rules text splits into its mana symbols, tap and untap marks, target phrases and plain words without losing a character, every symbol a card prints reads as its mana, an unknown brace stays text, and an ability splits into cost and effect at its first colon outside a reminder.
import { describe, expect, it } from 'vitest'
import { manaSymbolOf, rulesPieces, splitCost } from './rulesText'

describe('manaSymbolOf', () => {
  it.each([
    ['2', { Generic: 2 }],
    ['12', { Generic: 12 }],
    ['G', { Colored: 'Green' }],
    ['C', 'Colorless'],
    ['S', 'Snow'],
    ['W/U', { Hybrid: ['White', 'Blue'] }],
    ['2/R', { MonoHybrid: 'Red' }],
    ['B/P', { Phyrexian: 'Black' }],
    ['G/U/P', { HybridPhyrexian: ['Green', 'Blue'] }],
    ['C/W', { ColorlessHybrid: 'White' }],
    ['C/P', 'ColorlessPhyrexian'],
    ['X', null],
    ['E', null],
    ['W/W/W', null],
  ])('reads {%s}', (inner, symbol) => {
    expect(manaSymbolOf(inner)).toEqual(symbol)
  })
})

describe('rulesPieces', () => {
  it.each([
    ['{2}, {T}: Draw a card.', ['mana', 'text', 'tap', 'text']],
    ['{Q}: Untap it.', ['untap', 'text']],
    ['Deal 3 damage to any target.', ['text', 'target', 'text']],
    ['Destroy target creature, then scry 1.', ['text', 'target', 'text']],
    ['Pay {E}{E}: Draw.', ['text', 'text', 'text', 'text']],
    ['', []],
  ])('splits %j', (text, kinds) => {
    const pieces = rulesPieces(text)
    expect(pieces.map((piece) => piece.kind)).toEqual(kinds)
    expect(pieces.map((piece) => piece.text).join('')).toBe(text)
  })

  it('names the target phrase up to its comma', () => {
    const target = rulesPieces('Exile target nonland permanent, then draw.').find(
      (piece) => piece.kind === 'target',
    )
    expect(target?.text).toBe('target nonland permanent')
  })
})

describe('splitCost', () => {
  it.each([
    ['{2}, {T}: Draw a card.', '{2}, {T}', 'Draw a card.'],
    ['Draw a card.', '', 'Draw a card.'],
    ['{1}: Attach (Equip only as a sorcery: rules).', '{1}', 'Attach (Equip only as a sorcery: rules).'],
    ['(Reminder: none) Scry 1.', '', '(Reminder: none) Scry 1.'],
  ])('splits %j', (text, cost, effect) => {
    expect(splitCost(text)).toEqual({ cost, effect })
  })
})
