import { useEffect } from 'react'
import type { RefObject } from 'react'

/** The px of one line of a wheel that counts in lines (Firefox's mouse wheel). */
const LINE_PX = 16

/**
 * How far a wheel turn moves a line that scrolls sideways only, in px, or 0 where it should not: the line does not
 * scroll sideways, it scrolls up and down itself, the turn is sideways already, or the line is at the end it moves to.
 */
export function sidewaysStep(
  line: { scrollLeft: number; scrollWidth: number; clientWidth: number; upDown: boolean },
  wheel: { deltaX: number; deltaY: number; deltaMode: number },
): number {
  const room = line.scrollWidth - line.clientWidth
  if (room <= 1 || line.upDown) return 0
  if (Math.abs(wheel.deltaX) >= Math.abs(wheel.deltaY)) return 0
  const step =
    wheel.deltaMode === 1
      ? wheel.deltaY * LINE_PX
      : wheel.deltaMode === 2
        ? wheel.deltaY * line.clientWidth
        : wheel.deltaY
  if (step < 0 && line.scrollLeft <= 0) return 0
  if (step > 0 && line.scrollLeft >= room - 1) return 0
  return step
}

/**
 * A wheel turned over a line that scrolls sideways only (the hand, a strip of cards) moves the line, as a mouse
 * without a sideways wheel has no other way; at either end the turn passes on to the page. The line is the element
 * `ref` holds from the first render on.
 */
export function useWheelSideways(ref: RefObject<HTMLElement | null>): void {
  useEffect(() => {
    const line = ref.current
    if (line === null) return undefined
    const onWheel = (event: WheelEvent): void => {
      if (event.ctrlKey) return
      const overflow = getComputedStyle(line).overflowY
      const upDown =
        (overflow === 'auto' || overflow === 'scroll') && line.scrollHeight > line.clientHeight + 1
      const step = sidewaysStep(
        { scrollLeft: line.scrollLeft, scrollWidth: line.scrollWidth, clientWidth: line.clientWidth, upDown },
        event,
      )
      if (step === 0) return
      event.preventDefault()
      line.scrollLeft += step
    }
    // Not passive: the turn that moves the line must not scroll anything else as well.
    line.addEventListener('wheel', onWheel, { passive: false })
    return () => line.removeEventListener('wheel', onWheel)
  }, [ref])
}
