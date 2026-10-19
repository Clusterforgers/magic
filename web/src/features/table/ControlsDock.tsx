import { createContext, useContext, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import { createPortal } from 'react-dom'
import { CompactDockContext } from './dockLayout'
import { StopsDock } from './stops/StopsDock'
import { useEdgeFade } from './useEdgeFade'
import { YieldIndicator } from './YieldIndicator'

// `undefined`: no dock around, controls draw where they are. `null`: the dock is there but not in the page yet.
const SlotContext = createContext<HTMLElement | null | undefined>(undefined)
const SetSlotContext = createContext<(element: HTMLElement | null) => void>(() => undefined)

/**
 * Lets the pass controls of a prompt draw inside the controls dock, wherever the prompt itself sits. `compact` is the
 * dock of a phone (`useCompactDock`).
 */
export function ControlsSlotProvider({
  children,
  compact = false,
}: {
  children: ReactNode
  compact?: boolean
}) {
  const [slot, setSlot] = useState<HTMLElement | null>(null)
  return (
    <SetSlotContext value={setSlot}>
      <SlotContext value={slot}>
        <CompactDockContext value={compact}>{children}</CompactDockContext>
      </SlotContext>
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
 * The controls dock at the bottom right, top to bottom in the order of reading: the line that says the server is
 * passing for this seat, the stops (the ladder and Full Control in one line where the dock has the room, else one
 * "Stops" button that opens them), and the pass controls of a Priority prompt ("End turn", the menu of other ways to
 * pass, then the primary button in the corner: "Next: Combat", "Resolve Lightning Bolt"). At most five controls show
 * at any size.
 */
export function ControlsDock({ stopsRow }: { stopsRow: boolean }) {
  const setSlot = useContext(SetSlotContext)
  const dock = useRef<HTMLElement>(null)
  useEdgeFade(dock)
  return (
    <section
      ref={dock}
      aria-label="Pass controls"
      data-region="controls"
      // On a phone the dock is one wrapping line: the Stops button and the menu of passing, then the primary button alone.
      className="fade-scroll flex min-h-0 min-w-0 flex-col items-end justify-end-safe gap-1.5 border-t border-l bg-surface p-2 max-sm:flex-row max-sm:flex-wrap max-sm:content-end max-sm:justify-end"
    >
      <div data-slot="yield-slot" className="max-w-full empty:hidden max-sm:basis-full">
        <YieldIndicator />
      </div>
      <div data-slot="stops-slot" className="max-w-full empty:hidden">
        <StopsDock row={stopsRow} />
      </div>
      <div
        ref={setSlot}
        data-slot="pass-slot"
        className="flex w-full flex-wrap items-center justify-end gap-2 empty:hidden max-sm:contents"
      />
    </section>
  )
}
