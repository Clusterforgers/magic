import { createContext, useContext, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import { createPortal } from 'react-dom'
import { StopsBar } from './stops/StopsBar'
import { useEdgeFade } from './useEdgeFade'
import { YieldIndicator } from './YieldIndicator'

// `undefined`: no dock around, controls draw where they are. `null`: the dock is there but not in the page yet.
const SlotContext = createContext<HTMLElement | null | undefined>(undefined)
const SetSlotContext = createContext<(element: HTMLElement | null) => void>(() => undefined)

/** Lets the pass controls of a prompt draw inside the controls dock, wherever the prompt itself sits. */
export function ControlsSlotProvider({ children }: { children: ReactNode }) {
  const [slot, setSlot] = useState<HTMLElement | null>(null)
  return (
    <SetSlotContext value={setSlot}>
      <SlotContext value={slot}>{children}</SlotContext>
    </SetSlotContext>
  )
}

/**
 * Draws `children` in the controls dock when the table has one, else where it is called. The children stay part of
 * the prompt (its state, its keys, its context); only their place in the page is the dock's.
 */
export function InControlsDock({ children }: { children: ReactNode }) {
  const slot = useContext(SlotContext)
  if (slot === undefined) return <>{children}</>
  return slot === null ? null : createPortal(children, slot)
}

/**
 * The controls dock at the bottom right: where the pass controls of a Priority prompt are drawn ("Next: Combat", "End
 * turn", the menu of other ways to pass), the line that says the server is passing for this seat, and, when the dock is
 * tall enough, the stop ladder with Full Control. A lower dock leaves the last two to the table menu.
 */
export function ControlsDock({ stops }: { stops: boolean }) {
  const setSlot = useContext(SetSlotContext)
  const dock = useRef<HTMLElement>(null)
  useEdgeFade(dock)
  return (
    <section
      ref={dock}
      aria-label="Pass controls"
      data-region="controls"
      className="fade-scroll flex min-h-0 min-w-0 flex-col items-end gap-2 border-t border-l bg-surface p-2"
    >
      <YieldIndicator />
      <div
        ref={setSlot}
        data-slot="pass-slot"
        className="flex w-full flex-wrap items-center justify-end gap-2 empty:hidden"
      />
      {stops ? <StopsBar /> : null}
    </section>
  )
}
