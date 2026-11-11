import { useContext, useEffect, useId, useLayoutEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { useStore } from 'zustand'
import { Button } from '@/components/ui/button'
import { usePlay } from '@/features/game/gameContext'
import { useSide } from '../sideContext'
import { TableOptionsContext, TOUR_BY_DEFAULT } from '../tableOptions'
import { placeCard, spotlight } from './tourPlace'
import type { Box } from './tourPlace'
import { endTour, moveTour, startTour, tourSeen, tourStore } from './tourStore'
import { TOUR } from './tourSteps'
import type { TourStep } from './tourSteps'

const same = (a: Box | null, b: Box | null): boolean =>
  a === b ||
  (a !== null &&
    b !== null &&
    a.left === b.left &&
    a.top === b.top &&
    a.width === b.width &&
    a.height === b.height)

/** The window box of the first of the step's targets that is drawn and in the window, or null. */
function findTarget(step: TourStep): Box | null {
  for (const selector of step.targets) {
    for (const element of document.querySelectorAll(selector)) {
      const rect = element.getBoundingClientRect()
      const visible =
        rect.width > 0 &&
        rect.height > 0 &&
        rect.bottom > 0 &&
        rect.right > 0 &&
        rect.top < window.innerHeight &&
        rect.left < window.innerWidth
      if (visible) return { left: rect.left, top: rect.top, width: rect.width, height: rect.height }
    }
  }
  return null
}

/** Follows the step's target as the table moves under it, one look a frame. */
function useTarget(step: TourStep | undefined): Box | null {
  const [box, setBox] = useState<Box | null>(null)
  useEffect(() => {
    if (step === undefined) return
    let frame = 0
    const look = (): void => {
      const next = findTarget(step)
      setBox((was) => (same(was, next) ? was : next))
      frame = window.requestAnimationFrame(look)
    }
    look()
    return () => window.cancelAnimationFrame(frame)
  }, [step])
  return box
}

/** Brings up the side tab a step points at, and closes a rail or sheet panel the tour opened when it ends. */
function useStepTab(step: TourStep | undefined): void {
  const side = useSide()
  const opened = useRef(false)
  const { kind, tab, open, activate, setOpen } = side
  useEffect(() => {
    if (step?.tab === undefined) return
    if (tab === step.tab && open) return
    if (!open && kind !== 'column') opened.current = true
    if (tab === step.tab) setOpen(true)
    else activate(step.tab)
  }, [step, kind, tab, open, activate, setOpen])
  useEffect(() => {
    if (step !== undefined || !opened.current) return
    opened.current = false
    if (kind !== 'column') setOpen(false)
  }, [step, kind, setOpen])
}

/** The window dimmed around the lit box, in four bands, and a ring on the box's edge. */
function Shade({ lit }: { lit: Box }) {
  const right = lit.left + lit.width
  const bottom = lit.top + lit.height
  const bands = [
    { left: 0, top: 0, right: 0, height: lit.top },
    { left: 0, top: bottom, right: 0, bottom: 0 },
    { left: 0, top: lit.top, width: lit.left, height: lit.height },
    { left: right, top: lit.top, right: 0, height: lit.height },
  ]
  return (
    <>
      {bands.map((band, index) => (
        <div key={index} aria-hidden="true" className="absolute bg-scrim" style={band} />
      ))}
      <div
        aria-hidden="true"
        data-slot="tour-spotlight"
        className="absolute rounded-md outline-2 outline-ring"
        style={{ left: lit.left, top: lit.top, width: lit.width, height: lit.height, outlineStyle: 'solid' }}
      />
    </>
  )
}

/** Arrow keys move through the tour and Escape ends it. */
function useTourKeys(on: boolean): void {
  useEffect(() => {
    if (!on) return
    const press = (event: KeyboardEvent): void => {
      if (event.key === 'ArrowRight') moveTour(1)
      else if (event.key === 'ArrowLeft') moveTour(-1)
      else if (event.key === 'Escape') endTour()
      else return
      event.preventDefault()
      event.stopPropagation()
    }
    window.addEventListener('keydown', press, true)
    return () => window.removeEventListener('keydown', press, true)
  }, [on])
}

/**
 * The table tour for a new player: one part of the table lit at a time with a card that says what it is for, Back, Next
 * and Skip. It starts by itself the first time a seat sits at a table in this browser (`tourSeen`) and again from the
 * help menu (`TableOptions.tour`). While it is up the table under it takes no presses.
 */
export function TableTour() {
  const index = useStore(tourStore, (state) => state.step)
  const seated = usePlay((state) => state.seat !== null)
  const step = index === null ? undefined : TOUR[index]
  const target = useTarget(step)
  const card = useRef<HTMLDivElement>(null)
  const next = useRef<HTMLButtonElement>(null)
  const [size, setSize] = useState({ width: 0, height: 0 })
  const titleId = useId()
  const textId = useId()
  useStepTab(step)
  useTourKeys(step !== undefined)

  const auto = useContext(TableOptionsContext).tour ?? TOUR_BY_DEFAULT
  useEffect(() => {
    // A browser under automation (end-to-end tests, screenshots) is no new player.
    if (auto && seated && !navigator.webdriver && !tourSeen() && tourStore.getState().step === null)
      startTour()
  }, [auto, seated])

  useEffect(() => {
    if (index !== null) next.current?.focus({ preventScroll: true })
  }, [index])

  useLayoutEffect(() => {
    const element = card.current
    if (element === null) return
    const measure = (): void =>
      setSize((was) =>
        was.width === element.offsetWidth && was.height === element.offsetHeight
          ? was
          : { width: element.offsetWidth, height: element.offsetHeight },
      )
    measure()
    if (typeof ResizeObserver !== 'function') return
    // Measured on the next frame, so a new place for the card is no resize inside this observer round.
    let frame = 0
    const observer = new ResizeObserver(() => {
      window.cancelAnimationFrame(frame)
      frame = window.requestAnimationFrame(measure)
    })
    observer.observe(element)
    return () => {
      window.cancelAnimationFrame(frame)
      observer.disconnect()
    }
  }, [step])

  if (step === undefined || index === null) return null
  const view = { width: window.innerWidth, height: window.innerHeight }
  const lit = target === null ? null : spotlight(target, view)
  const place = placeCard(lit, size, view)
  const last = index === TOUR.length - 1
  return createPortal(
    <div data-slot="table-tour" data-step={step.id} className="fixed inset-0 z-[70]">
      {lit === null ? <div aria-hidden="true" className="absolute inset-0 bg-scrim" /> : <Shade lit={lit} />}
      <div
        ref={card}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={textId}
        data-slot="tour-card"
        data-side={place.side}
        style={{ left: place.left, top: place.top, opacity: size.width === 0 ? 0 : undefined }}
        className="absolute grid w-[min(22rem,calc(100vw-2rem))] gap-3 rounded-xl border bg-popover p-4 text-popover-foreground shadow-e3"
      >
        <div className="grid gap-1">
          <p className="text-xs text-muted-foreground tabular-nums">
            Step {index + 1} of {TOUR.length}
          </p>
          <h2 id={titleId} className="font-heading text-base font-semibold">
            {step.title}
          </h2>
          <p id={textId} className="text-sm">
            {step.text}
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Button type="button" variant="ghost" size="sm" onClick={endTour}>
            Skip tour
          </Button>
          <span className="flex-1" />
          {index > 0 ? (
            <Button type="button" variant="outline" size="sm" onClick={() => moveTour(-1)}>
              Back
            </Button>
          ) : null}
          <Button ref={next} type="button" size="sm" onClick={() => moveTour(1)}>
            {last ? 'Done' : 'Next'}
          </Button>
        </div>
      </div>
    </div>,
    document.body,
  )
}
