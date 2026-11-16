import { createContext, useContext, useLayoutEffect, useRef, useState } from 'react'
import type { ReactNode, RefObject } from 'react'
import { createPortal } from 'react-dom'
import { cn } from '@/lib/utils'
import { DockFormContext } from './dockLayout'
import type { DockForm } from './dockLayout'
import { StopsDock } from './stops/StopsDock'
import { TakeBackControls } from './TakeBackControls'
import { useEdgeFade } from './useEdgeFade'
import { YieldIndicator } from './YieldIndicator'

// `undefined`: no dock around, controls draw where they are. `null`: the dock is there but not in the page yet.
const SlotContext = createContext<HTMLElement | null | undefined>(undefined)
const SetSlotContext = createContext<(element: HTMLElement | null) => void>(() => undefined)

/**
 * Lets the pass controls of a prompt draw inside the controls dock, wherever the prompt itself sits. `form` says how
 * much the dock writes (`DockForm`).
 */
export function ControlsSlotProvider({ children, form = 'full' }: { children: ReactNode; form?: DockForm }) {
  const [slot, setSlot] = useState<HTMLElement | null>(null)
  return (
    <SetSlotContext value={setSlot}>
      <SlotContext value={slot}>
        <DockFormContext value={form}>{children}</DockFormContext>
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
 * The controls dock at the bottom right, top to bottom in the order of reading: take-backs (the seat's "Take back"
 * button, the table's request and its notice), the line that says the server is passing for this seat, the stops (the ladder and Full Control in one line where the dock has the room, else one
 * "Stops" button that opens them), and the pass controls of a Priority prompt ("End turn", the menu of other ways to
 * pass, then the primary button in the corner: "Next: Combat", "Resolve Lightning Bolt"). At most five controls show
 * at any size. The pass controls never scroll or leave the dock: what is above them gives way (`useDockFit`), first by
 * folding the stops into their button and setting the take-backs and stops side by side, last by scrolling itself.
 */
export function ControlsDock({ stopsRow }: { stopsRow: boolean }) {
  const setSlot = useContext(SetSlotContext)
  const dock = useRef<HTMLElement>(null)
  const upper = useRef<HTMLDivElement>(null)
  useEdgeFade(upper)
  const tight = useDockFit(dock, upper)
  return (
    <section
      ref={dock}
      aria-label="Pass controls"
      data-region="controls"
      data-keep-clear=""
      data-tight={tight || undefined}
      // On a phone the dock is one wrapping line: the Stops button and the menu of passing, then the primary button alone.
      className="flex min-h-0 min-w-0 flex-col items-end justify-end gap-1.5 overflow-hidden dock border-l px-2 py-1.5 max-sm:flex-row max-sm:flex-wrap max-sm:content-end max-sm:justify-end"
    >
      <div
        ref={upper}
        data-slot="dock-upper"
        className={cn(
          'fade-scroll flex min-h-0 max-w-full shrink gap-1.5 empty:hidden max-sm:contents',
          tight ? 'flex-row flex-wrap content-end items-center justify-end' : 'flex-col items-end',
        )}
      >
        <div data-slot="take-back-slot" className="max-w-full empty:hidden max-sm:basis-full">
          <TakeBackControls />
        </div>
        <div data-slot="yield-slot" className="max-w-full empty:hidden max-sm:basis-full">
          <YieldIndicator />
        </div>
        <div data-slot="stops-slot" className="max-w-full empty:hidden">
          <StopsDock row={stopsRow && !tight} />
        </div>
      </div>
      <div
        ref={setSlot}
        data-slot="pass-slot"
        className="flex w-full shrink-0 flex-wrap items-center justify-end gap-2 empty:hidden max-sm:contents"
      />
    </section>
  )
}

/**
 * True while the controls above the pass controls do not fit the dock's height in their full form: then they fold
 * (`ControlsDock`). It unfolds again when the dock grows past the height it folded at, or what it holds changes.
 */
function useDockFit(dock: RefObject<HTMLElement | null>, upper: RefObject<HTMLElement | null>): boolean {
  const [tightAt, setTightAt] = useState<number | null>(null)
  useLayoutEffect(() => {
    const box = dock.current
    const zone = upper.current
    if (box === null || zone === null) return undefined
    const measure = (): void => {
      setTightAt((held) => {
        if (held !== null) return box.clientHeight > held + 1 ? null : held
        return zone.scrollHeight > zone.clientHeight + 1 ? box.clientHeight : null
      })
    }
    measure()
    const sizes = typeof ResizeObserver === 'function' ? new ResizeObserver(measure) : null
    sizes?.observe(box)
    sizes?.observe(zone)
    // A take-back or yield line that comes or goes changes what the full form needs: try it again.
    const content =
      typeof MutationObserver === 'function' ? new MutationObserver(() => setTightAt(null)) : null
    for (const slot of zone.querySelectorAll('[data-slot="take-back-slot"], [data-slot="yield-slot"]'))
      content?.observe(slot, { childList: true })
    return () => {
      sizes?.disconnect()
      content?.disconnect()
    }
  }, [dock, upper])
  return tightAt !== null
}
