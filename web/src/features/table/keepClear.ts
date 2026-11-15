/** The parts of the table the card preview keeps clear of: the open prompt's strip and the docks, measured where they stand. */
import { useLayoutEffect, useState } from 'react'
import { anchorOf } from './preview'
import type { Anchor } from './preview'

/** The attribute of an element the preview must not cover: the strip while it holds a decision, the hand dock and the controls. */
export const KEEP_CLEAR_ATTR = 'data-keep-clear'

const SELECTOR = `[${KEEP_CLEAR_ATTR}]`

/** The boxes of the marked elements in window px, the ones that are drawn. */
export function keepClearBoxes(): Anchor[] {
  return [...document.querySelectorAll(SELECTOR)]
    .map(anchorOf)
    .filter((box) => box.width > 0 && box.height > 0)
}

const sameBoxes = (a: readonly Anchor[], b: readonly Anchor[]): boolean =>
  a.length === b.length &&
  a.every((box, at) => {
    const other = b[at]
    return (
      other !== undefined &&
      box.left === other.left &&
      box.top === other.top &&
      box.width === other.width &&
      box.height === other.height
    )
  })

/** The boxes the preview keeps clear of, measured again when the window or one of them changes size. */
export function useKeepClear(): readonly Anchor[] {
  const [boxes, setBoxes] = useState(keepClearBoxes)
  useLayoutEffect(() => {
    const measure = (): void => {
      const next = keepClearBoxes()
      setBoxes((held) => (sameBoxes(held, next) ? held : next))
    }
    measure()
    const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(measure)
    for (const element of document.querySelectorAll(SELECTOR)) observer?.observe(element)
    window.addEventListener('resize', measure)
    return () => {
      observer?.disconnect()
      window.removeEventListener('resize', measure)
    }
  }, [])
  return boxes
}
