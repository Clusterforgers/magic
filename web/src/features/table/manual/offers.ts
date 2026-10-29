/** What the menu of a card offers: the manual actions the protocol allows for that card and seat, as of now. */
import type {
  CommonToken,
  CounterKind,
  ManualAction,
  ManualClause,
  ManualToken,
  ManualZone,
  ObjectId,
  ObjectView,
  PlayerId,
  Zone,
} from '@/protocol/generated'

/** The zone of an object, without its owner. */
export type ZoneKind =
  'battlefield' | 'hand' | 'graveyard' | 'exile' | 'command' | 'library' | 'stack' | 'phased'

/** The kind of zone `zone` is. */
export function zoneKind(zone: Zone): ZoneKind {
  if (zone === 'Battlefield') return 'battlefield'
  if (zone === 'Exile') return 'exile'
  if (zone === 'Stack') return 'stack'
  if (zone === 'PhasedOut') return 'phased'
  if ('Hand' in zone) return 'hand'
  if ('Graveyard' in zone) return 'graveyard'
  if ('Library' in zone) return 'library'
  return 'command'
}

/** Why nothing can be done by hand now. */
export const NEEDS_PRIORITY = 'You act by hand only while you hold priority.'

/** The limits of the protocol (`ManualAction`). */
export const LIMITS = { counters: 99, life: 1000, cards: 10, tokens: 10, name: 24 } as const

/** The forms that ask for what an action still needs. */
export type Ask =
  | 'addCounters'
  | 'removeCounters'
  | 'attach'
  | 'token'
  | 'gainLife'
  | 'loseLife'
  | 'addPlayerCounters'
  | 'removePlayerCounters'
  | 'draw'
  | 'mill'
  | 'moveTop'

/** One entry of the menu: an action sent as it is (`action`) or the form that completes it (`ask`). */
export interface ManualItem {
  key: string
  label: string
  /** Words after the label, such as how many counters there are. */
  hint?: string
  /** Why the entry cannot be used now; it is listed, inert. */
  unavailable?: string
  action?: ManualAction
  ask?: Ask
}

/** The groups of the menu, in order. */
export type GroupId = 'move' | 'counters' | 'tap' | 'attach' | 'token' | 'other'

/** A titled run of entries, or a note that says why it has none. */
export interface ManualGroup {
  id: GroupId
  label: string
  note?: string
  items: ManualItem[]
}

/** The menu of one card. */
export interface ManualMenu {
  title: string
  /** Set while nothing can be done by hand, and why. */
  notice?: string
  /** The clauses of the card's rules that the engine leaves to the table: what the seat applies by hand. */
  clauses: readonly ManualClause[]
  groups: ManualGroup[]
}

/** What the menu is made from. */
export interface MenuInput {
  seat: PlayerId
  object: ObjectView
  /** Whether the seat faces its own priority decision, the only moment a manual action is taken. */
  holding: boolean
  /** Whether the format has a command zone to move a card to. */
  hasCommandZone: boolean
  /** Cards in the seat's library. */
  library: number
  /** The seat's own counters. */
  counters: readonly (readonly [CounterKind, number])[]
  /** The name of the card's owner. */
  ownerName: string
}

/** The words of a counter kind: "+1/+1", "poison", "oil". */
export function counterName(kind: CounterKind): string {
  return typeof kind === 'string' ? kind : kind.Other
}

/** The kinds the protocol names; any other counter is `{Other: name}`. */
export const KNOWN_COUNTERS = ['+1/+1', '-1/-1', 'loyalty', 'poison', 'charge'] as const

const same = (a: CounterKind, b: CounterKind): boolean => counterName(a) === counterName(b)

/** The counter kind a name stands for: a known kind keeps its own form. */
export function counterKind(name: string): CounterKind {
  const known = KNOWN_COUNTERS.find((kind) => kind === name)
  return known ?? { Other: name }
}

/** A counter name the protocol takes: letters, digits, spaces and `- + /`, one to 24 characters. */
export function validCounterName(name: string): boolean {
  return name.length > 0 && name.length <= LIMITS.name && /^[\p{L}\p{N} +/-]+$/u.test(name)
}

const ATTACHMENTS = ['Aura', 'Equipment', 'Fortification']

/** Whether the object is an Aura, an Equipment or a Fortification. */
export function isAttachment(object: ObjectView): boolean {
  return object.subtypes.some((subtype) => ATTACHMENTS.includes(subtype))
}

const isSpell = (object: ObjectView): boolean =>
  object.types.includes('Instant') || object.types.includes('Sorcery')

interface Destination {
  to: ManualZone
  label: string
  zone: ZoneKind | null
}

const DESTINATIONS: readonly Destination[] = [
  { to: 'Battlefield', label: 'battlefield', zone: 'battlefield' },
  { to: 'Hand', label: 'hand', zone: 'hand' },
  { to: 'Graveyard', label: 'graveyard', zone: 'graveyard' },
  { to: 'Exile', label: 'exile', zone: 'exile' },
  { to: 'LibraryTop', label: 'top of library', zone: null },
  { to: 'LibraryBottom', label: 'bottom of library', zone: null },
  { to: 'Command', label: 'command zone', zone: 'command' },
]

/** "Hand" for a seat's own card, "Bo's hand" for one it only controls. */
function placeLabel(destination: Destination, input: MenuInput): string {
  const { to, label } = destination
  const theirs = input.object.owner !== input.seat
  if (to === 'Battlefield' || to === 'Exile' || !theirs) return capital(label)
  return `${input.ownerName}'s ${label}`
}

const capital = (text: string): string => text.charAt(0).toUpperCase() + text.slice(1)

function moveGroup(input: MenuInput, off: string | undefined): ManualGroup {
  const { object, seat } = input
  const group: ManualGroup = { id: 'move', label: 'Move to', items: [] }
  const from = zoneKind(object.zone)
  if (from === 'stack')
    return { ...group, note: 'A spell on the stack resolves or is countered; it is not moved by hand.' }
  if (from === 'library')
    return { ...group, note: 'A card in a library is not named; use the top-of-library entries.' }
  if (object.owner !== seat && object.controller !== seat) {
    return { ...group, note: `Only ${input.ownerName}, who owns or controls it, moves this card.` }
  }
  if (object.kind === 'Token') {
    const note = 'A token that leaves the battlefield ceases to exist.'
    const items: ManualItem[] =
      from === 'battlefield'
        ? [
            {
              key: 'remove-token',
              label: 'Remove token',
              hint: 'ceases to exist',
              action: { Move: { object: object.id, to: 'Graveyard' } },
              unavailable: off,
            },
          ]
        : []
    return { ...group, items, note }
  }
  if (from === 'hand') {
    group.items.push({
      key: 'discard',
      label: 'Discard',
      action: { Discard: { object: object.id } },
      unavailable: off,
    })
  }
  for (const destination of DESTINATIONS) {
    if (destination.zone === from) continue
    if (destination.to === 'Command' && !input.hasCommandZone) continue
    const spellOnTable = destination.to === 'Battlefield' && isSpell(object)
    group.items.push({
      key: `move-${destination.to}`,
      label: placeLabel(destination, input),
      hint: from === 'hand' && destination.to === 'Graveyard' ? 'not a discard' : undefined,
      action: { Move: { object: object.id, to: destination.to } },
      unavailable: off ?? (spellOnTable ? 'An instant or sorcery cannot enter the battlefield.' : undefined),
    })
  }
  return group
}

function countersGroup(input: MenuInput, off: string | undefined): ManualGroup | null {
  const { object } = input
  if (zoneKind(object.zone) !== 'battlefield') return null
  const held = object.counters ?? []
  const items: ManualItem[] = []
  const quick = (kind: CounterKind, count: number | null): void => {
    const name = counterName(kind)
    items.push({
      key: `add-${name}`,
      label: `Add a ${name} counter`,
      hint: count === null ? undefined : `${count} now`,
      action: { Counters: { object: object.id, counter: kind, delta: 1 } },
      unavailable: off,
    })
    if (count !== null) {
      items.push({
        key: `remove-${name}`,
        label: `Remove a ${name} counter`,
        action: { Counters: { object: object.id, counter: kind, delta: -1 } },
        unavailable: off,
      })
    }
  }
  for (const [kind, count] of held) quick(kind, count)
  if (held.length === 0 && object.types.includes('Creature')) quick('+1/+1', null)
  if (held.length === 0 && object.types.includes('Planeswalker')) quick('loyalty', null)
  items.push({ key: 'add-counters', label: 'Add counters…', ask: 'addCounters', unavailable: off })
  items.push({
    key: 'remove-counters',
    label: 'Remove counters…',
    ask: 'removeCounters',
    unavailable: off ?? (held.length === 0 ? 'There are no counters on it.' : undefined),
  })
  return { id: 'counters', label: 'Counters', items }
}

function tapGroup(input: MenuInput, off: string | undefined): ManualGroup | null {
  const { object } = input
  if (zoneKind(object.zone) !== 'battlefield') return null
  const tapped = object.permanent?.tapped === true
  const verb = tapped ? 'Untap' : 'Tap'
  const action: ManualAction = tapped ? { Untap: { object: object.id } } : { Tap: { object: object.id } }
  return { id: 'tap', label: 'Tap', items: [{ key: 'tap', label: verb, action, unavailable: off }] }
}

function attachGroup(input: MenuInput, off: string | undefined): ManualGroup | null {
  const { object, seat } = input
  if (zoneKind(object.zone) !== 'battlefield' || !isAttachment(object)) return null
  const group: ManualGroup = { id: 'attach', label: 'Attach', items: [] }
  if (object.owner !== seat && object.controller !== seat) {
    return { ...group, note: `Only ${input.ownerName}, who owns or controls it, attaches this.` }
  }
  group.items.push({ key: 'attach', label: 'Attach to…', ask: 'attach', unavailable: off })
  if (object.attached_to !== undefined) {
    group.items.push({
      key: 'unattach',
      label: 'Unattach',
      action: { Unattach: { object: object.id } },
      unavailable: off,
    })
  }
  return group
}

function otherGroup(input: MenuInput, off: string | undefined): ManualGroup {
  const noCards = input.library === 0 ? 'Your library is empty.' : undefined
  const items: ManualItem[] = [
    { key: 'draw', label: 'Draw cards…', ask: 'draw', unavailable: off },
    { key: 'mill', label: 'Mill cards…', ask: 'mill', unavailable: off ?? noCards },
    { key: 'move-top', label: 'Move top cards of library…', ask: 'moveTop', unavailable: off ?? noCards },
    { key: 'gain-life', label: 'Gain life…', ask: 'gainLife', unavailable: off },
    { key: 'lose-life', label: 'Lose life…', ask: 'loseLife', unavailable: off },
    { key: 'add-player', label: 'Add your counters…', ask: 'addPlayerCounters', unavailable: off },
    {
      key: 'remove-player',
      label: 'Remove your counters…',
      ask: 'removePlayerCounters',
      unavailable: off ?? (input.counters.length === 0 ? 'You have no counters.' : undefined),
    },
  ]
  return { id: 'other', label: 'You', items }
}

/**
 * The menu of one card for the seat that faces its priority decision (`holding`), as the protocol allows it
 * (`ManualAction`): moves, discards and attachments of what the seat owns or controls, counters and tapping of any
 * permanent, and life, counters, draws, library cards and tokens of the seat itself. Without priority the entries are
 * listed and inert.
 */
export function manualMenu(input: MenuInput): ManualMenu {
  const off = input.holding ? undefined : NEEDS_PRIORITY
  const groups = [
    moveGroup(input, off),
    countersGroup(input, off),
    tapGroup(input, off),
    attachGroup(input, off),
    {
      id: 'token' as const,
      label: 'Token',
      items: [{ key: 'token', label: 'Create a token…', ask: 'token' as const, unavailable: off }],
    },
    otherGroup(input, off),
  ].filter((group): group is ManualGroup => group !== null)
  return { title: input.object.name, notice: off, clauses: input.object.manual ?? [], groups }
}

/** The permanents an attachment can go to: every other permanent but its host, the seat's own first, then by name. */
export function attachTargets(
  objects: Iterable<ObjectView>,
  attachment: ObjectView,
  seat: PlayerId,
): ObjectView[] {
  const found: ObjectView[] = []
  for (const object of objects) {
    if (object.zone !== 'Battlefield' || object.kind === 'Ability') continue
    if (object.id === attachment.id || object.id === attachment.attached_to) continue
    found.push(object)
  }
  const rank = (object: ObjectView): number => (object.controller === seat ? 0 : 1)
  return found.sort((a, b) => rank(a) - rank(b) || a.name.localeCompare(b.name) || a.id - b.id)
}

/** The tokens the protocol lists, in the order a menu shows them (`CommonToken::ALL`). */
export const COMMON_TOKENS: readonly CommonToken[] = [
  'Treasure',
  'Clue',
  'Food',
  'Soldier',
  'Zombie',
  'Goblin',
  'Spirit',
  'Saproling',
  'Beast',
  'Thopter',
  'Servo',
  'Human',
  'Insect',
  'Squirrel',
  'Wolf',
  'Bird',
  'Dragon',
  'Angel',
]

/** What a form has typed so far. */
export interface Draft {
  count: number
  /** A kind of `KNOWN_COUNTERS`, or `other` with the name in `other`. */
  kind: string
  other: string
  to: ManualZone
  target: ObjectId | null
  /** A `CommonToken` or `custom`. */
  token: string
  name: string
  power: number
  toughness: number
}

/** The start of every form. */
export const EMPTY_DRAFT: Draft = {
  count: 1,
  kind: '+1/+1',
  other: '',
  to: 'Graveyard',
  target: null,
  token: 'Treasure',
  name: '',
  power: 1,
  toughness: 1,
}

/** The kind a draft names, or null while it names none the protocol takes. */
export function draftKind(draft: Draft): CounterKind | null {
  if (draft.kind !== 'other') return counterKind(draft.kind)
  const name = draft.other.trim()
  return validCounterName(name) ? counterKind(name) : null
}

/** The draft a form starts from: removal starts on a kind that is held. */
export function startDraft(ask: Ask, input: Omit<BuildInput, 'draft'>): Draft {
  const held =
    ask === 'removeCounters'
      ? input.object.counters
      : ask === 'removePlayerCounters'
        ? input.counters
        : undefined
  const first = held?.at(0)
  return first === undefined ? EMPTY_DRAFT : { ...EMPTY_DRAFT, kind: counterName(first[0]) }
}

/** The result of building an action: the action, or why the form is not complete. */
export type Built = { action: ManualAction } | { problem: string }

const problem = (text: string): { problem: string } => ({ problem: text })

const whole = (value: number, low: number, high: number): boolean =>
  Number.isInteger(value) && value >= low && value <= high

/** How many of `kind` a list of counters holds. */
export function heldOf(held: readonly (readonly [CounterKind, number])[], kind: CounterKind): number {
  return held.find(([other]) => same(other, kind))?.[1] ?? 0
}

/** What `buildAction` needs besides the menu's own input. */
export interface BuildInput extends MenuInput {
  /** What the form has typed. */
  draft: Draft
}

/** The action a filled form stands for, or the first thing wrong with it. */
export function buildAction(ask: Ask, input: BuildInput): Built {
  const { object, seat, draft } = input
  const { count } = draft
  const range = (low: number, high: number, what: string): Built | null =>
    whole(count, low, high) ? null : problem(`Enter ${what} from ${low} to ${high}.`)
  switch (ask) {
    case 'addCounters':
    case 'removeCounters': {
      const kind = draftKind(draft)
      if (kind === null) return problem('Name the counter with letters, digits, spaces and - + /.')
      const adding = ask === 'addCounters'
      const held = heldOf(object.counters ?? [], kind)
      const bad = range(1, adding ? LIMITS.counters : Math.min(LIMITS.counters, held), 'a number')
      if (!adding && held === 0) return problem(`There is no ${counterName(kind)} counter on it.`)
      if (bad !== null) return bad
      return { action: { Counters: { object: object.id, counter: kind, delta: adding ? count : -count } } }
    }
    case 'addPlayerCounters':
    case 'removePlayerCounters': {
      const kind = draftKind(draft)
      if (kind === null) return problem('Name the counter with letters, digits, spaces and - + /.')
      const adding = ask === 'addPlayerCounters'
      const held = heldOf(input.counters, kind)
      if (!adding && held === 0) return problem(`You have no ${counterName(kind)} counter.`)
      const bad = range(1, adding ? LIMITS.counters : Math.min(LIMITS.counters, held), 'a number')
      if (bad !== null) return bad
      return {
        action: { PlayerCounters: { player: seat, counter: kind, delta: adding ? count : -count } },
      }
    }
    case 'gainLife':
    case 'loseLife': {
      const bad = range(1, LIMITS.life, 'an amount')
      if (bad !== null) return bad
      return { action: { Life: { player: seat, delta: ask === 'gainLife' ? count : -count } } }
    }
    case 'draw': {
      const bad = range(1, LIMITS.cards, 'a number of cards')
      return bad ?? { action: { Draw: { count } } }
    }
    case 'mill':
    case 'moveTop': {
      const bad = range(1, Math.min(LIMITS.cards, input.library), 'a number of cards')
      if (bad !== null) return bad
      return { action: { MoveTop: { count, to: ask === 'mill' ? 'Graveyard' : draft.to } } }
    }
    case 'attach':
      return draft.target === null
        ? problem('Choose what to attach it to.')
        : { action: { Attach: { object: object.id, to: draft.target } } }
    case 'token': {
      const bad = range(1, LIMITS.tokens, 'a number of tokens')
      if (bad !== null) return bad
      const token = tokenOf(draft)
      return 'problem' in token ? token : { action: { CreateToken: { token: token.token, count } } }
    }
  }
}

function tokenOf(draft: Draft): { token: ManualToken } | { problem: string } {
  if (draft.token !== 'custom') {
    const common = COMMON_TOKENS.find((token) => token === draft.token)
    return common === undefined ? problem('Choose a token.') : { token: { Common: common } }
  }
  const name = draft.name.trim()
  if (name.length === 0 || name.length > 40) return problem('Name the token in 1 to 40 characters.')
  if (!whole(draft.power, 0, 99) || !whole(draft.toughness, 0, 99)) {
    return problem('Power and toughness are whole numbers from 0 to 99.')
  }
  return {
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
