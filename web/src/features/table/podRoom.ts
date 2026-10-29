import { createContext, useContext } from 'react'

/** What a pod column knows about the room around it. */
export interface PodRoom {
  /** The height of the column in px; null until it is measured. */
  height: number | null
  /** The width of the creatures on the board beside it in px; null until that board is fitted. */
  card: number | null
}

/** Nothing is measured: a pod outside a column (a test, a stand-alone pod) draws the least pictures. */
export const NO_ROOM: PodRoom = { height: null, card: null }

/** The room of the column a pod stands in, set by the column (`Seats.tsx`). */
export const PodRoomContext = createContext<PodRoom>(NO_ROOM)

/** The room of the pod's column. */
export function usePodRoom(): PodRoom {
  return useContext(PodRoomContext)
}
