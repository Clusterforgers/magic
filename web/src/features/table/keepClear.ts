/** The parts of the table the card preview keeps clear of: the open prompt's strip and the docks, measured where they stand. */
import { useLayoutEffect, useState } from 'react'
import { anchorOf } from './preview'
import type { ClearBox } from './previewClear'

/**
 * The attribute of an element the preview must not cover: the strip while it holds a decision, its buttons, the hand dock
 * and the controls. The value `soft` (the strip) lets a preview cover it rather than shrink below a readable size.
 */
export const KEEP_CLEAR_ATTR = 'data-keep-clear'

const SELECTOR = `[${KEEP_CLEAR_ATTR}]`

/** The boxes of the marked elements in window px, the ones that are drawn. */
export function keepClearBoxes(): ClearBox[] {
  return [...document.querySelectorAll(SELECTOR)]
    .map((element) => ({ ...anchorOf(element), soft: element.getAttribute(KEEP_CLEAR_ATTR) === 'soft' }))
    .filter((box) => box.width > 0 && box.height > 0)
}

const sameBoxes = (a: readonly ClearBox[], b: readonly ClearBox[]): boolean =>
  a.length === b.length &&
  a.every((box, at) => {
    const other = b[at]
    return (
      other !== undefined &&
      box.left === other.left &&
      box.top === other.top &&
      box.width === other.width &&
      box.height === other.height &&
      box.soft === other.soft
    )
  })

/** The boxes the preview keeps clear of, measured again when the window or one of them changes size. */
export function useKeepClear(): readonly ClearBox[] {
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
