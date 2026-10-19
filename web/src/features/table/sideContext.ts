import { createContext, useContext } from 'react'
import type { SideKind } from './layout'
import type { SideTab } from './sideCounts'

/** What the side area shows and what the top bar and the rail say about it. */
export interface SideState {
  kind: SideKind
  /** The tab on show. */
  tab: SideTab
  /** Whether the panel is up: always in a column, on request in a rail or a sheet. */
  open: boolean
  /** Unread lines for the log and the chat; the spells on the stack for the stack. */
  counts: Record<SideTab, number>
  /** What still wants a look, for the one button of a sheet: unread lines and spells on the stack. */
  attention: number
  /** A press on a tab: choose it, and in a rail or sheet open the panel, or close it if that tab is the one on show. */
  activate(tab: SideTab): void
  /** Opens or closes the panel of a rail or a sheet. */
  setOpen(open: boolean): void
}

/** Provided by `SideProvider`. */
export const SideContext = createContext<SideState | null>(null)

/** The side area's state; throws outside a `SideProvider`. */
export function useSide(): SideState {
  const value = useContext(SideContext)
  if (value === null) throw new Error('useSide must be used inside a SideProvider')
  return value
}
