/** Reads the mana cost text of a card (`{2}{W/U}`) into the symbols the table draws, so a cost looks the same everywhere. */
import { costPips } from '@/features/table/cardModel'
import type { ManaPip } from '@/features/table/cardModel'
import type { Color, ManaSymbol } from '@/protocol/generated'

const COLOR_OF: Readonly<Record<string, Color>> = { W: 'White', U: 'Blue', B: 'Black', R: 'Red', G: 'Green' }
const colorOf = (letter: string | undefined): Color | undefined =>
  letter !== undefined && Object.hasOwn(COLOR_OF, letter) ? COLOR_OF[letter] : undefined

// The shapes of CR 107.4 that the engine's `ManaSymbol` has; anything else (X, snow) is drawn as a plain disc.
function symbolOf(text: string): ManaSymbol | null {
  if (/^\d{1,2}$/.test(text)) return { Generic: Number(text) }
  const [a, b, c, ...rest] = text.split('/')
  if (rest.length > 0) return null
  const first = colorOf(a)
  const second = colorOf(b)
  if (b === undefined) {
    if (a === 'C') return 'Colorless'
    return first === undefined ? null : { Colored: first }
  }
  if (c === undefined) {
    if (first !== undefined && second !== undefined) return { Hybrid: [first, second] }
    if (a === '2' && second !== undefined) return { MonoHybrid: second }
    if (first !== undefined && b === 'P') return { Phyrexian: first }
    if (a === 'C' && second !== undefined) return { ColorlessHybrid: second }
    return a === 'C' && b === 'P' ? 'ColorlessPhyrexian' : null
  }
  return first !== undefined && second !== undefined && c === 'P'
    ? { HybridPhyrexian: [first, second] }
    : null
}

const OTHER_NAME: Readonly<Record<string, string>> = { X: 'X', Y: 'Y', Z: 'Z', S: 'snow' }

function otherPip(text: string, index: number): ManaPip {
  const name = Object.hasOwn(OTHER_NAME, text) ? OTHER_NAME[text] : undefined
  return {
    key: `${index}-${text}`,
    symbol: text,
    tone: 'generic',
    text: text.length > 2 ? text.slice(0, 2) : text,
    name: name ?? text,
  }
}

/** The symbols of one cost, such as `{2}{U}`, in card order; text outside braces is ignored. */
export function pipsOf(cost: string): ManaPip[] {
  const pips: ManaPip[] = []
  for (const match of cost.matchAll(/\{([^{}]+)\}/g)) {
    const text = match[1] ?? ''
    const index = pips.length
    const symbol = symbolOf(text)
    const pip = symbol === null ? undefined : costPips([symbol])[0]
    pips.push(pip === undefined ? otherPip(text, index) : { ...pip, key: `${index}-${text}` })
  }
  return pips
}

/** The costs of a card's faces: `{1}{W} // {2}{U}` is two, and a face without a cost is left out. */
export const faceCosts = (cost: string): ManaPip[][] =>
  cost
    .split('//')
    .map((face) => pipsOf(face))
    .filter((pips) => pips.length > 0)
