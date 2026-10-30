/** Short ways to write mana costs, pools and sources in tests of payments. */
import type { Color, ManaCost, ManaOption, ManaPool, ManaSymbol, ManaType } from '@/protocol/generated'

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

const COLOR: Record<string, Color> = { W: 'White', U: 'Blue', B: 'Black', R: 'Red', G: 'Green' }
const LETTER_OF: Record<Color, string> = { White: 'W', Blue: 'U', Black: 'B', Red: 'R', Green: 'G' }
const COLORS: readonly Color[] = ['White', 'Blue', 'Black', 'Red', 'Green']
const colorOf = (letter: string | undefined): Color => {
  const color = letter === undefined ? undefined : COLOR[letter]
  if (color === undefined) throw new Error(`no color is written ${String(letter)}`)
  return color
}

/** The ten colour pairs as cards print them, so a pair written either way round is the same symbol. */
const PAIRS = ['WU', 'UB', 'BR', 'RG', 'GW', 'WB', 'UR', 'BG', 'RW', 'GU']
const pairOf = (a: string, b: string): [Color, Color] => {
  const printed = PAIRS.find((pair) => a !== b && (pair === a + b || pair === b + a))
  if (printed === undefined) throw new Error(`no hybrid symbol of ${a} and ${b}`)
  return [colorOf(printed[0]), colorOf(printed[1])]
}
const pairRank = ([a, b]: [Color, Color]): number => PAIRS.indexOf(LETTER_OF[a] + LETTER_OF[b])

/** One symbol from the text inside its braces: `2`, `G`, `C`, `W/U`, `2/W`, `W/P`, `G/U/P`, `C/W` or `C/P`. */
export function symbol(inner: string): ManaSymbol {
  const parts = inner.split('/')
  const [a, b, c] = parts
  if (parts.length === 1) {
    if (/^\d+$/.test(inner)) return { Generic: Number(inner) }
    return inner === 'C' ? 'Colorless' : { Colored: colorOf(inner) }
  }
  if (parts.length === 2 && a !== undefined && b !== undefined) {
    if (a === '2') return { MonoHybrid: colorOf(b) }
    if (a === 'C' && b === 'P') return 'ColorlessPhyrexian'
    if (a === 'C') return { ColorlessHybrid: colorOf(b) }
    if (b === 'P') return { Phyrexian: colorOf(a) }
    return { Hybrid: pairOf(a, b) }
  }
  if (parts.length === 3 && a !== undefined && b !== undefined && c === 'P') {
    return { HybridPhyrexian: pairOf(a, b) }
  }
  throw new Error(`bad mana symbol {${inner}}`)
}

/** Where a symbol sits in the order the server sends: generic, colourless, colours, then the symbols with a choice. */
function rank(value: ManaSymbol): [number, number] {
  if (value === 'Colorless') return [1, 0]
  if (value === 'ColorlessPhyrexian') return [7, 0]
  if ('Generic' in value) return [0, 0]
  if ('Colored' in value) return [2, COLORS.indexOf(value.Colored)]
  if ('ColorlessHybrid' in value) return [3, COLORS.indexOf(value.ColorlessHybrid)]
  if ('Hybrid' in value) return [4, pairRank(value.Hybrid)]
  if ('MonoHybrid' in value) return [5, COLORS.indexOf(value.MonoHybrid)]
  if ('Phyrexian' in value) return [6, COLORS.indexOf(value.Phyrexian)]
  return [8, pairRank(value.HybridPhyrexian)]
}

const isGeneric = (value: ManaSymbol): value is { Generic: number } =>
  typeof value === 'object' && 'Generic' in value

/**
 * `{2}{G}` style text as a cost, in the order the server sends it: digits are generic (and add up), W U B R G
 * colours, C colourless, and the hybrid and Phyrexian forms are read as cards write them; empty is free.
 */
export function cost(text: string): ManaCost {
  const symbols = (text.match(/\{[^}]+\}/g) ?? []).map((written) => symbol(written.slice(1, -1)))
  const generic = symbols.reduce((sum, one) => sum + (isGeneric(one) ? one.Generic : 0), 0)
  const rest = symbols
    .filter((one) => !isGeneric(one))
    .map((one, index) => ({ one, index, at: rank(one) }))
    .sort((x, y) => x.at[0] - y.at[0] || x.at[1] - y.at[1] || x.index - y.index)
    .map(({ one }) => one)
  return generic > 0 ? [{ Generic: generic }, ...rest] : rest
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
