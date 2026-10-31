/** The Priority prompt in plain words. */
import type { PlayState } from '@/play/store'
import type { ManaOption, ObjectId, PlayerId, PriorityOptions } from '@/protocol/generated'

type Names = (seat: PlayerId) => string

/** The spell on top of the stack, which the player would be responding to. */
export interface StackTop {
  name: string
  controller: PlayerId
}

/** The top of the stack, or null when it is empty; wrap in `useShallow`. */
export function topOfStack(state: PlayState): StackTop | null {
  const id = state.view?.stack.cards?.at(-1)
  const object = id === undefined ? undefined : state.objectsById[id]
  return object === undefined ? null : { name: object.name, controller: object.controller }
}

/** "a", "a or b", "a, b or c". */
function orList(items: readonly string[]): string {
  const last = items.at(-1)
  if (last === undefined) return ''
  return items.length === 1 ? last : `${items.slice(0, -1).join(', ')} or ${last}`
}

/** What a seat held at its by-hand window reads: the clauses of its resolved card are its to apply before it passes. */
export const BY_HAND_TEXT = "Apply the card's text by hand, then pass."

/**
 * What the player may do and, over a non-empty stack, that they are responding to its top spell; `byHand` (the seat is
 * held at its by-hand window, `PlayState.byHand`) says what is left to do by hand instead.
 */
export function priorityText(
  options: PriorityOptions,
  top: StackTop | null,
  viewer: PlayerId,
  name: Names,
  byHand = false,
): string {
  if (byHand) return BY_HAND_TEXT
  const lead =
    top === null
      ? 'You have priority.'
      : `You are responding to ${top.controller === viewer ? 'your' : `${name(top.controller)}'s`} ${top.name}.`
  const can = [
    options.playable_lands.length > 0 ? 'play a land' : null,
    options.castable.length > 0 ? 'cast a spell' : null,
    options.mana_abilities.length > 0 ? 'tap for mana' : null,
  ].filter((what) => what !== null)
  const after = top === null ? '' : ' to let it resolve'
  if (can.length === 0) {
    return options.can_pass
      ? `${lead} You have nothing to play right now; pass${top === null ? ' to go on' : after}.`
      : `${lead} You have no legal move.`
  }
  return options.can_pass
    ? `${lead} You can ${orList(can)}, or pass${after}.`
    : `${lead} You can ${orList(can)}.`
}

/** The legal mana abilities grouped by permanent, in the order the options list them. */
export function manaByPermanent(
  options: readonly ManaOption[],
): ReadonlyMap<ObjectId, readonly ManaOption[]> {
  const grouped = new Map<ObjectId, ManaOption[]>()
  for (const option of options) {
    const list = grouped.get(option.source.object)
    if (list === undefined) grouped.set(option.source.object, [option])
    else list.push(option)
  }
  return grouped
}
