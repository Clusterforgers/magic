/** Pure: the token a Tokens row makes, a common one, one the seat's cards make or a custom creature, with the words of its preview and the action it sends. */
import type {
  CardType,
  Color,
  CommonToken,
  ManualKeyword,
  ManualToken,
  TokenOfferView,
} from '@/protocol/generated'
import { COMMON_TOKENS, GRANT_KEYWORDS } from '../offers'
import type { Preview } from './commands'

/** The choice that makes a creature of the seat's own: name, power and toughness. */
export const CUSTOM = 'custom'

/** The choice of the token at a place of the table's list of tokens the seat's cards make (`TableView.tokens`). */
export type CardTokenChoice = `card:${number}`

/** What the Tokens row's list offers: a common token, a token of the seat's cards or `custom`. */
export type TokenChoice = CommonToken | CardTokenChoice | typeof CUSTOM

/** The list value of the token at `at` of the seat's card tokens. */
export const cardChoice = (at: number): CardTokenChoice => `card:${at}`

/** Whether a choice names a token of the seat's cards. */
export const isCardChoice = (choice: TokenChoice): choice is CardTokenChoice => choice.startsWith('card:')

/** The token of the seat's cards a choice names, or undefined for any other choice or one the list no longer has. */
export function cardTokenOf(
  choice: TokenChoice,
  offers: readonly TokenOfferView[],
): TokenOfferView | undefined {
  if (!isCardChoice(choice)) return undefined
  return offers[Number(choice.slice('card:'.length))]
}

/** "Eldrazi Scion 1/1": a token of the seat's cards as its option and buttons name it. */
export function offerLabel(offer: TokenOfferView): string {
  const stats = offer.power != null && offer.toughness != null ? ` ${offer.power}/${offer.toughness}` : ''
  return `${offer.name}${stats}`
}

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
  /** The colors of a custom token; none is colorless. */
  colors: readonly Color[]
  /** Whether a custom creature is also an artifact or an enchantment. */
  artifact: boolean
  enchantment: boolean
  /** The subtypes typed for a custom token, apart by spaces or commas; empty takes the name when it is letters only. */
  subtypes: string
  /** The keyword abilities of a custom token, which it carries as a token a card makes does. */
  keywords: readonly ManualKeyword[]
}

/** The first token of the list, which a row opens with. */
export const FIRST_TOKEN: TokenChoice = COMMON_TOKENS[0] ?? 'Treasure'

/** The draft values a Tokens row opens with: the first common token, and a 1/1 for a custom one. */
export const START_TOKEN: TokenDraft = {
  token: FIRST_TOKEN,
  name: '',
  power: 1,
  toughness: 1,
  colors: [],
  artifact: false,
  enchantment: false,
  subtypes: '',
  keywords: [],
}

const plural = (n: number, one: string): string => `${n} ${one}${n === 1 ? '' : 's'}`

const whole = (value: number, high: number): boolean => Number.isInteger(value) && value >= 0 && value <= high

/** The most subtypes the protocol takes on a custom token. */
export const TOKEN_SUBTYPES_MAX = 6

const SUBTYPE = /^[\p{L}'-]{1,24}$/u

/** The colors a custom token can have, in WUBRG order. */
export const TOKEN_COLORS: readonly Color[] = ['White', 'Blue', 'Black', 'Red', 'Green']

/** The subtypes of a draft: the typed words, else the name when it is letters only (the protocol takes letters only). */
function subtypesOf(draft: TokenDraft, name: string): string[] | { problem: string } {
  const words = draft.subtypes.split(/[\s,]+/).filter((word) => word.length > 0)
  if (words.length === 0) return /^[\p{L} '-]{1,24}$/u.test(name) ? [name] : []
  if (words.length > TOKEN_SUBTYPES_MAX || !words.every((word) => SUBTYPE.test(word))) {
    return {
      problem: `Subtypes are up to ${TOKEN_SUBTYPES_MAX} words of letters, apart by spaces or commas.`,
    }
  }
  return [...new Set(words)]
}

/** " with flying and vigilance": the keywords of a custom token after its words, or nothing without one. */
function abilityWords(words: readonly string[]): string {
  const last = words.at(-1)
  if (last === undefined) return ''
  const rest = words.slice(0, -1)
  return ` with ${rest.length === 0 ? last : `${rest.join(', ')} and ${last}`}`
}

/** The custom creature of a draft, or what is wrong with it. */
function customOf(
  draft: TokenDraft,
): { token: ManualToken; label: string; kind: string; abilities: string } | { problem: string } {
  const name = draft.name.trim()
  if (name.length === 0 || name.length > TOKEN_NAME_MAX) {
    return { problem: `Name the token in 1 to ${TOKEN_NAME_MAX} characters.` }
  }
  if (!whole(draft.power, TOKEN_STAT_MAX) || !whole(draft.toughness, TOKEN_STAT_MAX)) {
    return { problem: `Power and toughness are whole numbers from 0 to ${TOKEN_STAT_MAX}.` }
  }
  const subtypes = subtypesOf(draft, name)
  if ('problem' in subtypes) return subtypes
  const colors = TOKEN_COLORS.filter((color) => draft.colors.includes(color))
  const types: CardType[] = []
  if (draft.artifact) types.push('Artifact')
  if (draft.enchantment) types.push('Enchantment')
  types.push('Creature')
  const keywords = GRANT_KEYWORDS.filter(([keyword]) => draft.keywords.includes(keyword))
  const kind = [...colors.map((color) => color.toLowerCase()), ...types.map((type) => type.toLowerCase())]
  return {
    label: `${name} ${draft.power}/${draft.toughness}`,
    kind: kind.join(' '),
    abilities: abilityWords(keywords.map(([, words]) => words.toLowerCase())),
    token: {
      Custom: {
        name,
        colors,
        types,
        subtypes,
        power: draft.power,
        toughness: draft.toughness,
        ...(keywords.length === 0 ? {} : { keywords: keywords.map(([keyword]) => keyword) }),
      },
    },
  }
}

/** The preview of making `amount` tokens of `draft`, in the seat's words, with the action it sends. */
export function tokenPreview(
  draft: TokenDraft,
  amount: number,
  offers: readonly TokenOfferView[] = [],
): Preview {
  if (isCardChoice(draft.token)) {
    const offer = cardTokenOf(draft.token, offers)
    if (offer === undefined) return { problem: 'Choose a token from the list.' }
    const label = offerLabel(offer)
    const kind = [
      ...(offer.colors ?? []).map((color) => color.toLowerCase()),
      ...offer.types.map((type) => type.toLowerCase()),
    ]
    const abilities =
      (offer.rules ?? []).length === 0
        ? ''
        : ` with its ${(offer.rules ?? []).length === 1 ? 'ability' : 'abilities'}`
    return {
      preview: `${plural(amount, `${label} ${kind.join(' ')} token`)}${abilities} ${amount === 1 ? 'enters' : 'enter'} under your control (from ${offer.card})`,
      apply: `Create ${plural(amount, `${label} token`)}`,
      action: { CreateToken: { token: { Card: { card: offer.card, index: offer.index } }, count: amount } },
    }
  }
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
    preview: `${plural(amount, `${custom.label} ${custom.kind} token`)}${custom.abilities} ${amount === 1 ? 'enters' : 'enter'} under your control`,
    apply: `Create ${plural(amount, `${custom.label} token`)}`,
    action: { CreateToken: { token: custom.token, count: amount } },
  }
}
