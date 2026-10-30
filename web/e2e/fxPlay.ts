// The run that `fx-flier.spec.ts` and `fx-spotlight.spec.ts` share: a move of `/dev/table?fx=` held still where it can be measured.
import type { Page } from '@playwright/test'
import type { Size } from './layoutAudit'
import { DEV_WEB_URL } from './ports'

/** The layer the moves of a batch fly on (`fx/FxTravel.tsx`). */
export const LAYER = '[data-slot="fx-travel"]'

/** The stretch of the move's time (`?slow=`, at most 20): long enough for a page that has been told to hold it still. */
const SLOW = 20

/** Opens the development table at `size` with the move named in `query` (`fx=cast`) playing, stretched by `SLOW`. */
export async function openPlay(page: Page, size: Size, query: string): Promise<void> {
  await page.setViewportSize(size)
  await page.goto(`${DEV_WEB_URL}/dev/table?${query}&slow=${SLOW}`)
}

/**
 * Waits for the layer to hold `target` and stops every animation in it, `target`'s halfway through its time (the delay
 * counted) where its picture stands whole and fully drawn, the others at their start. It then stays as it is for the
 * measurements; nothing ends on its own, so the layer stays too.
 */
export async function holdMidway(page: Page, target: string): Promise<void> {
  await page.locator(`${LAYER} ${target}`).waitFor({ state: 'attached' })
  await page.evaluate(
    ({ layer, held }) => {
      const root = document.querySelector(layer)
      const mine = root?.querySelector(held)
      if (root === null || mine === null || mine === undefined) throw new Error('nothing to hold')
      for (const animation of root.getAnimations({ subtree: true })) {
        animation.pause()
        // Linear, so that halfway through the time is halfway along a flier's way, where it is whole.
        animation.effect?.updateTiming({ easing: 'linear' })
        const timing = animation.effect?.getTiming()
        const midway = Number(timing?.delay ?? 0) + Number(timing?.duration ?? 0) / 2
        const on = animation.effect instanceof KeyframeEffect ? animation.effect.target : null
        animation.currentTime = on === mine ? midway : 0
      }
    },
    { layer: LAYER, held: target },
  )
}
