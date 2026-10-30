/** Pure: the token a Tokens row makes, a common one or a custom creature, with the words of its preview and the action it sends. */
import type { CommonToken, ManualToken } from '@/protocol/generated'
import { COMMON_TOKENS } from '../offers'
import type { Preview } from './commands'

/** The choice that makes a creature of the seat's own: name, power and toughness. */
export const CUSTOM = 'custom'

/** What the Tokens row's list offers: a common token or `custom`. */
export type TokenChoice = CommonToken | typeof CUSTOM

/** The longest name of a custom token the protocol takes. */
export const TOKEN_NAME_MAX = 40

/** The greatest power or toughness of a custom token. */
export const TOKEN_STAT_MAX = 99

/** What a Tokens row has chosen so far. */
export interface TokenDraft {
  token: TokenChoice
  /** The name of a custom token. */
  name: string
  power: number
  toughness: number
}

/** The first token of the list, which a row opens with. */
export const FIRST_TOKEN: TokenChoice = COMMON_TOKENS[0] ?? 'Treasure'

/** The draft values a Tokens row opens with: the first common token, and a 1/1 for a custom one. */
export const START_TOKEN: TokenDraft = { token: FIRST_TOKEN, name: '', power: 1, toughness: 1 }

const plural = (n: number, one: string): string => `${n} ${one}${n === 1 ? '' : 's'}`

const whole = (value: number, high: number): boolean => Number.isInteger(value) && value >= 0 && value <= high

/** The custom creature of a draft, or what is wrong with it. */
function customOf(draft: TokenDraft): { token: ManualToken; label: string } | { problem: string } {
  const name = draft.name.trim()
  if (name.length === 0 || name.length > TOKEN_NAME_MAX) {
    return { problem: `Name the token in 1 to ${TOKEN_NAME_MAX} characters.` }
  }
  if (!whole(draft.power, TOKEN_STAT_MAX) || !whole(draft.toughness, TOKEN_STAT_MAX)) {
    return { problem: `Power and toughness are whole numbers from 0 to ${TOKEN_STAT_MAX}.` }
  }
  return {
    label: `${name} ${draft.power}/${draft.toughness}`,
    token: {
      Custom: {
        name,
        colors: [],
        types: ['Creature'],
        // The protocol takes letters only as a subtype, so a name with digits is a token with none.
        subtypes: /^[\p{L} '-]{1,24}$/u.test(name) ? [name] : [],
        power: draft.power,
        toughness: draft.toughness,
      },
    },
  }
}

/** The preview of making `amount` tokens of `draft`, in the seat's words, with the action it sends. */
export function tokenPreview(draft: TokenDraft, amount: number): Preview {
  if (draft.token !== CUSTOM) {
    return {
      preview: `${plural(amount, `${draft.token} token`)} ${amount === 1 ? 'enters' : 'enter'} under your control`,
      apply: `Create ${plural(amount, draft.token)}`,
      action: { CreateToken: { token: { Common: draft.token }, count: amount } },
    }
  }
  const custom = customOf(draft)
  if ('problem' in custom) return custom
  return {
    preview: `${plural(amount, `${custom.label} creature token`)} ${amount === 1 ? 'enters' : 'enter'} under your control`,
    apply: `Create ${plural(amount, `${custom.label} token`)}`,
    action: { CreateToken: { token: custom.token, count: amount } },
  }
}
