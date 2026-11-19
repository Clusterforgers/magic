import type { PlayState } from '@/play/store'
import type {
  Answer,
  ManaOption,
  ObjectId,
  ObjectView,
  PriorityOptions,
  StateView,
} from '@/protocol/generated'
import { MANA_TYPES } from '../prompt/payment'

/**
 * The dev table's answer to tapping for mana at priority, and to pressing the source again, as the server gives them:
 * the source taps and its mana floats, and a second press (`UndoMana`) untaps it and takes the mana back. Any other
 * answer is `null`, left to the fixture session. `tapped` keeps the ability each source was tapped with, to give back.
 */
export function devManaAnswer(
  state: PlayState,
  answer: Answer,
  tapped: Map<ObjectId, ManaOption>,
): Partial<PlayState> | null {
  const { pending, view } = state
  if (pending === null || view === null || !('Priority' in pending.decision.kind)) return null
  if (typeof answer !== 'object' || !('Priority' in answer)) return null
  const options = pending.decision.kind.Priority
  const action = answer.Priority
  if (typeof action !== 'object') return null
  if ('ActivateMana' in action) {
    const option = options.mana_abilities.find((one) => sameSource(one, action.ActivateMana))
    if (option === undefined) return null
    const next: PriorityOptions = {
      ...options,
      mana_abilities: options.mana_abilities.filter((one) => one.source.object !== option.source.object),
      undo_mana: [...(options.undo_mana ?? []), { source: option.source.object }],
    }
    tapped.set(option.source.object, option)
    return moved(state, view, option.source.object, true, option, 1, next)
  }
  if ('UndoMana' in action) {
    const option = tapped.get(action.UndoMana)
    if (option === undefined) return null
    tapped.delete(action.UndoMana)
    const next: PriorityOptions = {
      ...options,
      mana_abilities: [...options.mana_abilities, option],
      undo_mana: (options.undo_mana ?? []).filter((one) => one.source !== action.UndoMana),
    }
    return moved(state, view, action.UndoMana, false, option, -1, next)
  }
  return null
}

const sameSource = (option: ManaOption, source: ManaOption['source']): boolean =>
  option.source.object === source.object && option.source.ability === source.ability

/** The state with `id` tapped or untapped, the mana of `option` added to or taken from the viewer's pool and `options` asked. */
function moved(
  state: PlayState,
  view: StateView,
  id: ObjectId,
  tap: boolean,
  option: ManaOption,
  sign: 1 | -1,
  options: PriorityOptions,
): Partial<PlayState> {
  const at = MANA_TYPES.indexOf(option.produces)
  const objects = view.objects.map((object): ObjectView => {
    const permanent = object.permanent
    if (object.id !== id || permanent === null) return object
    return { ...object, permanent: { ...permanent, tapped: tap } }
  })
  const players = view.players.map((player) => {
    if (player.seat !== view.viewer) return player
    const counts = [...player.mana.counts] as typeof player.mana.counts
    counts[at] = Math.max(0, (counts[at] ?? 0) + sign)
    return { ...player, mana: { ...player.mana, counts } }
  })
  const next = { ...view, objects, players }
  const pending = state.pending
  return {
    view: next,
    objectsById: {
      ...state.objectsById,
      ...Object.fromEntries(objects.filter((o) => o.id === id).map((o) => [o.id, o])),
    },
    pending:
      pending === null
        ? null
        : { ...pending, id: pending.id + 1, decision: { ...pending.decision, kind: { Priority: options } } },
  }
}
