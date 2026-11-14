/**
 * The sounds of what a batch did at the table (docs/ui-audit.md, Game feel): pure over the batch's events, so it is
 * tested without audio. Every sound comes from an event every seat's log holds (a draw or a shuffle of another seat
 * is its hidden event, which says no more than that it happened), so every seat hears the same; nothing reads timing.
 */
import { batchBetween } from '@/features/table/fx/batch'
import type { PlayState } from '@/play/store'
import type { EventView } from '@/protocol/generated'

/** A sound of the table, by what happened. */
export type TableSound =
  'attack' | 'block' | 'hit' | 'die' | 'cast' | 'counter' | 'resolve' | 'gain' | 'land' | 'shuffle' | 'draw'

/** The order a batch plays its sounds in, the strongest first; a batch plays at most `MAX_TABLE_SOUNDS`. */
export const SOUND_ORDER: readonly TableSound[] = [
  'attack',
  'block',
  'hit',
  'die',
  'cast',
  'counter',
  'resolve',
  'gain',
  'land',
  'shuffle',
  'draw',
]

/** The most sounds one batch plays, each kind once, so a big batch is a short phrase, never a din. */
export const MAX_TABLE_SOUNDS = 3

/** The sound of one event, or null. */
export function soundOf(event: EventView): TableSound | null {
  if ('Hidden' in event) {
    const hidden = event.Hidden
    if (typeof hidden === 'string') return null
    if ('CardDrawn' in hidden) return 'draw'
    return 'LibraryShuffled' in hidden ? 'shuffle' : null
  }
  const shown = event.Public
  if (typeof shown === 'string') return null
  if ('AttackersDeclared' in shown) return shown.AttackersDeclared.attackers.length > 0 ? 'attack' : null
  if ('BlockersDeclared' in shown) return shown.BlockersDeclared.blocks.length > 0 ? 'block' : null
  if ('DamageDealt' in shown) return 'hit'
  if ('CreatureDied' in shown) return 'die'
  if ('SpellCast' in shown) return 'cast'
  if ('ResolutionEnded' in shown) return shown.ResolutionEnded.outcome === 'Resolved' ? 'resolve' : 'counter'
  if ('LifeGained' in shown) return 'gain'
  if ('LandPlayed' in shown) return 'land'
  if ('LibraryShuffled' in shown) return 'shuffle'
  return 'CardDrawn' in shown ? 'draw' : null
}

/** The sounds of these events: each kind once, in `SOUND_ORDER`, at most `MAX_TABLE_SOUNDS`. */
export function soundsOf(events: readonly EventView[]): TableSound[] {
  const heard = new Set<TableSound>()
  for (const event of events) {
    const sound = soundOf(event)
    if (sound !== null) heard.add(sound)
  }
  return SOUND_ORDER.filter((sound) => heard.has(sound)).slice(0, MAX_TABLE_SOUNDS)
}

/** The sounds of the change from `prev` to `next`: none unless it is a batch (a snapshot, a decision or chat plays nothing). */
export function tableSoundsBetween(prev: PlayState, next: PlayState): TableSound[] {
  const step = batchBetween(prev, next)
  return step === null ? [] : soundsOf(step.events)
}
