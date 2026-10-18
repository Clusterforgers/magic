/**
 * The symbols of a mana cost (CR 107.4) as the client needs them: how each is written, what it is worth and every
 * way it can be paid. A port of `ManaSymbol` in `mtg-core`; the server sends costs in canonical order.
 */
import type { Color, ManaSymbol, ManaType } from '@/protocol/generated'

/** Life paid for one Phyrexian symbol (CR 107.4f). */
export const LIFE_PER_SYMBOL = 2

const LETTER: Record<Color, string> = { White: 'W', Blue: 'U', Black: 'B', Red: 'R', Green: 'G' }

/** The letter of a colour as costs write it: W, U, B, R or G. */
export function colorLetter(color: Color): string {
  return LETTER[color]
}

/** One way to pay a symbol: one mana of a kind, some generic mana, or some life. */
export type Way =
  { kind: 'mana'; mana: ManaType } | { kind: 'generic'; count: number } | { kind: 'life'; amount: number }

/** The symbol's single kind of payload, so one place tells the shapes apart. */
export type SymbolShape =
  | { kind: 'generic'; count: number }
  | { kind: 'colored'; color: Color }
  | { kind: 'colorless' }
  | { kind: 'hybrid'; colors: [Color, Color] }
  | { kind: 'monoHybrid'; color: Color }
  | { kind: 'phyrexian'; color: Color }
  | { kind: 'hybridPhyrexian'; colors: [Color, Color] }
  | { kind: 'colorlessHybrid'; color: Color }
  | { kind: 'colorlessPhyrexian' }

/** Reads a symbol off the wire into a flat shape. */
export function shapeOf(symbol: ManaSymbol): SymbolShape {
  if (symbol === 'Colorless') return { kind: 'colorless' }
  if (symbol === 'ColorlessPhyrexian') return { kind: 'colorlessPhyrexian' }
  if ('Generic' in symbol) return { kind: 'generic', count: symbol.Generic }
  if ('Colored' in symbol) return { kind: 'colored', color: symbol.Colored }
  if ('Hybrid' in symbol) return { kind: 'hybrid', colors: symbol.Hybrid }
  if ('MonoHybrid' in symbol) return { kind: 'monoHybrid', color: symbol.MonoHybrid }
  if ('Phyrexian' in symbol) return { kind: 'phyrexian', color: symbol.Phyrexian }
  if ('HybridPhyrexian' in symbol) return { kind: 'hybridPhyrexian', colors: symbol.HybridPhyrexian }
  return { kind: 'colorlessHybrid', color: symbol.ColorlessHybrid }
}

/** The symbol as a card prints it, without braces: "2", "G", "W/U", "2/W", "W/P", "G/U/P", "C/W", "C/P". */
export function symbolText(symbol: ManaSymbol): string {
  const shape = shapeOf(symbol)
  switch (shape.kind) {
    case 'generic':
      return String(shape.count)
    case 'colored':
      return colorLetter(shape.color)
    case 'colorless':
      return 'C'
    case 'hybrid':
      return `${colorLetter(shape.colors[0])}/${colorLetter(shape.colors[1])}`
    case 'monoHybrid':
      return `2/${colorLetter(shape.color)}`
    case 'phyrexian':
      return `${colorLetter(shape.color)}/P`
    case 'hybridPhyrexian':
      return `${colorLetter(shape.colors[0])}/${colorLetter(shape.colors[1])}/P`
    case 'colorlessHybrid':
      return `C/${colorLetter(shape.color)}`
    case 'colorlessPhyrexian':
      return 'C/P'
  }
}

/** What the symbol adds to a mana value (CR 202.3): its number for a generic symbol, 2 for `{2/W}`, else 1. */
export function symbolValue(symbol: ManaSymbol): number {
  const shape = shapeOf(symbol)
  if (shape.kind === 'generic') return shape.count
  return shape.kind === 'monoHybrid' ? 2 : 1
}

/** Every way to pay the symbol, the mana ones first and life last. */
export function waysOf(symbol: ManaSymbol): Way[] {
  const shape = shapeOf(symbol)
  const life: Way = { kind: 'life', amount: LIFE_PER_SYMBOL }
  const mana = (type: ManaType): Way => ({ kind: 'mana', mana: type })
  switch (shape.kind) {
    case 'generic':
      return [{ kind: 'generic', count: shape.count }]
    case 'colored':
      return [mana(shape.color)]
    case 'colorless':
      return [mana('Colorless')]
    case 'hybrid':
      return [mana(shape.colors[0]), mana(shape.colors[1])]
    case 'monoHybrid':
      return [mana(shape.color), { kind: 'generic', count: 2 }]
    case 'phyrexian':
      return [mana(shape.color), life]
    case 'hybridPhyrexian':
      return [mana(shape.colors[0]), mana(shape.colors[1]), life]
    case 'colorlessHybrid':
      return [mana(shape.color), mana('Colorless')]
    case 'colorlessPhyrexian':
      return [mana('Colorless'), life]
  }
}

/** The mana that pays the symbol, besides life and generic mana, in words: "white mana", "green or blue mana". */
export function manaWords(symbol: ManaSymbol): string {
  const kinds = waysOf(symbol).flatMap((way) => (way.kind === 'mana' ? [way.mana.toLowerCase()] : []))
  return `${kinds.join(' or ')} mana`
}

/** Whether 2 life can pay the symbol: a Phyrexian symbol of any kind. */
export function canPayLife(symbol: ManaSymbol): boolean {
  return waysOf(symbol).some((way) => way.kind === 'life')
}

/** Whether the symbol has exactly one way to pay it: generic, coloured or colourless. */
export function isPlain(symbol: ManaSymbol): boolean {
  return waysOf(symbol).length === 1
}

/** A cost's mana value. */
export function costValue(symbols: readonly ManaSymbol[]): number {
  return symbols.reduce((sum, symbol) => sum + symbolValue(symbol), 0)
}
