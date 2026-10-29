import { useLayoutEffect, useState } from 'react'
import type { RefObject } from 'react'

/** The height of `ref`'s box in whole px, followed with a `ResizeObserver`; null until it has one (a test, a hidden box). */
export function useBoxHeight(ref: RefObject<HTMLElement | null>): number | null {
  const [height, setHeight] = useState<number | null>(null)
  useLayoutEffect(() => {
    const box = ref.current
    if (box === null) return
    const measure = (): void => {
      const next = box.offsetHeight
      setHeight(next > 0 ? next : null)
    }
    measure()
    if (typeof ResizeObserver !== 'function') return
    const observer = new ResizeObserver(measure)
    observer.observe(box)
    return () => observer.disconnect()
  }, [ref])
  return height
}
