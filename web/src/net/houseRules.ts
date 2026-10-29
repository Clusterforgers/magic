/** Reads the `house_rules` of a game browser row or a table view: every rule that is there must be right, unknown rules are left out. */
import type { HouseRules } from '@/protocol/generated'
import { isCount, isFlag, isRecord, isText } from './check'

type Rules = Record<string, unknown>

const COUNTS = [
  'starting_life',
  'opening_hand_size',
  'free_mulligans',
  'mulligan_cap',
  'draw_step_cards',
  'land_plays_per_turn',
  'turn_limit',
  'winning_life',
  'poison_threshold',
  'commander_tax',
  'commanders_per_seat',
] as const

const FLAGS = ['mana_burn', 'open_hands', 'revealed_library_top', 'opening_hand_smoothing'] as const

const WORDS: Record<string, readonly string[]> = {
  skip_first_draw: ['always', 'never'],
  empty_library: ['lose', 'reshuffle_graveyard'],
  attack_range: ['any', 'neighbours', 'left'],
  mulligan_style: ['london', 'paris', 'vancouver'],
}

const isHandLimit = (value: unknown): boolean =>
  value === 'unlimited' || (isRecord(value) && isCount(value.cards))
const isDamageLimit = (value: unknown): boolean =>
  value === 'off' || (isRecord(value) && isCount(value.lethal))

const optional = (value: unknown, check: (item: unknown) => boolean): boolean =>
  value === undefined || check(value)

const isDeckRules = (value: unknown): boolean =>
  isRecord(value) &&
  optional(value.singleton, isFlag) &&
  optional(value.colour_identity, isFlag) &&
  optional(value.deck_size, isCount) &&
  optional(value.max_copies, isCount)

const isHandicap = (value: unknown): boolean =>
  isRecord(value) &&
  isCount(value.seat) &&
  optional(value.starting_life, isCount) &&
  optional(value.opening_hand_size, isCount)

/** The rules a body holds, `undefined` for none, or null when a rule is there but wrong. */
export function readHouseRules(body: unknown): HouseRules | undefined | null {
  if (body === undefined) return undefined
  if (!isRecord(body)) return null
  const given: Rules = body
  const found: Rules = {}
  for (const key of COUNTS) {
    if (given[key] === undefined) continue
    if (!isCount(given[key])) return null
    found[key] = given[key]
  }
  for (const key of FLAGS) {
    if (given[key] === undefined) continue
    if (!isFlag(given[key])) return null
    found[key] = given[key]
  }
  for (const [key, words] of Object.entries(WORDS)) {
    const value = given[key]
    if (value === undefined) continue
    if (!isText(value) || !words.includes(value)) return null
    found[key] = value
  }
  const shaped: [string, (value: unknown) => boolean][] = [
    ['max_hand_size', isHandLimit],
    ['commander_damage', isDamageLimit],
    ['deck', isDeckRules],
    ['handicaps', (value) => Array.isArray(value) && value.every(isHandicap)],
  ]
  for (const [key, check] of shaped) {
    if (given[key] === undefined) continue
    if (!check(given[key])) return null
    found[key] = given[key]
  }
  // Every key was checked against the type of `HouseRules` above.
  return found as HouseRules
}
