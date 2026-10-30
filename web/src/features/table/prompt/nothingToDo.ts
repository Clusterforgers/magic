/** The "nothing to do" notes: the engine says a seat has no possible attackers or blockers instead of asking it. */
import { useEffect, useRef, useState } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import type { PlayState } from '@/play/store'
import type { NothingToDo } from '@/protocol/generated'

/** The newest `NothingToDo` event for this seat and where it sits in the log. */
export interface NothingToDoNote {
  seq: number
  kind: NothingToDo
}

/** Each kind in plain words. */
export const NOTE_TEXT: Record<NothingToDo, string> = {
  Attack: 'You have no creatures that can attack',
  Block: 'You have no creatures that can block',
}

/** The newest `NothingToDo` for the viewer since the current turn began, or null; wrap in `useShallow`. */
export function latestNothingToDo(state: PlayState): NothingToDoNote | null {
  if (state.seat === null) return null
  for (let index = state.log.length - 1; index >= 0; index -= 1) {
    const entry = state.log[index]
    if (entry === undefined || !('Public' in entry.event)) continue
    const event = entry.event.Public
    if (typeof event === 'string') continue
    if ('TurnBegan' in event) return null
    if ('NothingToDo' in event && event.NothingToDo.player === state.seat) {
      return { seq: entry.seq, kind: event.NothingToDo.kind }
    }
  }
  return null
}

/**
 * The note to show: the newest one of this turn, until the player has been asked something and answered it. The
 * engine usually asks for priority right after saying there is nothing to do, so the note outlasts that question.
 */
export function useNothingToDo(): NothingToDoNote | null {
  const note = usePlay(useShallow(latestNothingToDo))
  return useUntilAnswered(note, note === null ? null : `${note.seq}:${note.kind}`)
}

/** `note`, named by `key`, until the player has been asked something after it and answered it. */
export function useUntilAnswered<T>(note: T | null, key: string | null): T | null {
  const deciding = usePlay((state) => state.pending !== null)
  const [answered, setAnswered] = useState<string | null>(null)
  const seen = useRef<{ key: string | null; asked: boolean }>({ key: null, asked: false })
  useEffect(() => {
    if (key === null) return
    if (seen.current.key !== key) seen.current = { key, asked: deciding }
    else if (deciding) seen.current.asked = true
    else if (seen.current.asked) setAnswered(key)
  }, [key, deciding])
  return key === answered ? null : note
}
