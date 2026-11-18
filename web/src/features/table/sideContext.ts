import { createContext, useContext } from 'react'
import type { SideKind } from './layout'
import type { SideTab } from './sideCounts'

/** What the side area shows and what the top bar and the rail say about it. */
export interface SideState {
  kind: SideKind
  /** The window has room for the column, so a rail here is the viewer's fold and can open back into it. */
  foldable: boolean
  /** The tab on show. */
  tab: SideTab
  /** Whether the panel is up: always in a column, on request in a rail or a sheet. */
  open: boolean
  /** Unread lines for the log and the chat; the spells on the stack for the stack. */
  counts: Record<SideTab, number>
  /** What still wants a look, for the one button of a sheet: unread lines and spells on the stack. */
  attention: number
  /** A press on a tab: choose it, and in a rail or sheet open the panel; in a rail the tab on show closes it. */
  activate(tab: SideTab): void
  /** Opens or closes the panel of a rail or a sheet. */
  setOpen(open: boolean): void
  /** The overlay of the table's main area that an open rail panel or sheet is drawn into; null until it is in the page. */
  mount: HTMLElement | null
  /** Hands over the overlay (a ref callback). */
  setMount(element: HTMLElement | null): void
  /**
   * The element the pending decision's choices are drawn into: in the Stack tab of a column, else in the decision sheet
   * over the hand; null while neither is up.
   */
  detail: HTMLElement | null
  /** Hands over the choices' element (a ref callback). */
  setDetail(element: HTMLElement | null): void
  /** Whether the viewer folded (true) or opened (false) the decision sheet; null until they press its handle. */
  sheetFolded: boolean | null
  /** Folds or opens the decision sheet. */
  setSheetFolded(folded: boolean): void
}

/** Provided by `SideProvider`. */
export const SideContext = createContext<SideState | null>(null)

/** The side area's state; throws outside a `SideProvider`. */
export function useSide(): SideState {
  const value = useContext(SideContext)
  if (value === null) throw new Error('useSide must be used inside a SideProvider')
  return value
}
