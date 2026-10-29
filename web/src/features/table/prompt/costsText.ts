/**
 * The optional costs of a spell in plain words, as the ChooseCosts prompt shows them: each cost as the card prints it
 * ("Kicker {2}{R}"), the extra cost of a choice added up, and the rule that refuses a combination. A gift is offered
 * once per opponent as `SEAT_BASE + seat`, and the engine accepts one gift at most.
 */
import type { Color, ManaCost, ManaSymbol, OptionalCost } from '@/protocol/generated'
import { symbolText } from '../manaSymbols'

/** The first number among the costs that names a seat: a gift promised to an opponent (`gift::SEAT_BASE` in the engine). */
export const SEAT_BASE = 1 << 16

/** Whether the cost is a gift promised to an opponent. */
export const isGift = (cost: OptionalCost): boolean => cost.index >= SEAT_BASE

/** The keys that toggle the first ten costs, for a screen reader: "Press 1 to 3 to take a cost or leave it." */
export function costKeysWords(count: number): string {
  if (count <= 1) return 'Press 1 to take the cost or leave it.'
  if (count <= 9) return `Press 1 to ${count} to take a cost or leave it.`
  return 'Press 1 to 9, or 0, to take one of the first ten costs or leave it.'
}

/** The live sentence: who asks, and what a player with no wish to pay more does. */
export function costsSentence(asker: string, chosen: number): string {
  if (chosen === 0) return `${asker}: choose the optional costs to pay, or none.`
  return `${asker}: ${chosen === 1 ? '1 optional cost' : `${chosen} optional costs`} chosen. Confirm, or change the choice.`
}

const COLOR: Record<string, Color> = {
  W: 'White',
  U: 'Blue',
  B: 'Black',
  R: 'Red',
  G: 'Green',
}
const COLORS: readonly Color[] = ['White', 'Blue', 'Black', 'Red', 'Green']

/** The mana symbol written inside braces (`2`, `G`, `C`), or null for one the prompt does not draw (hybrid, X). */
export function parseSymbol(inner: string): ManaSymbol | null {
  if (/^\d+$/.test(inner)) return { Generic: Number(inner) }
  if (inner === 'C') return 'Colorless'
  const color = COLOR[inner]
  return color === undefined ? null : { Colored: color }
}

/** A piece of a cost as the card prints it: plain words, or one mana symbol (`inner` is the text inside the braces). */
export type LabelPiece = { text: string } | { symbol: ManaSymbol; inner: string }

/** The label cut into words and mana symbols, in order; a symbol the prompt cannot draw stays words with its braces. */
export function labelPieces(label: string): LabelPiece[] {
  const pieces: LabelPiece[] = []
  let words = ''
  for (const part of label.split(/(\{[^}]*\})/)) {
    const inner = /^\{([^}]*)\}$/.exec(part)?.[1]
    const symbol = inner === undefined ? null : parseSymbol(inner)
    if (inner !== undefined && symbol !== null) {
      if (words !== '') pieces.push({ text: words })
      words = ''
      pieces.push({ symbol, inner })
    } else {
      words += part
    }
  }
  if (words !== '') pieces.push({ text: words })
  return pieces
}

/** What the chosen costs add to the cost of the spell: their mana added up, and the costs that are not mana. */
export interface ExtraCost {
  mana: ManaCost
  /** The parts that are not mana ("Gift to player 2", "Discard a card"), as the card prints them. */
  others: string[]
}

/**
 * The extra cost of the costs `labels`. A label with mana symbols adds them (generic mana summed, each colour counted)
 * and any part after its first comma is another cost; a label with no symbol is one cost as a whole.
 */
export function extraCost(labels: readonly string[]): ExtraCost {
  let generic = 0
  let colorless = 0
  const colored = new Map<Color, number>()
  const others: string[] = []
  for (const label of labels) {
    const pieces = labelPieces(label)
    const symbols = pieces.flatMap((piece) => ('symbol' in piece ? [piece.symbol] : []))
    if (symbols.length === 0) {
      others.push(label)
      continue
    }
    for (const symbol of symbols) {
      if (symbol === 'Colorless') colorless += 1
      else if (typeof symbol === 'string') continue
      else if ('Generic' in symbol) generic += symbol.Generic
      else if ('Colored' in symbol) colored.set(symbol.Colored, (colored.get(symbol.Colored) ?? 0) + 1)
    }
    const [, ...parts] = label.replace(/\{[^}]*\}/g, '').split(',')
    others.push(...parts.map((part) => part.trim()).filter((part) => part !== ''))
  }
  const mana: ManaCost = [
    ...(generic > 0 ? [{ Generic: generic } satisfies ManaSymbol] : []),
    ...COLORS.flatMap((color) =>
      Array.from({ length: colored.get(color) ?? 0 }, (): ManaSymbol => ({ Colored: color })),
    ),
    ...Array.from({ length: colorless }, (): ManaSymbol => 'Colorless'),
  ]
  return { mana, others }
}

/** The extra cost in words for a screen reader and the line under the toggles: "{2}{R}" and "Discard a card". */
export function extraCostWords({ mana, others }: ExtraCost): string {
  const symbols = mana.map((symbol) => `{${symbolText(symbol)}}`).join('')
  return [symbols, ...others].filter((part) => part !== '').join(' and ') || 'nothing'
}

/**
 * Why toggling `cost` on is refused with `chosen` taken, or undefined when it is allowed. The engine accepts one gift:
 * a second is refused until the first is taken back.
 */
export function refusal(
  cost: OptionalCost,
  chosen: readonly number[],
  optional: readonly OptionalCost[],
): string | undefined {
  if (chosen.includes(cost.index) || !isGift(cost)) return undefined
  const held = optional.find((other) => isGift(other) && chosen.includes(other.index))
  return held === undefined
    ? undefined
    : `Only one gift can be promised: take back "${held.label}" to choose "${cost.label}".`
}

/** The indices of the chosen costs, in ascending order, as the answer lists them. */
export function costsAnswer(chosen: readonly number[]): number[] {
  return chosen.toSorted((a, b) => a - b)
}
