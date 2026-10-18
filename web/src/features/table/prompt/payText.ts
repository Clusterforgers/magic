/** The PayMana prompt in plain words: what a cost asks for and what to do next. */
import type { ManaCost, ManaType } from '@/protocol/generated'
import { manaValue } from './payment'

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

/** "a", "a and b", "a, b and c". */
function andList(items: readonly string[]): string {
  const last = items.at(-1)
  if (last === undefined) return ''
  return items.length === 1 ? last : `${items.slice(0, -1).join(', ')} and ${last}`
}

/** A cost in words, such as "2 generic, 1 white and 1 green"; "nothing" for an empty cost. */
export function costWords(cost: ManaCost): string {
  const [white, blue, black, red, green] = cost.colored
  const parts: [number, string][] = [
    [cost.generic, 'generic'],
    [cost.colorless, 'colorless'],
    [white, 'white'],
    [blue, 'blue'],
    [black, 'black'],
    [red, 'red'],
    [green, 'green'],
  ]
  const said = parts.flatMap(([count, what]) => (count > 0 ? [`${count} ${what}`] : []))
  return said.length === 0 ? 'nothing' : andList(said)
}

/** What `payText` needs to know. */
export interface PayTextInput {
  spell: string
  cost: ManaCost
  owed: ManaCost
  /** How many pieces are in the payment so far. */
  chosen: number
  /** True when the payment is one the engine accepts. */
  covered: boolean
  /** True when something may still be added to the payment. */
  canAdd: boolean
  /** True when the game offers to pay by itself. */
  auto: boolean
  /** What the last click did, as a sentence; said before the status. */
  note: string
}

/** The one live sentence of the prompt: the last change, then what is owed and what to do. */
export function payText({ spell, cost, owed, chosen, covered, canAdd, auto, note }: PayTextInput): string {
  const lead = note === '' ? '' : `${note} `
  if (covered) {
    return manaValue(cost) === 0
      ? `${spell} costs nothing. Confirm to cast it.`
      : `${lead}The cost is covered. Confirm to cast ${spell}.`
  }
  const still = `Still to pay: ${costWords(owed)}.`
  if (chosen === 0) {
    if (!auto && !canAdd) {
      return `You cannot pay ${costWords(cost)} for ${spell} with the mana you have. Cancel to go back.`
    }
    const pick = 'Choose a land or other mana source to use its mana'
    const how = auto ? `${pick}, or pay automatically.` : `${pick}.`
    return `${lead}Pay ${costWords(cost)} for ${spell}. ${how}`
  }
  return canAdd ? `${lead}${still}` : `${lead}${still} Nothing left can pay it; remove a choice or cancel.`
}
