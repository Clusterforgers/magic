import { useEffect, useLayoutEffect, useState } from 'react'
import type { RefObject } from 'react'
import { FIXED_ANCHORS } from './anchors'
import type { AnchorId, Target } from './anchors'
import { boxOf, resolve, shownAnchors } from './resolve'
import type { Box, CardLookup } from './resolve'

/** How often the coach measures the page again, in ms. */
export const LOOK_MS = 150
/** The ring stands this far outside the lit element, in px. */
export const RING_PAD = 4
/** The width of the coach card on a wide window, in px. */
export const CARD_W = 300
/** Under this width the card is a bar across the window. */
export const PHONE_W = 640

/** One measured look at the page: each lit target's element and box, the opener used, and the anchors shown. */
export interface Look {
  lit: { target: Target; box: Box; via: AnchorId | null; element: HTMLElement }[]
  missing: Target[]
  clear: Box[]
  phone: { top: number; bottom: number } | undefined
  window: { width: number; height: number }
  shown: ReadonlySet<AnchorId>
  coarse: boolean
}

const CLEAR = [
  '[data-coach="ask"]',
  '[data-coach="pass.primary"]',
  '[data-slot="decision-sheet"]',
  '[data-slot="decision-detail"]',
  '[data-coach="preview"]',
]

/** Measures the page for `targets`. */
export function look(targets: readonly Target[], lookup: CardLookup): Look {
  const lit: Look['lit'] = []
  const missing: Target[] = []
  for (const target of targets) {
    const found = resolve(target, lookup)
    if (found.kind === 'found')
      lit.push({ target, box: boxOf(found.element), via: found.via, element: found.element })
    else {
      if (found.kind === 'many' && import.meta.env.DEV)
        console.error(`coach: ${target} resolves to ${found.count}`)
      missing.push(target)
    }
  }
  const clear = CLEAR.flatMap((selector) =>
    [...document.querySelectorAll<HTMLElement>(selector)].map(boxOf).filter((box) => box.width > 0),
  )
  const width = window.innerWidth
  const height = window.innerHeight
  const bar = document.querySelector('[data-region="bar"]')
  // On a phone the card may lie over the hand, never over the decision bar under it (its answers are kept clear).
  const below =
    document.querySelector('[data-region="controls"]') ?? document.querySelector('[data-region="hand"]')
  const phone =
    width < PHONE_W
      ? {
          top: bar === null ? 0 : bar.getBoundingClientRect().bottom,
          bottom: below === null ? height : below.getBoundingClientRect().top,
        }
      : undefined
  return {
    lit,
    missing,
    clear,
    phone,
    window: { width, height },
    shown: shownAnchors(FIXED_ANCHORS),
    coarse: window.matchMedia?.('(pointer: coarse)').matches === true,
  }
}

/** The box `box` grown by `pad` on every side. */
export const grown = (box: Box, pad: number): Box => ({
  x: box.x - pad,
  y: box.y - pad,
  width: box.width + 2 * pad,
  height: box.height + 2 * pad,
})

/** Draws the page again every `LOOK_MS` and on every resize while `on`, so the light follows what moves. */
export function useLooking(on: boolean, onLook?: () => void): void {
  const [, setTick] = useState(0)
  useEffect(() => {
    if (!on) return undefined
    const again = (): void => setTick((value) => value + 1)
    const timer = window.setInterval(() => {
      again()
      onLook?.()
    }, LOOK_MS)
    window.addEventListener('resize', again)
    return () => {
      window.clearInterval(timer)
      window.removeEventListener('resize', again)
    }
  }, [on, onLook])
}

/** The open size of the coach card at `card`; a folded bar keeps the size it had open. */
export function useCardSize(card: RefObject<HTMLElement | null>): { width: number; height: number } {
  const [size, setSize] = useState({ width: CARD_W, height: 160 })
  useLayoutEffect(() => {
    const element = card.current
    // Folding and unfolding would chase each other if a folded bar placed itself by its folded size.
    if (element === null || element.dataset['folded'] !== undefined) return
    const { width, height } = element.getBoundingClientRect()
    if (Math.abs(width - size.width) > 1 || Math.abs(height - size.height) > 1) setSize({ width, height })
  })
  return size
}
