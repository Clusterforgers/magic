/** The table's decision homes as elements, and the home of the decision on hand (`home.ts` joined with the table). */
import { createContext, useContext, useMemo, useState } from 'react'
import type { ReactNode } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { SideContext } from '../sideContext'
import { kindKey } from './detail/asking'
import { decisionHome } from './home'
import type { Home } from './home'
import { ownDecision } from './ownDecision'

/** The elements a decision is drawn into, one for each home, and whether the table is a phone's. */
export interface HomeSlots {
  phone: boolean
  panel: HTMLElement | null
  dock: HTMLElement | null
  bar: HTMLElement | null
  setPanel(element: HTMLElement | null): void
  setDock(element: HTMLElement | null): void
  setBar(element: HTMLElement | null): void
}

/** Provided by `HomeSlotsProvider` at a table; none elsewhere, where a prompt draws in place. */
export const HomeSlotsContext = createContext<HomeSlots | null>(null)

/** Holds the elements of the homes a table draws (`decision-panel`, `decision-dock`, `decision-bar`). */
export function HomeSlotsProvider({ phone, children }: { phone: boolean; children: ReactNode }) {
  const [panel, setPanel] = useState<HTMLElement | null>(null)
  const [dock, setDock] = useState<HTMLElement | null>(null)
  const [bar, setBar] = useState<HTMLElement | null>(null)
  const value = useMemo<HomeSlots>(
    () => ({ phone, panel, dock, bar, setPanel, setDock, setBar }),
    [phone, panel, dock, bar],
  )
  return <HomeSlotsContext value={value}>{children}</HomeSlotsContext>
}

/** The home of a decision of `kind` at this table; null outside a table. */
export function useHomeOf(kind: string): Home | null {
  const slots = useContext(HomeSlotsContext)
  const side = useContext(SideContext)
  if (slots === null) return null
  return decisionHome(kind, side === null ? null : { kind: side.kind, open: side.open, tab: side.tab }, {
    phone: slots.phone,
  })
}

/** The home of the decision this seat must answer: which one and its element (null before it is in the page). */
export function useDecisionHome(): { home: Home | null; element: HTMLElement | null } {
  const kind = usePlay((state) => {
    const own = ownDecision(state)
    return own === null ? '' : kindKey(own.decision.kind)
  })
  const home = useHomeOf(kind)
  const slots = useContext(HomeSlotsContext)
  return { home, element: home === null || slots === null ? null : slots[home] }
}
