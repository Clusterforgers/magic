/**
 * The house rules a game's creator can set, as data: one spec per rule with its range, the format's own value and the
 * words for naming it. The create form edits them as field text, the lobby and the table name those that differ from
 * the format's values, and the request carries only what differs. The ranges repeat `HouseRules::validate`
 * (docs/engine.md, House rules); the server is the judge.
 */
import type {
  AttackRange,
  EmptyLibrary,
  HouseRules,
  MulliganStyle,
  SkipFirstDraw,
} from '@/protocol/generated'

/** The formats a game is made in. */
export type GameFormat = 'normal' | 'commander'

/** Where a rule sits in the form. */
export type RuleGroup = 'Life and cards' | 'Mulligans' | 'Turns and winning' | 'Table' | 'Commander'

/** The control a rule is edited with. */
export type RuleInput =
  | { kind: 'number'; min: number; max: number; word?: string }
  | { kind: 'choice'; options: readonly { value: string; label: string }[] }

/** The field text of every rule being edited, by `RuleSpec.id`; an empty or missing text keeps the format's value. */
export type RuleTexts = Readonly<Record<string, string>>

/** One rule: how it is edited, parsed and named. */
export interface RuleSpec {
  /** The key of the rule's field text, and the suffix of its DOM id. */
  id: string
  label: string
  /** One line under the field. */
  help: string
  group: RuleGroup
  /** The formats whose games the rule applies to. */
  formats: readonly GameFormat[]
  input: RuleInput
  /** Puts the field text into `rules`; returns the problem in words, or null. */
  apply(rules: HouseRules, text: string): string | null
  /** The words for the value `rules` holds, when it is set and differs from the format's own; else null. */
  phrase(rules: HouseRules, format: GameFormat): string | null
  /** The format's own value as field text, for the placeholder; empty when the format sets none. */
  fallback(format: GameFormat): string
}

const BOTH: readonly GameFormat[] = ['normal', 'commander']
const COMMANDER: readonly GameFormat[] = ['commander']

interface Spec<V> {
  id: string
  label: string
  help: string
  group: RuleGroup
  formats?: readonly GameFormat[]
  input: RuleInput
  get(rules: HouseRules): V | undefined
  put(rules: HouseRules, value: V): void
  /** The value the field text means; null when it means none. */
  from(text: string): V | null
  /** The field text of a value. */
  text(value: V): string
  /** The words for a value when it is named. */
  show(value: V): string
  /** The format's own values. */
  own?: Partial<Record<GameFormat, V>>
}

function rule<V>(spec: Spec<V>): RuleSpec {
  const { input } = spec
  const problem =
    input.kind === 'number'
      ? `Use a whole number from ${input.min} to ${input.max}${input.word === undefined ? '' : `, or "${input.word}"`}.`
      : 'Choose one of the listed values.'
  return {
    id: spec.id,
    label: spec.label,
    help: spec.help,
    group: spec.group,
    formats: spec.formats ?? BOTH,
    input,
    apply(rules, text) {
      const trimmed = text.trim()
      if (trimmed === '') return null
      const value = spec.from(trimmed)
      if (value === null) return problem
      spec.put(rules, value)
      return null
    },
    phrase(rules, format) {
      const value = spec.get(rules)
      if (value === undefined) return null
      const own = spec.own?.[format]
      return own !== undefined && JSON.stringify(own) === JSON.stringify(value) ? null : spec.show(value)
    },
    fallback(format) {
      const own = spec.own?.[format]
      return own === undefined ? '' : spec.text(own)
    },
  }
}

const whole = (text: string, min: number, max: number): number | null => {
  if (!/^\d{1,6}$/.test(text)) return null
  const value = Number(text)
  return value >= min && value <= max ? value : null
}

type CountKey =
  | 'starting_life'
  | 'opening_hand_size'
  | 'free_mulligans'
  | 'mulligan_cap'
  | 'draw_step_cards'
  | 'land_plays_per_turn'
  | 'turn_limit'
  | 'winning_life'
  | 'poison_threshold'
  | 'commander_tax'
  | 'commanders_per_seat'

interface Count {
  label: string
  help: string
  group: RuleGroup
  min: number
  max: number
  key: CountKey
  phrase(value: number): string
  own?: Partial<Record<GameFormat, number>>
  formats?: readonly GameFormat[]
}

const counted = (c: Count): RuleSpec =>
  rule<number>({
    id: c.key,
    label: c.label,
    help: c.help,
    group: c.group,
    ...(c.formats === undefined ? {} : { formats: c.formats }),
    input: { kind: 'number', min: c.min, max: c.max },
    get: (rules) => rules[c.key],
    put: (rules, value) => {
      rules[c.key] = value
    },
    from: (text) => whole(text, c.min, c.max),
    text: String,
    show: c.phrase,
    ...(c.own === undefined ? {} : { own: c.own }),
  })

const plural = (n: number, one: string, many = `${one}s`): string => `${n} ${n === 1 ? one : many}`

type Flag = keyof HouseRules &
  ('mana_burn' | 'open_hands' | 'revealed_library_top' | 'opening_hand_smoothing')

const flag = (id: Flag, label: string, help: string, group: RuleGroup, on: string, off: string): RuleSpec =>
  rule<boolean>({
    id,
    label,
    help,
    group,
    input: {
      kind: 'choice',
      options: [
        { value: 'on', label: 'On' },
        { value: 'off', label: 'Off' },
      ],
    },
    get: (rules) => rules[id],
    put: (rules, value) => {
      rules[id] = value
    },
    from: (text) => (text === 'on' ? true : text === 'off' ? false : null),
    text: (value) => (value ? 'on' : 'off'),
    show: (value) => (value ? on : off),
    own: { normal: false, commander: false },
  })

const deckFlag = (
  id: 'singleton' | 'colour_identity',
  label: string,
  help: string,
  on: string,
  off: string,
) =>
  rule<boolean>({
    id: `deck.${id}`,
    label,
    help,
    group: 'Commander',
    formats: COMMANDER,
    input: {
      kind: 'choice',
      options: [
        { value: 'on', label: 'On' },
        { value: 'off', label: 'Off' },
      ],
    },
    get: (rules) => rules.deck?.[id],
    put: (rules, value) => {
      rules.deck = { ...rules.deck, [id]: value }
    },
    from: (text) => (text === 'on' ? true : text === 'off' ? false : null),
    text: (value) => (value ? 'on' : 'off'),
    show: (value) => (value ? on : off),
    own: { commander: true },
  })

const deckCount = (id: 'deck_size' | 'max_copies', label: string, help: string, min: number, max: number) =>
  rule<number>({
    id: `deck.${id}`,
    label,
    help,
    group: 'Commander',
    formats: COMMANDER,
    input: { kind: 'number', min, max },
    get: (rules) => rules.deck?.[id],
    put: (rules, value) => {
      rules.deck = { ...rules.deck, [id]: value }
    },
    from: (text) => whole(text, min, max),
    text: String,
    show: (value) =>
      id === 'deck_size'
        ? `Decks of ${plural(value, 'card')}`
        : `Up to ${plural(value, 'copy', 'copies')} of a card`,
    ...(id === 'deck_size' ? { own: { commander: 100 } } : {}),
  })

interface Choice<V extends string> {
  id: string
  label: string
  help: string
  group: RuleGroup
  options: readonly { value: V; label: string; phrase: string }[]
  get(rules: HouseRules): V | undefined
  put(rules: HouseRules, value: V): void
  own?: V
}

const choice = <V extends string>(c: Choice<V>): RuleSpec =>
  rule<V>({
    id: c.id,
    label: c.label,
    help: c.help,
    group: c.group,
    input: { kind: 'choice', options: c.options.map(({ value, label }) => ({ value, label })) },
    get: c.get,
    put: c.put,
    from: (text) => c.options.find((option) => option.value === text)?.value ?? null,
    text: (value) => value,
    show: (value) => c.options.find((option) => option.value === value)?.phrase ?? value,
    ...(c.own === undefined ? {} : { own: { normal: c.own, commander: c.own } }),
  })

/** Every rule the create form edits, in the order of its groups. */
export const RULES: readonly RuleSpec[] = [
  counted({
    key: 'starting_life',
    label: 'Starting life',
    help: 'Life of every player.',
    group: 'Life and cards',
    min: 1,
    max: 1000,
    phrase: (n) => `Starting life ${n}`,
    own: { normal: 20, commander: 40 },
  }),
  counted({
    key: 'opening_hand_size',
    label: 'Opening hand',
    help: 'Cards in a starting hand.',
    group: 'Life and cards',
    min: 1,
    max: 20,
    phrase: (n) => `Opening hand of ${plural(n, 'card')}`,
    own: { normal: 7, commander: 7 },
  }),
  rule<NonNullable<HouseRules['max_hand_size']>>({
    id: 'max_hand_size',
    label: 'Hand limit',
    help: 'Cards kept at the end of the turn.',
    group: 'Life and cards',
    input: { kind: 'number', min: 0, max: 50, word: 'unlimited' },
    get: (rules) => rules.max_hand_size,
    put: (rules, value) => {
      rules.max_hand_size = value
    },
    from: (text) => {
      if (text.toLowerCase() === 'unlimited') return 'unlimited'
      const cards = whole(text, 0, 50)
      return cards === null ? null : { cards }
    },
    text: (value) => (value === 'unlimited' ? 'unlimited' : String(value.cards)),
    show: (value) => (value === 'unlimited' ? 'No hand limit' : `Hand limit ${value.cards}`),
    own: { normal: { cards: 7 }, commander: { cards: 7 } },
  }),
  counted({
    key: 'draw_step_cards',
    label: 'Cards drawn each turn',
    help: 'Drawn in the draw step.',
    group: 'Life and cards',
    min: 1,
    max: 10,
    phrase: (n) => `${plural(n, 'card')} drawn each turn`,
    own: { normal: 1, commander: 1 },
  }),
  counted({
    key: 'land_plays_per_turn',
    label: 'Land plays per turn',
    help: 'Lands each player may play on their turn.',
    group: 'Life and cards',
    min: 1,
    max: 10,
    phrase: (n) => `${plural(n, 'land play')} per turn`,
    own: { normal: 1, commander: 1 },
  }),
  choice<MulliganStyle>({
    id: 'mulligan_style',
    label: 'Mulligan style',
    help: 'London bottoms cards; Paris deals fewer; Vancouver scries.',
    group: 'Mulligans',
    options: [
      { value: 'london', label: 'London', phrase: 'London mulligan' },
      { value: 'paris', label: 'Paris', phrase: 'Paris mulligan' },
      { value: 'vancouver', label: 'Vancouver', phrase: 'Vancouver mulligan' },
    ],
    get: (rules) => rules.mulligan_style,
    put: (rules, value) => {
      rules.mulligan_style = value
    },
    own: 'london',
  }),
  counted({
    key: 'free_mulligans',
    label: 'Free mulligans',
    help: 'Mulligans that put nothing on the bottom.',
    group: 'Mulligans',
    min: 0,
    max: 100,
    phrase: (n) => `${plural(n, 'free mulligan')}`,
    own: { normal: 0, commander: 1 },
  }),
  counted({
    key: 'mulligan_cap',
    label: 'Most mulligans',
    help: '0 forbids them.',
    group: 'Mulligans',
    min: 0,
    max: 100,
    phrase: (n) => (n === 0 ? 'No mulligans' : `At most ${plural(n, 'mulligan')}`),
  }),
  flag(
    'opening_hand_smoothing',
    'Smoothed opening hands',
    'Of two hands, keeps the one nearer the deck’s land share.',
    'Mulligans',
    'Smoothed opening hands',
    'Opening hands not smoothed',
  ),
  choice<SkipFirstDraw>({
    id: 'skip_first_draw',
    label: 'First player’s draw',
    help: 'Whether the first player skips their first draw.',
    group: 'Turns and winning',
    options: [
      { value: 'always', label: 'Always skipped', phrase: 'First draw always skipped' },
      { value: 'never', label: 'Never skipped', phrase: 'First draw never skipped' },
    ],
    get: (rules) => rules.skip_first_draw,
    put: (rules, value) => {
      rules.skip_first_draw = value
    },
  }),
  counted({
    key: 'turn_limit',
    label: 'Turn limit',
    help: 'Rounds after which the highest life wins.',
    group: 'Turns and winning',
    min: 1,
    max: 1000,
    phrase: (n) => `Ends after ${plural(n, 'round')}`,
  }),
  counted({
    key: 'winning_life',
    label: 'Winning life',
    help: 'A player at this life or more wins.',
    group: 'Turns and winning',
    min: 1,
    max: 10000,
    phrase: (n) => `Win at ${n} life`,
  }),
  counted({
    key: 'poison_threshold',
    label: 'Poison counters to lose',
    help: 'Counters that make a player lose.',
    group: 'Turns and winning',
    min: 1,
    max: 100,
    phrase: (n) => `Lose at ${plural(n, 'poison counter')}`,
    own: { normal: 10, commander: 10 },
  }),
  choice<EmptyLibrary>({
    id: 'empty_library',
    label: 'Empty library',
    help: 'What drawing from an empty library does.',
    group: 'Turns and winning',
    options: [
      { value: 'lose', label: 'Lose the game', phrase: 'Empty library loses' },
      {
        value: 'reshuffle_graveyard',
        label: 'Reshuffle the graveyard',
        phrase: 'Empty library reshuffles the graveyard',
      },
    ],
    get: (rules) => rules.empty_library,
    put: (rules, value) => {
      rules.empty_library = value
    },
    own: 'lose',
  }),
  flag(
    'mana_burn',
    'Mana burn',
    'Unspent mana costs 1 life each.',
    'Turns and winning',
    'Mana burn',
    'No mana burn',
  ),
  choice<AttackRange>({
    id: 'attack_range',
    label: 'Who can be attacked',
    help: 'Every opponent, the players next to you, or the next player only.',
    group: 'Table',
    options: [
      { value: 'any', label: 'Any opponent', phrase: 'Attack any opponent' },
      { value: 'neighbours', label: 'Neighbours only', phrase: 'Attack neighbours only' },
      { value: 'left', label: 'Next player only', phrase: 'Attack the next player only' },
    ],
    get: (rules) => rules.attack_range,
    put: (rules, value) => {
      rules.attack_range = value
    },
    own: 'any',
  }),
  flag('open_hands', 'Open hands', 'Every player sees every hand.', 'Table', 'Open hands', 'Hands hidden'),
  flag(
    'revealed_library_top',
    'Revealed top cards',
    'Every player sees the top card of every library.',
    'Table',
    'Top cards revealed',
    'Top cards hidden',
  ),
  counted({
    key: 'commander_tax',
    label: 'Commander tax',
    help: 'Generic mana more for each earlier cast from the command zone.',
    group: 'Commander',
    formats: COMMANDER,
    min: 0,
    max: 10,
    phrase: (n) => (n === 0 ? 'No commander tax' : `Commander tax ${n}`),
    own: { commander: 2 },
  }),
  rule<NonNullable<HouseRules['commander_damage']>>({
    id: 'commander_damage',
    label: 'Commander damage',
    help: 'Combat damage from one commander that loses the game.',
    group: 'Commander',
    formats: COMMANDER,
    input: { kind: 'number', min: 1, max: 100, word: 'off' },
    get: (rules) => rules.commander_damage,
    put: (rules, value) => {
      rules.commander_damage = value
    },
    from: (text) => {
      if (text.toLowerCase() === 'off') return 'off'
      const lethal = whole(text, 1, 100)
      return lethal === null ? null : { lethal }
    },
    text: (value) => (value === 'off' ? 'off' : String(value.lethal)),
    show: (value) => (value === 'off' ? 'No commander damage loss' : `Commander damage ${value.lethal}`),
    own: { commander: { lethal: 21 } },
  }),
  deckFlag(
    'singleton',
    'One copy of each card',
    'Lifts the limit of one copy of a card when off.',
    'Singleton decks',
    'Copies of a card unlimited',
  ),
  deckFlag(
    'colour_identity',
    'Colour identity',
    'Lets any card in when off.',
    'Colour identity enforced',
    'Colour identity not enforced',
  ),
  deckCount('deck_size', 'Deck size', 'Cards in a deck with its commanders.', 40, 250),
  deckCount('max_copies', 'Copies of a card', 'Most copies when one copy is off.', 1, 99),
  counted({
    key: 'commanders_per_seat',
    label: 'Commanders per player',
    help: '1 refuses a Partner pair; 2 allows any two.',
    group: 'Commander',
    formats: COMMANDER,
    min: 1,
    max: 2,
    phrase: (n) => plural(n, 'commander') + ' per player',
  }),
]

/** The rules that apply in `format`, grouped in the order the form shows them. */
export function ruleGroups(format: GameFormat): { title: RuleGroup; specs: RuleSpec[] }[] {
  const groups: { title: RuleGroup; specs: RuleSpec[] }[] = []
  for (const spec of RULES) {
    if (!spec.formats.includes(format)) continue
    const last = groups[groups.length - 1]
    if (last?.title === spec.group) last.specs.push(spec)
    else groups.push({ title: spec.group, specs: [spec] })
  }
  return groups
}

/** A format key as the wire spells it, or Normal for any other. */
export const gameFormat = (key: string): GameFormat =>
  key.toLowerCase() === 'commander' ? 'commander' : 'normal'

/** The rules the field texts mean in `format`, and the problem of each field that holds a bad text. */
export function rulesFromTexts(
  texts: RuleTexts,
  format: GameFormat,
): { rules: HouseRules; problems: Readonly<Record<string, string>> } {
  const rules: HouseRules = {}
  const problems: Record<string, string> = {}
  for (const spec of RULES) {
    if (!spec.formats.includes(format)) continue
    const problem = spec.apply(rules, texts[spec.id] ?? '')
    if (problem !== null) problems[spec.id] = problem
  }
  if (rules.deck?.max_copies !== undefined && rules.deck.singleton !== false) {
    problems['deck.max_copies'] = 'Set “One copy of each card” to Off to allow more copies.'
  }
  return { rules, problems }
}

/** The rules as the request carries them: `undefined` when none is set. */
export function requestRules(texts: RuleTexts, format: GameFormat): HouseRules | undefined {
  const { rules } = rulesFromTexts(texts, format)
  return Object.keys(rules).length === 0 ? undefined : rules
}

/** The words for each rule that differs from the format's own values, in the form's order; handicaps are named last. */
export function describeRules(rules: HouseRules | undefined, format: string): string[] {
  if (rules === undefined) return []
  const kind = gameFormat(format)
  const named = RULES.filter((spec) => spec.formats.includes(kind))
    .map((spec) => spec.phrase(rules, kind))
    .filter((words) => words !== null)
  const handicaps = rules.handicaps?.length ?? 0
  return handicaps === 0 ? named : [...named, `Handicaps for ${plural(handicaps, 'player')}`]
}
