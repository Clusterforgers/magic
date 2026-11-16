/** The parts of the table the card preview keeps clear of: the open prompt's answers and sentence, the strip and the docks, measured where they stand. */
import { useLayoutEffect, useState } from 'react'
import { anchorOf } from './preview'
import type { ClearBox } from './previewClear'

/**
 * The attribute of an element the preview keeps clear of. Its value ranks it (`fitClear`): `answers` (the prompt's
 * buttons and the pass controls, each button measured on its own, the primary one marked), `text` (the sentence),
 * `soft` (the strip while it holds a decision), and empty for a dock.
 */
export const KEEP_CLEAR_ATTR = 'data-keep-clear'

const SELECTOR = `[${KEEP_CLEAR_ATTR}]`

const drawn = (box: ClearBox): boolean => box.width > 0 && box.height > 0

/** The answer buttons in `element`; the primary one is the marked `primary-action`, else the first of a prompt's answers. */
function answersOf(element: Element): ClearBox[] {
  const buttons = [...element.querySelectorAll('button')]
  const primary =
    element.querySelector('[data-slot="primary-action"]') ??
    (element.getAttribute('data-slot') === 'prompt-actions' ? (buttons[0] ?? null) : null)
  return buttons.map((button) => ({
    ...anchorOf(button),
    kind: button === primary ? ('primary' as const) : ('answer' as const),
  }))
}

/** The box of the words in `element`, which a block sentence leaves narrower than its row; the element's where unmeasured. */
function wordsOf(element: Element): ClearBox {
  const range = typeof document.createRange === 'function' ? document.createRange() : null
  range?.selectNodeContents(element)
  const words = typeof range?.getBoundingClientRect === 'function' ? range.getBoundingClientRect() : null
  const box = words !== null && words.width > 0 && words.height > 0 ? words : element.getBoundingClientRect()
  return { left: box.left, top: box.top, width: box.width, height: box.height, kind: 'text' }
}

/** The boxes of the marked elements in window px, the ones that are drawn. */
export function keepClearBoxes(): ClearBox[] {
  return [...document.querySelectorAll(SELECTOR)].flatMap((element): ClearBox[] => {
    const value = element.getAttribute(KEEP_CLEAR_ATTR)
    const box = anchorOf(element)
    if (value === 'answers') {
      const answers = answersOf(element).filter(drawn)
      return answers.length > 0 ? answers : [box].filter(drawn)
    }
    if (value === 'soft') return [{ ...box, soft: true }].filter(drawn)
    if (value === 'text') return [wordsOf(element)].filter(drawn)
    return [box].filter(drawn)
  })
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
      box.soft === other.soft &&
      box.kind === other.kind
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
