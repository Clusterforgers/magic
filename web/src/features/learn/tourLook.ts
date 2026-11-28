import { useEffect } from 'react'
import { cardRef } from './anchors'
import type { Target } from './anchors'
import { resolve } from './resolve'
import type { CardLookup } from './resolve'

/** The first of `at` that resolves to a drawn element (or its opener), as the one target to light; else none. */
export function firstShown(at: readonly Target[], lookup: CardLookup, doc: Document = document): Target[] {
  const found = at.find((target) => resolve(target, lookup, doc).kind === 'found')
  return found === undefined ? [] : [found]
}

/** Arrow keys move through a tour and Escape ends it; `forward` absent leaves the right arrow alone. */
export function useTourKeys(
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

/** Scrolls the first element carrying one of `at` into the window when the page put it out of sight. */
export function useInSight(at: readonly Target[]): void {
  useEffect(() => {
    for (const id of at.filter((target) => cardRef(target) === null)) {
      const element = [...document.querySelectorAll<HTMLElement>(`[data-coach="${id}"]`)].find(
        (each) => each.getBoundingClientRect().width > 0,
      )
      if (element === undefined) continue
      const box = element.getBoundingClientRect()
      if (box.top < 0 || box.bottom > window.innerHeight) {
        const still = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches === true
        element.scrollIntoView?.({ block: 'center', behavior: still ? 'auto' : 'smooth' })
      }
      return
    }
  }, [at])
}
