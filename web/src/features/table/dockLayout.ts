import { createContext, useContext } from 'react'

/**
 * How much the controls dock writes. `full` has the room for the key hints and the whole words of the primary button
 * ("Resolve Lightning Bolt"); `short` leaves the hints out and writes "Resolve" and "Next: Attackers", so the hand dock
 * beside it keeps its cards in one row; `compact`, the dock of a phone, has room for no more than the stops and the
 * primary button, and "End turn" waits in the menu.
 */
export type DockForm = 'full' | 'short' | 'compact'

/** The form of the dock the pass controls draw into; `full` where there is no table around. */
export const DockFormContext = createContext<DockForm>('full')

/** The form of the controls dock (`DockFormContext`). */
export function useDockForm(): DockForm {
  return useContext(DockFormContext)
}

/** Whether the dock is at its widest, with the stops in a row of their own (`planTable().stopsRow`); true where there is no table around. */
export const DockWideContext = createContext(true)

/** True in the widest dock (`DockWideContext`): a question's line has room for the stops' switches beside their button. */
export function useDockWide(): boolean {
  return useContext(DockWideContext)
}

/** True in the dock of a phone: the pass controls leave "End turn" to their menu, so the primary button keeps the corner. */
export function useCompactDock(): boolean {
  return useDockForm() === 'compact'
}
