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
import { DockFormContext, DockWideContext, useDockForm } from './dockLayout'
import type { DockForm } from './dockLayout'
import { StopsDock } from './stops/StopsDock'
import { GoBackControl } from './GoBackControl'
import { TakeBackControls } from './TakeBackControls'
import { useEdgeFade } from './useEdgeFade'
import { YieldIndicator } from './YieldIndicator'

// `undefined`: no dock around, controls draw where they are. `null`: the dock is there but not in the page yet.
const SlotContext = createContext<HTMLElement | null | undefined>(undefined)
const SetSlotContext = createContext<(element: HTMLElement | null) => void>(() => undefined)

/**
 * Lets the pass controls of a prompt draw inside the controls dock, wherever the prompt itself sits. `form` says how
 * much the dock writes (`DockForm`) and `wide` that it is at its widest (`DockWideContext`).
 */
export function ControlsSlotProvider({
  children,
  form = 'full',
  wide = true,
}: {
  children: ReactNode
  form?: DockForm
  wide?: boolean
}) {
  const [slot, setSlot] = useState<HTMLElement | null>(null)
  return (
    <SetSlotContext value={setSlot}>
      <SlotContext value={slot}>
        <DockFormContext value={form}>
          <DockWideContext value={wide}>{children}</DockWideContext>
        </DockFormContext>
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
 * pass, "Resolve all" in place of "End turn" over a stack of two or more, then the primary button in the corner: "Next: Combat", "Resolve Lightning Bolt"). At most five controls show
 * at any size. The pass controls never scroll or leave the dock: what is above them gives way (`useDockFit`), first by
 * folding the stops into their button and setting the take-backs and stops side by side, last by scrolling itself.
 * Off a phone the dock's body grows upwards past its cell for what the decision and the pass controls need.
 */
export function ControlsDock({ stopsRow }: { stopsRow: boolean }) {
  const phone = useContext(HomeSlotsContext)?.phone === true
  // A phone's dock is other elements: it is mounted anew, so what watches the dock's parts watches the new ones.
  return <Dock key={phone ? 'phone' : 'desk'} stopsRow={stopsRow} />
}

function Dock({ stopsRow }: { stopsRow: boolean }) {
  const setSlot = useContext(SetSlotContext)
  const homes = useContext(HomeSlotsContext)
  const dock = useRef<HTMLElement>(null)
  const upper = useRef<HTMLDivElement>(null)
  useEdgeFade(upper)
  const phone = homes?.phone === true
  const { tight, least: upperLeast } = useDockFit(dock, upper)
  const { home } = useDecisionHome()
  const form = useDockForm()
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
  const upperZone = (
    <div
      ref={upper}
      data-slot="dock-upper"
      hidden={phone && asking}
      className={cn(
        'fade-scroll flex min-h-0 max-w-full shrink gap-1.5 empty:hidden',
        tight || phone ? 'flex-row flex-wrap content-end items-center justify-end' : 'flex-col items-end',
        // Off a phone it stands at the foot of the room the decision and the pass controls leave.
        !phone && 'absolute right-0 bottom-0 max-h-full',
      )}
    >
      {/* On a phone the take-backs give way on their line, so the stops and the Stack chip stand beside them. */}
      <div
        data-slot="take-back-slot"
        className={cn('max-w-full empty:hidden', phone && 'min-w-0 flex-[1_1_8rem] [&_button]:max-w-full')}
      >
        <TakeBackControls />
      </div>
      <div
        data-slot="go-back-slot"
        className={cn('max-w-full empty:hidden', phone && 'min-w-0 flex-[1_1_8rem] [&_button]:max-w-full')}
      >
        <GoBackControl />
      </div>
      <div data-slot="yield-slot" className="max-w-full empty:hidden">
        <YieldIndicator />
      </div>
      {/* A question answered here takes the stops' row: its home holds their folded button meanwhile. */}
      <div
        data-slot="stops-slot"
        className="flex max-w-full flex-wrap items-center justify-end gap-x-1.5 empty:hidden"
      >
        {folded ? null : (
          <StopsDock row={stopsRow && !tight} tuck={tight && form === 'short' ? 'skip' : undefined} />
        )}
      </div>
      {phone ? <div ref={homes?.setBarLine} data-slot="bar-stack-slot" className="contents" /> : null}
    </div>
  )
  const lower = (
    <>
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
    </>
  )
  // On a phone the dock is the decision bar under the hand: the take-backs and stops on a line, the decision, the
  // pass controls; one height in every prompt and between prompts.
  if (phone)
    return (
      <section
        ref={dock}
        aria-label="Pass controls"
        data-region="controls"
        data-keep-clear="answers"
        data-tight={tight || undefined}
        data-phone
        className="flex min-h-0 min-w-0 flex-col items-stretch justify-end gap-1 overflow-hidden border-t px-2.5 py-1.5 dock"
      >
        {upperZone}
        {lower}
      </section>
    )
  return (
    // The cell keeps the hand row's height in every prompt. Its body is at least that high and grows upwards over the
    // corner of your board when the decision and the pass controls need more, so a short window cuts nothing off.
    <section
      ref={dock}
      aria-label="Pass controls"
      data-region="controls"
      data-keep-clear="answers"
      data-tight={tight || undefined}
      data-dock-form={form}
      className="relative min-h-0 min-w-0"
    >
      <div
        data-slot="dock-body"
        className="absolute inset-x-0 bottom-0 flex max-h-[70dvh] min-h-full flex-col items-end justify-end gap-1 overflow-hidden border-l px-2 py-1.5 dock"
      >
        <div
          data-slot="dock-upper-room"
          style={{ minHeight: upperLeast }}
          className="relative min-h-0 w-full flex-[1_1_0]"
        >
          {upperZone}
        </div>
        {lower}
      </div>
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

/** The most the folded controls above the pass controls hold the dock's body open, in px: two rows of them. */
const UPPER_MOST_PX = 76

/**
 * `tight` while the controls above the pass controls do not fit the dock's height in their full form: then they fold
 * (`ControlsDock`). It unfolds again when the dock grows past the height it folded at, or what it holds changes.
 * `least` is the room the folded controls keep in the dock's body, so they push it up before they scroll.
 */
function useDockFit(
  dock: RefObject<HTMLElement | null>,
  upper: RefObject<HTMLElement | null>,
): { tight: boolean; least: number } {
  const [tightAt, setTightAt] = useState<number | null>(null)
  const [least, setLeast] = useState(0)
  useLayoutEffect(() => {
    const box = dock.current
    const zone = upper.current
    if (box === null || zone === null) return undefined
    const measure = (): void => {
      setTightAt((held) => {
        if (held !== null) return box.clientHeight > held + 1 ? null : held
        return zone.scrollHeight > zone.clientHeight + 1 ? box.clientHeight : null
      })
      setLeast(Math.min(zone.scrollHeight, UPPER_MOST_PX))
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
  const tight = tightAt !== null
  return { tight, least: tight ? least : 0 }
}
