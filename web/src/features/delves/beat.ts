/**
 * How a Delves choice answers a press (docs/ui-design.md, Feel): the chosen card beats (a squash, a small overshoot and a
 * settle) in the frame it is pressed, before the server answers, and a count that changes (votes, gold) pops into its
 * new value. Motion only where it is welcome; under reduced motion the marks the choice already carries stand alone.
 */
import { useState } from 'react'
import { prefersReducedMotion } from '@/hooks/useReducedMotion'

/** How long a pressed choice beats, in ms. */
export const BEAT_MS = 260

/** Lets `element` (the chosen card) beat once; a second press beats again. */
export function beat(element: Element | null): void {
  if (element === null || prefersReducedMotion() || typeof element.animate !== 'function') return
  element.animate(
    [
      { transform: 'scale(1)' },
      { transform: 'scale(0.96)', offset: 0.3 },
      { transform: 'scale(1.03)', offset: 0.65 },
      { transform: 'scale(1)' },
    ],
    { duration: BEAT_MS, easing: 'cubic-bezier(0.16, 1, 0.3, 1)' },
  )
}

/** How many times `value` has changed since the first render. */
export function useTick(value: unknown): number {
  const [seen, setSeen] = useState(value)
  const [tick, setTick] = useState(0)
  if (!Object.is(seen, value)) {
    setSeen(value)
    setTick((was) => was + 1)
  }
  return tick
}
