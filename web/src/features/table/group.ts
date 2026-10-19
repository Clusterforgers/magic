/** Pure grouping of identical permanents into stacks, so a battlefield row draws one box for many of a kind. */
import type { CounterKind, ObjectId, ObjectView } from '@/protocol/generated'

/** A zone whose cards would fall under this many px also merges identical cards that are no lands or tokens. */
export const GROUP_OTHERS_BELOW = 64

/**
 * What grouping reads from a permanent: the parts of an `ObjectView` that its face draws, plus counters and
 * attachments, which the wire does not carry yet.
 */
export type Groupable = Pick<
  ObjectView,
  | 'id'
  | 'name'
  | 'owner'
  | 'controller'
  | 'kind'
  | 'supertypes'
  | 'types'
  | 'subtypes'
  | 'colors'
  | 'power'
  | 'toughness'
  | 'abilities'
  | 'permanent'
> & {
  /** The counters on it, by kind. */
  counters?: readonly (readonly [CounterKind, number])[]
  /** The objects attached to it. */
  attachments?: readonly ObjectId[]
}

/** How `groupPermanents` merges. */
export interface GroupOptions {
  /** Also merge identical cards that are neither lands nor tokens; without it each of them is a stack of its own. */
  groupOthers?: boolean
}

/** Permanents drawn as one box. */
export interface Stack<T extends Groupable = Groupable> {
  /** Names the stack across draws: kept while any member stays, and shared with no other stack. */
  key: string
  /** How many permanents the stack holds. */
  count: number
  /** The permanents in the order they were given; the first is the one drawn while the stack is closed. */
  members: readonly T[]
}

const byText = (a: string, b: string): number => (a < b ? -1 : a > b ? 1 : 0)

// Equal counters are equal whatever order or split they are listed in, and a count of zero is no counter.
function countersOf(counters: Groupable['counters']): [string, number][] {
  const totals = new Map<string, number>()
  for (const [kind, amount] of counters ?? []) {
    const name = JSON.stringify(kind)
    totals.set(name, (totals.get(name) ?? 0) + amount)
  }
  return [...totals].filter(([, amount]) => amount !== 0).sort(([a], [b]) => byText(a, b))
}

const attachmentsOf = (attachments: Groupable['attachments']): ObjectId[] =>
  [...(attachments ?? [])].sort((a, b) => a - b)

// Everything a card face shows. Two permanents are interchangeable exactly when this is equal, so a stack never
// hides a pumped, damaged, equipped or tapped member behind the face of another.
function faceOf(permanent: Groupable): string {
  const state = permanent.permanent
  return JSON.stringify([
    permanent.name,
    permanent.controller,
    permanent.owner,
    permanent.kind ?? 'Card',
    permanent.supertypes,
    permanent.types,
    permanent.subtypes,
    permanent.colors,
    permanent.power,
    permanent.toughness,
    permanent.abilities,
    state?.tapped ?? false,
    state?.summoning_sick ?? false,
    state?.damage ?? 0,
    countersOf(permanent.counters),
    attachmentsOf(permanent.attachments),
  ])
}

const merges = (permanent: Groupable, options: GroupOptions): boolean =>
  options.groupOthers === true || permanent.kind === 'Token' || permanent.types.includes('Land')

/**
 * Merges permanents with the same face into stacks, in the order of each stack's first member. Lands and tokens
 * always merge, other cards only with `groupOthers`; tapped and untapped permanents are never in one stack. A merged
 * stack is keyed by its face and a permanent that stays alone by its id, so a key survives members joining and
 * leaving and no other stack's key moves.
 */
export function groupPermanents<T extends Groupable>(
  permanents: readonly T[],
  options: GroupOptions = {},
): Stack<T>[] {
  const stacks = new Map<string, T[]>()
  for (const permanent of permanents) {
    const key = merges(permanent, options) ? faceOf(permanent) : `#${permanent.id}`
    const members = stacks.get(key)
    if (members === undefined) stacks.set(key, [permanent])
    else members.push(permanent)
  }
  return Array.from(stacks, ([key, members]) => ({ key, count: members.length, members }))
}

/** The members of a stack in the order they were given, to draw each one or to ask for each one. */
export function expandStack<T extends Groupable>(stack: Stack<T>): readonly T[] {
  return stack.members
}
