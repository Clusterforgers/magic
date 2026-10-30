/** The note of a play the engine took back: a cast or activation the viewer began could not go on, and why. */
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import type { PlayState } from '@/play/store'
import { REASON_WORDS } from '../handReason'
import { useUntilAnswered } from './nothingToDo'

/** The newest `AnnouncementRewound` of the viewer, in words, and where it sits in the log. */
export interface RewoundNote {
  seq: number
  text: string
}

/** The newest play of the viewer taken back, while nothing went on the stack since in this turn, or null; wrap in `useShallow`. */
export function latestRewound(state: PlayState): RewoundNote | null {
  if (state.seat === null) return null
  for (let index = state.log.length - 1; index >= 0; index -= 1) {
    const entry = state.log[index]
    if (entry === undefined || !('Public' in entry.event)) continue
    const event = entry.event.Public
    if (typeof event === 'string') continue
    if ('TurnBegan' in event || 'SpellCast' in event || 'AbilityActivated' in event) return null
    if ('AnnouncementRewound' in event && event.AnnouncementRewound.player === state.seat) {
      const { object, reason } = event.AnnouncementRewound
      const name = state.objectsById[object]?.name || 'That card'
      return { seq: entry.seq, text: `${name} could not be played: ${REASON_WORDS[reason].line.toLowerCase()}` }
    }
  }
  return null
}

/** The note to show: the newest one, until the player has been asked something after it and answered it. */
export function useRewound(): RewoundNote | null {
  const note = usePlay(useShallow(latestRewound))
  return useUntilAnswered(note, note === null ? null : String(note.seq))
}
