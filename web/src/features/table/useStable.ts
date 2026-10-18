import { useRef } from 'react'
import { sameJson } from '@/play/view'

/**
 * Like zustand's `useShallow`, but compares deeply: a selector may return a small object holding arrays, and the
 * component redraws only when its content changes. Keep the selected value small and plain JSON.
 */
export function useStable<S, U>(selector: (state: S) => U): (state: S) => U {
  const previous = useRef<{ value: U } | null>(null)
  return (state) => {
    const next = selector(state)
    if (previous.current !== null && sameJson(previous.current.value, next)) return previous.current.value
    previous.current = { value: next }
    return next
  }
}
