import { createContext, useContext } from 'react'

/** Whether the controls dock is the narrow one of a phone, which has room for no more than the stops and the pass button. */
export const CompactDockContext = createContext(false)

/** True in the dock of a phone: the pass controls leave "End turn" to their menu, so the primary button keeps the corner. */
export function useCompactDock(): boolean {
  return useContext(CompactDockContext)
}
