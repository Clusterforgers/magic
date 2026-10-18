import { useState } from 'react'
import { announcement } from './lobbyModel'
import type { LobbyRow } from './lobbyModel'

interface Memory {
  rows: readonly LobbyRow[]
  known: boolean
  text: string
}

/**
 * The sentence for a polite live region: what changed between two looks at the lobby ("Bo is ready. 2 of 3 ready,
 * waiting for Cy to join"). It stays what it last was until something changes again, and is empty before that.
 * `known` says the rows are real data; the stand-in rows shown before the first answer are not news, so nothing is
 * said for the step from them to the real ones. Pass the same `rows` while nothing changed.
 */
export function useLobbyAnnouncement(rows: readonly LobbyRow[], known: boolean): string {
  const [memory, setMemory] = useState<Memory>({ rows, known, text: '' })
  if (memory.rows !== rows || memory.known !== known) {
    const said = memory.known && known ? announcement(memory.rows, rows) : null
    // Setting state while rendering is how React derives a value from the previous render.
    setMemory({ rows, known, text: said ?? memory.text })
  }
  return memory.text
}
