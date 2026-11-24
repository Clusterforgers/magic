import { useLayoutEffect, useState } from 'react'
import type { RefObject } from 'react'

/** One side of `ref`'s box in whole px, followed with a `ResizeObserver`; null until it has one (a test, a hidden box). */
function useBoxSide(ref: RefObject<HTMLElement | null>, side: 'offsetHeight' | 'offsetWidth'): number | null {
  const [size, setSize] = useState<number | null>(null)
  useLayoutEffect(() => {
    const box = ref.current
    if (box === null) return
    const measure = (): void => {
      const next = box[side]
      setSize(next > 0 ? next : null)
    }
    measure()
    if (typeof ResizeObserver !== 'function') return
    const observer = new ResizeObserver(measure)
    observer.observe(box)
    return () => observer.disconnect()
  }, [ref, side])
  return size
}

/** The height of `ref`'s box in whole px, followed with a `ResizeObserver`; null until it has one (a test, a hidden box). */
export function useBoxHeight(ref: RefObject<HTMLElement | null>): number | null {
  return useBoxSide(ref, 'offsetHeight')
}

/** The width of `ref`'s box in whole px, followed with a `ResizeObserver`; null until it has one (a test, a hidden box). */
export function useBoxWidth(ref: RefObject<HTMLElement | null>): number | null {
  return useBoxSide(ref, 'offsetWidth')
}
