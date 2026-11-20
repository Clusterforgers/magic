import { createContext, useContext, useLayoutEffect, useRef, useState } from 'react'
import type { ReactNode, RefObject } from 'react'
import { createPortal } from 'react-dom'
import { ArrowUpRight } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import { useAsker } from './prompt/detail/useAsker'
import { focusAnswers } from './prompt/homeKeys'
import { KeyHint } from './prompt/KeyHint'
import { ownDecision } from './prompt/ownDecision'
import { HomeSlotsContext, useDecisionHome } from './prompt/useDecisionHome'
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
  const homes = useContext(HomeSlotsContext)
  const dock = useRef<HTMLElement>(null)
  const upper = useRef<HTMLDivElement>(null)
  useEdgeFade(upper)
  const tight = useDockFit(dock, upper)
  const { home } = useDecisionHome()
  // A question other than priority answered here takes the stops' row: they fold into their button meanwhile.
  const asking = usePlay((state) => {
    const own = ownDecision(state)
    // The opening hand answers in its centred panel off a phone: the dock keeps its stops then.
    return (
      own !== null &&
      !('Priority' in own.decision.kind) &&
      !('Mulligan' in own.decision.kind && homes?.phone !== true)
    )
  })
  const folded = asking && home !== 'panel'
  const phone = homes?.phone === true
  return (
    <section
      ref={dock}
      aria-label="Pass controls"
      data-region="controls"
      data-keep-clear="answers"
      data-tight={tight || undefined}
      data-phone={phone || undefined}
      // On a phone the dock is the decision bar under the hand: the take-backs and stops on a line, the decision, the
      // pass controls; one height in every prompt and between prompts.
      className={cn(
        'flex min-h-0 min-w-0 flex-col gap-1 overflow-hidden dock',
        phone
          ? 'items-stretch justify-end border-t px-2.5 py-1.5'
          : 'items-end justify-end border-l px-2 py-1.5',
      )}
    >
      <div
        ref={upper}
        data-slot="dock-upper"
        hidden={phone && asking}
        className={cn(
          'fade-scroll flex min-h-0 max-w-full shrink gap-1.5 empty:hidden',
          tight || phone ? 'flex-row flex-wrap content-end items-center justify-end' : 'flex-col items-end',
        )}
      >
        <div data-slot="take-back-slot" className="max-w-full empty:hidden">
          <TakeBackControls />
        </div>
        <div data-slot="yield-slot" className="max-w-full empty:hidden">
          <YieldIndicator />
        </div>
        {/* A question answered here takes the stops' row: its home holds their folded button meanwhile. */}
        <div data-slot="stops-slot" className="max-w-full empty:hidden">
          {folded ? null : <StopsDock row={stopsRow && !tight} />}
        </div>
      </div>
      {home === 'panel' ? <AnswerJump /> : null}
      <div
        ref={homes === null ? undefined : phone ? homes.setBar : homes.setDock}
        data-slot={phone ? 'decision-bar' : 'decision-dock'}
        className={cn('w-full min-w-0 empty:hidden', phone ? 'min-h-0 flex-1 overflow-hidden' : 'shrink-0')}
      />
      <div
        ref={setSlot}
        data-slot="pass-slot"
        className={cn(
          'flex w-full shrink-0 items-center justify-end gap-2 empty:hidden',
          phone ? 'flex-nowrap' : 'flex-wrap',
        )}
      />
    </section>
  )
}

/** While the Stack panel is the home, the dock says where to answer and takes the focus there (A). */
function AnswerJump() {
  const asker = useAsker()
  const name = asker === null || asker.shown === '' ? 'the asking card' : asker.shown
  return (
    <Button
      variant="outline"
      data-slot="answer-jump"
      aria-keyshortcuts="A"
      onClick={() => void focusAnswers()}
      className="h-auto min-h-11 w-full min-w-0 shrink-0 justify-end gap-1 py-1 font-normal whitespace-normal"
    >
      <span className="min-w-0 text-right leading-4 line-clamp-2">
        Answer under {name} in the Stack panel
      </span>
      <KeyHint>A</KeyHint>
      <ArrowUpRight aria-hidden="true" className="size-4 shrink-0" />
    </Button>
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
