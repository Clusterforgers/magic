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

/** What a seat held at its by-hand window reads: the clauses of its resolved card `card` are its to apply, then it goes on. */
export const byHandText = (card: string): string => `Apply ${card}'s text by hand, then continue.`

/**
 * What the player may do in one line ("Play a land, cast a spell or tap for mana, or pass."), led over a non-empty stack by
 * the spell they respond to ("Responding to Bo's Giant Growth: …"); `byHand` (the card the seat is
 * held at its by-hand window for, `PlayState.byHand`) says what is left to do by hand instead.
 */
export function priorityText(
  options: PriorityOptions,
  top: StackTop | null,
  viewer: PlayerId,
  name: Names,
  byHand: string | null = null,
): string {
  if (byHand !== null) return byHandText(byHand)
  // One short line, as Arena words it: the strip's corner already says who holds priority.
  const lead =
    top === null
      ? ''
      : `Responding to ${top.controller === viewer ? 'your' : `${name(top.controller)}'s`} ${top.name}: `
  const say = (rest: string): string =>
    lead === '' ? rest.charAt(0).toUpperCase() + rest.slice(1) : lead + rest
  const can = [
    options.playable_lands.length > 0 ? 'play a land' : null,
    options.castable.length > 0 ? 'cast a spell' : null,
    (options.abilities?.length ?? 0) > 0 ? 'use an ability' : null,
    (options.unlockable?.length ?? 0) > 0 ? 'unlock a door' : null,
    (options.endable?.length ?? 0) > 0 ? 'pay to end an effect' : null,
    (options.ignorable?.length ?? 0) > 0 ? 'pay to ignore an effect' : null,
    options.companion === true ? 'bring your companion to hand' : null,
    options.mana_abilities.length > 0 ? 'tap for mana' : null,
  ].filter((what) => what !== null)
  const after = top === null ? '' : ' to let it resolve'
  if (can.length === 0) {
    return options.can_pass
      ? say(`nothing to play; pass${top === null ? ' to go on' : after}.`)
      : say('no legal move.')
  }
  return options.can_pass ? say(`${orList(can)}, or pass${after}.`) : say(`${orList(can)}.`)
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
