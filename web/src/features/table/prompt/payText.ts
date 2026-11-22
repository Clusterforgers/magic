/** The PayMana prompt in plain words: what a cost asks for and what to do next. */
import type {
  AskedWhy,
  Color,
  ManaCost,
  ManaOption,
  ManaSymbol,
  ManaType,
  ObjectId,
} from '@/protocol/generated'
import { LIFE_PER_SYMBOL, manaWords, shapeOf, symbolText } from '../manaSymbols'
import { madeOf, manaValue } from './payment'
import type { Spend, Whole } from './payment'

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
  if (spend.kind === 'convoke') {
    return `${spend.pays === null ? '1 generic' : `1 ${manaName(spend.pays)}`} by tapping ${nameOf(spend.creature)} to convoke`
  }
  if (spend.kind === 'improvise') return `1 generic by tapping ${nameOf(spend.artifact)} to improvise`
  if (spend.kind === 'delve') return `1 generic by exiling ${nameOf(spend.card)} to delve`
  const made = madeOf(spend)
  const [first] = made
  const alike = first !== undefined && made.every((mana) => mana === first)
  const what = alike && made.length > 1 ? `${made.length} ${manaName(first)}` : andList(made.map(manaName))
  if (spend.kind === 'pool') return `${what} from your pool`
  if (spend.kind === 'restricted') return `restricted ${what} from your pool`
  return `${what} from ${nameOf(spend.option.source.object)}`
}

/** "a", "a and b", "a, b and c". */
export function andList(items: readonly string[]): string {
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
    case 'snow':
      return 'snow'
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
  /** The ability being activated, when the payment is for one: its keyword ("Plainscycling"), or "an ability" for one no keyword adds. */
  ability?: string | undefined
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
  /** True when the payment is the tax on an attack being declared; the words say 'attack' and "go back". */
  attack?: boolean
  /** True when the payment is the tax on a block being declared; the words say 'block' and "go back". */
  block?: boolean
  /** What the last click did, as a sentence; said before the status. */
  note: string
  /** Life paid and payable; left out for a cost with no Phyrexian symbol. */
  life?: LifeText
}

/** The one live sentence of the prompt: the last change, then what is owed and what to do. */
export function payText({
  spell,
  ability,
  cost,
  owed,
  chosen,
  covered,
  canAdd,
  auto,
  cancel,
  attack = false,
  block = false,
  note,
  life,
}: PayTextInput): string {
  const lead = note === '' ? '' : `${note} `
  // What the payment is for and what Confirm does: "Plainscycling of Angel of the Ruins", "activate Plainscycling of …".
  const paid = ability === undefined ? spell : `${ability} of ${spell}`
  const doing = ability === undefined ? `cast ${spell}` : `activate ${paid}`
  const declaring = attack ? 'attack' : block ? 'block' : undefined
  const may = life?.may === true
  const paying = life !== undefined && life.paid > 0 ? `, paying ${life.paid} life (${life.after} left)` : ''
  if (covered) {
    return manaValue(cost) === 0
      ? `${paid} costs nothing. Confirm to ${ability === undefined ? 'cast it' : 'activate it'}.`
      : `${lead}The cost is covered${paying}. Confirm to ${declaring ?? doing}.`
  }
  const still = `Still to pay: ${costWords(owed)}.`
  const lifeChoice = may ? `, pay ${LIFE_PER_SYMBOL} life for a Phyrexian symbol` : ''
  if (chosen === 0) {
    // The game offers an automatic payment whenever the cost can be paid at all, unless that takes the last life.
    if (!canAdd || (!auto && !may)) {
      const back =
        declaring !== undefined
          ? ` Add the mana by hand in the Commands panel, or go back to choose other ${declaring}ers.`
          : ' Undo to go back.'
      return `You cannot pay ${costWords(cost)} for ${declaring !== undefined ? `the ${declaring}` : paid} with the mana you have.${cancel ? back : ''}`
    }
    const automatic = auto ? ', or pay automatically' : ''
    return `${lead}Pay ${costWords(cost)} for ${declaring !== undefined ? `the ${declaring}` : paid}. Choose a land or other mana source to use its mana${lifeChoice}${automatic}.`
  }
  if (!canAdd) {
    return `${lead}${still} Nothing left can pay it; remove a choice${cancel ? ' or cancel' : ''}.`
  }
  return auto ? `${lead}${still} Choose more${lifeChoice}, or pay the rest for me.` : `${lead}${still}`
}

/** The note of a proposed payment ("Pay for me" without "Always pay for me"): what it uses, then what to do. */
export function proposalNote(whole: Whole, nameOf: (id: ObjectId) => string): string {
  const parts = whole.spends.map((spend) => spendWords(spend, nameOf))
  if (whole.life.length > 0) parts.push(`${whole.life.length * LIFE_PER_SYMBOL} life`)
  return `The game proposes ${parts.length === 0 ? 'nothing' : andList(parts)}. Confirm to pay, or change it.`
}

/** The letter of each kind of mana, as a cost writes it. */
const LETTER: Record<ManaType, string> = {
  White: 'W',
  Blue: 'U',
  Black: 'B',
  Red: 'R',
  Green: 'G',
  Colorless: 'C',
}

/** The badge of a source a proposed payment taps: "pays U", "pays GG" for two. */
export function paysWords(option: ManaOption): string {
  return `pays ${LETTER[option.produces].repeat(Math.max(1, option.count ?? 1))}`
}

/** The names of `ids` joined in words: "Treasure", "Treasure and Gold". */
function namesOf(ids: readonly ObjectId[], nameOf: (id: ObjectId) => string): string {
  const names = ids.map(nameOf)
  const last = names.pop() ?? ''
  return names.length === 0 ? last : `${names.join(', ')} and ${last}`
}

/** Why "Always pay for me" with "Even with life, sacrifices and other costs" asked: only the last life is never paid. */
export const LAST_LIFE_ASKED = 'Asked: paying it would take your last life.'

/** Why "Always pay for me" asked about this payment, in one short line; empty when no automatic payment exists at all. */
export function askedText(why: AskedWhy, nameOf: (id: ObjectId) => string): string {
  if (why === 'NoPayment') return ''
  if (why === 'Convoke' || why === 'Improvise' || why === 'Delve') {
    return `Asked: the rest needs ${why.toLowerCase()}.`
  }
  if ('Life' in why) return `Asked: paying {${symbolText(why.Life)}} needs life or ${manaWords(why.Life)}.`
  const [ids, words] =
    'GivesUp' in why
      ? [why.GivesUp, 'sacrificed or exiled']
      : 'PaysLife' in why
        ? [why.PaysLife, 'pays life']
        : 'Energy' in why
          ? [why.Energy, 'pays energy']
          : 'PaidAbility' in why
            ? [why.PaidAbility, 'costs mana of its own']
            : 'Takes' in why
              ? [why.Takes, 'takes other permanents']
              : [why.DoesMore, 'does more than add mana']
  return `Asked: only ${namesOf(ids, nameOf)} can pay the rest (${words}).`
}
