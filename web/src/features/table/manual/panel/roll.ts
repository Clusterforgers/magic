/** Pure: the Roll row of the Commands panel, the choices it offers (a coin, a d6, a d20, a die of any size), its preview and the action it sends. */
import type { ManualRoll } from '@/protocol/generated'
import type { AmountSpec, Preview } from './commands'

/** What the row rolls: a coin, one of the common dice, or a die of the size in the field. */
export type RollPick = 'coin' | 'd6' | 'd20' | 'custom'

/** The choices in the order the row lists them. */
export const ROLL_PICKS: readonly { pick: RollPick; label: string }[] = [
  { pick: 'coin', label: 'Coin' },
  { pick: 'd6', label: 'd6' },
  { pick: 'd20', label: 'd20' },
  { pick: 'custom', label: 'd N' },
]

/** The size of a die of any size: 2 to 100 faces, as the engine takes it. */
export const SIDES: AmountSpec = { signed: false, min: 2, max: 100, presets: [], start: 10 }

/** The roll a choice names, or null while the size in the field is no whole number from 2 to 100. */
export function rollKind(pick: RollPick, sides: number): ManualRoll | null {
  switch (pick) {
    case 'coin':
      return { Coin: {} }
    case 'd6':
      return { Die: { sides: 6 } }
    case 'd20':
      return { Die: { sides: 20 } }
    case 'custom':
      return Number.isInteger(sides) && sides >= SIDES.min && sides <= SIDES.max
        ? { Die: { sides } }
        : null
  }
}

/** "a coin" or "a d20": what the roll is, for the sentences of the panel and the log. */
export function rollNoun(kind: ManualRoll): string {
  return 'Coin' in kind ? 'a coin' : `a d${kind.Die.sides}`
}

/** "heads" or "tails" for a coin (1 and 2), the number for a die. */
export function rollFace(kind: ManualRoll, result: number): string {
  return 'Coin' in kind ? (result === 1 ? 'heads' : 'tails') : `${result}`
}

/** What a seat did and what came up, as the log tells it: "Ana rolls a d20: 14", "Ana flips a coin: heads". */
export function rollSentence(who: string, kind: ManualRoll, result?: number): string {
  const verb = 'Coin' in kind ? 'flips' : 'rolls'
  const told = result === undefined ? '' : `: ${rollFace(kind, result)}`
  return `${who} ${verb} ${rollNoun(kind)}${told}`
}

/** What applying the row with `pick` and the size in the field does, in the seat's words, and the action it sends. */
export function rollPreview(pick: RollPick, sides: number): Preview {
  const kind = rollKind(pick, sides)
  if (kind === null) return { problem: `Choose a die of ${SIDES.min} to ${SIDES.max} faces.` }
  const coin = 'Coin' in kind
  return {
    preview: `${coin ? 'Flips' : 'Rolls'} ${rollNoun(kind)}; everyone sees the result`,
    apply: `${coin ? 'Flip' : 'Roll'} ${rollNoun(kind)}`,
    action: { Roll: { kind } },
  }
}
