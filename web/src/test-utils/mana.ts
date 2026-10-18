/** Short ways to write mana costs, pools and sources in tests of payments. */
import type { ManaCost, ManaOption, ManaPool, ManaType } from '@/protocol/generated'

/** The mana a letter stands for: W, U, B, R, G or C. */
export const LETTER: Record<string, ManaType> = {
  W: 'White',
  U: 'Blue',
  B: 'Black',
  R: 'Red',
  G: 'Green',
  C: 'Colorless',
}

/** The kinds of mana in the order of `ManaPool.counts`. */
const ORDER: readonly ManaType[] = ['White', 'Blue', 'Black', 'Red', 'Green', 'Colorless']

/** The mana a letter stands for; a test with a typo fails here. */
export function manaOfLetter(letter: string): ManaType {
  const mana = LETTER[letter]
  if (mana === undefined) throw new Error(`no mana is written ${letter}`)
  return mana
}

/** `{2}{G}` style text as a cost: digits are generic, W U B R G colours and C colourless; empty is free. */
export function cost(text: string): ManaCost {
  const made: ManaCost = { generic: 0, colorless: 0, colored: [0, 0, 0, 0, 0] }
  for (const symbol of text.match(/\{[^}]+\}/g) ?? []) {
    const inner = symbol.slice(1, -1)
    const colour = 'WUBRG'.indexOf(inner)
    if (/^\d+$/.test(inner)) made.generic += Number(inner)
    else if (inner === 'C') made.colorless += 1
    else if (colour >= 0) made.colored[colour] = (made.colored[colour] ?? 0) + 1
    else throw new Error(`bad mana symbol ${symbol}`)
  }
  return made
}

/** Floating mana from letters, such as `poolOf('WWG')`. */
export function poolOf(letters: string): ManaPool {
  const counts = ORDER.map((type) => [...letters].filter((letter) => manaOfLetter(letter) === type).length)
  return { counts: counts as ManaPool['counts'] }
}

/** The mana ability `ability` of permanent `object`, which makes the mana a letter stands for. */
export function source(object: number, letter: string, ability = 0): ManaOption {
  return { source: { object, ability }, produces: manaOfLetter(letter) }
}
