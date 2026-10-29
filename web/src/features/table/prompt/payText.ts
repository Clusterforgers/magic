/** The PayMana prompt in plain words: what a cost asks for and what to do next. */
import type { Color, ManaCost, ManaSymbol, ManaType, ObjectId } from '@/protocol/generated'
import { LIFE_PER_SYMBOL, shapeOf, symbolText } from '../manaSymbols'
import { madeOf, manaValue } from './payment'
import type { Spend } from './payment'

const NAME: Record<ManaType, string> = {
  White: 'white',
  Blue: 'blue',
  Black: 'black',
  Red: 'red',
  Green: 'green',
  Colorless: 'colorless',
}

/** The lower-case word for a kind of mana: "white", "colorless". */
export function manaName(mana: ManaType): string {
  return NAME[mana]
}

/** What a piece of the payment is, in words: "green from Forest", "red from your pool". */
export function spendWords(spend: Spend, nameOf: (id: ObjectId) => string): string {
  const made = madeOf(spend)
  const [first] = made
  const alike = first !== undefined && made.every((mana) => mana === first)
  const what = alike && made.length > 1 ? `${made.length} ${manaName(first)}` : andList(made.map(manaName))
  return spend.kind === 'pool'
    ? `${what} from your pool`
    : `${what} from ${nameOf(spend.option.source.object)}`
}

/** "a", "a and b", "a, b and c". */
function andList(items: readonly string[]): string {
  const last = items.at(-1)
  if (last === undefined) return ''
  return items.length === 1 ? last : `${items.slice(0, -1).join(', ')} and ${last}`
}

const colorName = (color: Color): string => manaName(color)

/** What one symbol asks for, in words: 'green', 'white or blue', 'white or 2 life', 'white or 2 generic'. */
function symbolWords(symbol: ManaSymbol): string {
  const shape = shapeOf(symbol)
  const life = `${LIFE_PER_SYMBOL} life`
  switch (shape.kind) {
    case 'generic':
      return 'generic'
    case 'colored':
      return colorName(shape.color)
    case 'colorless':
      return 'colorless'
    case 'hybrid':
      return `${colorName(shape.colors[0])} or ${colorName(shape.colors[1])}`
    case 'monoHybrid':
      return `${colorName(shape.color)} or 2 generic`
    case 'phyrexian':
      return `${colorName(shape.color)} or ${life}`
    case 'hybridPhyrexian':
      return `${colorName(shape.colors[0])} or ${colorName(shape.colors[1])} or ${life}`
    case 'colorlessHybrid':
      return `colorless or ${colorName(shape.color)}`
    case 'colorlessPhyrexian':
      return `colorless or ${life}`
  }
}

/** A cost in words, such as '2 generic, 1 white and 1 green' or '1 white or 2 life'; 'nothing' for an empty cost. */
export function costWords(cost: ManaCost): string {
  const runs: { symbol: ManaSymbol; count: number }[] = []
  for (const symbol of cost) {
    const last = runs.at(-1)
    const generic = shapeOf(symbol)
    if (last !== undefined && symbolText(last.symbol) === symbolText(symbol) && generic.kind !== 'generic') {
      last.count += 1
    } else if (generic.kind === 'generic' && last !== undefined && shapeOf(last.symbol).kind === 'generic') {
      last.count += generic.count
    } else {
      runs.push({ symbol, count: generic.kind === 'generic' ? generic.count : 1 })
    }
  }
  const said = runs.map(({ symbol, count }) => `${count} ${symbolWords(symbol)}`)
  return said.length === 0 ? 'nothing' : andList(said)
}

/** The life side of a payment, for the sentence. */
export interface LifeText {
  /** Life the payment pays so far. */
  paid: number
  /** The life total after paying it. */
  after: number
  /** True when the player may still pay 2 life for a Phyrexian symbol. */
  may: boolean
}

/** What `payText` needs to know. */
export interface PayTextInput {
  spell: string
  cost: ManaCost
  owed: ManaCost
  /** How many pieces are in the payment so far, life included. */
  chosen: number
  /** True when the payment is one the engine accepts. */
  covered: boolean
  /** True when something may still be added to the payment. */
  canAdd: boolean
  /** True when the game offers to pay by itself. */
  auto: boolean
  /** True when the cast may be backed out of. */
  cancel: boolean
  /** What the last click did, as a sentence; said before the status. */
  note: string
  /** Life paid and payable; left out for a cost with no Phyrexian symbol. */
  life?: LifeText
}

/** The one live sentence of the prompt: the last change, then what is owed and what to do. */
export function payText({
  spell,
  cost,
  owed,
  chosen,
  covered,
  canAdd,
  auto,
  cancel,
  note,
  life,
}: PayTextInput): string {
  const lead = note === '' ? '' : `${note} `
  const may = life?.may === true
  const paying = life !== undefined && life.paid > 0 ? `, paying ${life.paid} life (${life.after} left)` : ''
  if (covered) {
    return manaValue(cost) === 0
      ? `${spell} costs nothing. Confirm to cast it.`
      : `${lead}The cost is covered${paying}. Confirm to cast ${spell}.`
  }
  const still = `Still to pay: ${costWords(owed)}.`
  const lifeChoice = may ? `, pay ${LIFE_PER_SYMBOL} life for a Phyrexian symbol` : ''
  if (chosen === 0) {
    // The game offers an automatic payment whenever the cost can be paid at all, unless that takes the last life.
    if (!canAdd || (!auto && !may)) {
      return `You cannot pay ${costWords(cost)} for ${spell} with the mana you have.${cancel ? ' Cancel to go back.' : ''}`
    }
    const automatic = auto ? ', or pay automatically' : ''
    return `${lead}Pay ${costWords(cost)} for ${spell}. Choose a land or other mana source to use its mana${lifeChoice}${automatic}.`
  }
  if (!canAdd) {
    return `${lead}${still} Nothing left can pay it; remove a choice${cancel ? ' or cancel' : ''}.`
  }
  return auto ? `${lead}${still} Choose more${lifeChoice}, or pay the rest for me.` : `${lead}${still}`
}
