import { useEffect, useId, useLayoutEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { Button } from '@/components/ui/button'
import { placeCard, spotlight } from './tourPlace'
import type { Box } from './tourPlace'

/** One stop of a tour: what it explains and the part of the page it points at. */
export interface TourStop {
  id: string
  title: string
  text: string
  /** Selectors of the part to light up, the first one in view wins; none in view centres the card. */
  targets: readonly string[]
}

const same = (a: Box | null, b: Box | null): boolean =>
  a === b ||
  (a !== null &&
    b !== null &&
    a.left === b.left &&
    a.top === b.top &&
    a.width === b.width &&
    a.height === b.height)

/** The window box of the first of `targets` that is drawn and in the window, or null. */
function findTarget(targets: readonly string[]): Box | null {
  for (const selector of targets) {
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

/** Follows the stop's target as the page moves under it, one look a frame. */
function useTarget(stop: TourStop | undefined): Box | null {
  const [box, setBox] = useState<Box | null>(null)
  useEffect(() => {
    if (stop === undefined) return
    let frame = 0
    const look = (): void => {
      const next = findTarget(stop.targets)
      setBox((was) => (same(was, next) ? was : next))
      frame = window.requestAnimationFrame(look)
    }
    look()
    return () => window.cancelAnimationFrame(frame)
  }, [stop])
  return box
}

/** The window dimmed around the lit box, in four bands, and a ring on the box's edge. */
function Shade({ lit, passThrough }: { lit: Box; passThrough: boolean }) {
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
        <div
          key={index}
          aria-hidden="true"
          className={passThrough ? 'absolute bg-scrim/50' : 'absolute bg-scrim'}
          style={band}
        />
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

/** Arrow keys move through the tour and Escape ends it; `forward` absent leaves the right arrow alone. */
function useTourKeys(
  on: boolean,
  forward: (() => void) | undefined,
  back: (() => void) | undefined,
  end: () => void,
): void {
  useEffect(() => {
    if (!on) return
    const press = (event: KeyboardEvent): void => {
      if (event.key === 'ArrowRight' && forward !== undefined) forward()
      else if (event.key === 'ArrowLeft' && back !== undefined) back()
      else if (event.key === 'Escape') end()
      else return
      event.preventDefault()
      event.stopPropagation()
    }
    window.addEventListener('keydown', press, true)
    return () => window.removeEventListener('keydown', press, true)
  }, [on, forward, back, end])
}

/** What `TourCard` takes. */
export interface TourCardProps {
  stop: TourStop
  /** The stop's place, from 0, and how many there are. */
  index: number
  count: number
  /** Back, shown from the second stop. */
  onBack?: () => void
  /** Next; without it the stop waits for the player and says so in `waiting`. */
  onNext?: () => void
  /** The Next button's words: "Next" by default. */
  nextLabel?: string
  /** What the stop waits for, in place of Next. */
  waiting?: string
  /** Ends the tour. */
  onSkip: () => void
  /** The Skip button's words. */
  skipLabel: string
  /**
   * Whether the page under the tour stays usable: the shade takes no pointer and the card is no modal dialog, so the
   * player can do what the stop asks. Off, the page under it takes no presses.
   */
  passThrough?: boolean
  /** The root's `data-slot`. */
  slot?: string
  /** Keys: arrows and Escape; off where the page needs them. */
  keys?: boolean
}

/**
 * A tour's card: the stop's part of the page lit with the rest shaded, and a card beside it that says what it is, with
 * Back, Next (or what it waits for) and Skip, placed by `placeCard`. Shared by the table tour, the guided lesson and
 * the page tours.
 */
export function TourCard({
  stop,
  index,
  count,
  onBack,
  onNext,
  nextLabel = 'Next',
  waiting,
  onSkip,
  skipLabel,
  passThrough = false,
  slot = 'tour',
  keys = true,
}: TourCardProps) {
  const target = useTarget(stop)
  const card = useRef<HTMLDivElement>(null)
  const next = useRef<HTMLButtonElement>(null)
  const [size, setSize] = useState({ width: 0, height: 0 })
  const titleId = useId()
  const textId = useId()
  useTourKeys(keys, onNext, index > 0 ? onBack : undefined, onSkip)

  useEffect(() => {
    if (!passThrough) next.current?.focus({ preventScroll: true })
  }, [index, passThrough])

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
  }, [stop])

  const view = { width: window.innerWidth, height: window.innerHeight }
  const lit = target === null ? null : spotlight(target, view)
  const place = placeCard(lit, size, view)
  return createPortal(
    <div
      data-slot={slot}
      data-step={stop.id}
      className={passThrough ? 'pointer-events-none fixed inset-0 z-[70]' : 'fixed inset-0 z-[70]'}
    >
      {lit === null ? (
        passThrough ? null : (
          <div aria-hidden="true" className="absolute inset-0 bg-scrim" />
        )
      ) : (
        <Shade lit={lit} passThrough={passThrough} />
      )}
      <div
        ref={card}
        role="dialog"
        aria-modal={passThrough ? undefined : true}
        aria-labelledby={titleId}
        aria-describedby={textId}
        data-slot="tour-card"
        data-side={place.side}
        style={{ left: place.left, top: place.top, opacity: size.width === 0 ? 0 : undefined }}
        className="pointer-events-auto absolute grid w-[min(22rem,calc(100vw-2rem))] gap-3 rounded-xl border bg-popover p-4 text-popover-foreground shadow-e3"
      >
        <div className="grid gap-1">
          <p className="text-xs text-muted-foreground tabular-nums">
            Step {index + 1} of {count}
          </p>
          <h2 id={titleId} className="font-heading text-base font-semibold">
            {stop.title}
          </h2>
          <p id={textId} className="text-sm">
            {stop.text}
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Button type="button" variant="ghost" size="sm" onClick={onSkip}>
            {skipLabel}
          </Button>
          <span className="flex-1" />
          {index > 0 && onBack !== undefined ? (
            <Button type="button" variant="outline" size="sm" onClick={onBack}>
              Back
            </Button>
          ) : null}
          {onNext !== undefined ? (
            <Button ref={next} type="button" size="sm" onClick={onNext}>
              {nextLabel}
            </Button>
          ) : (
            <p data-slot="tour-waiting" role="status" className="flex items-center gap-2 text-xs font-medium">
              <span aria-hidden="true" className="size-2 rounded-full bg-primary motion-safe:animate-pulse" />
              {waiting ?? 'Waiting for you'}
            </p>
          )}
        </div>
      </div>
    </div>,
    document.body,
  )
}
