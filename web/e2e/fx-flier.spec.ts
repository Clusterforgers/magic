// Goal: on the development table at 1440x900 and 390x844 the flier of `?fx=cast` (the viewer's spell flying from the hand to the stack) is drawn over the table, takes no click (the pass control under it answers) and moves no layout: the boxes of everything else are the same with the layer shown and hidden, and a flier far outside the window makes the page no larger.
import { expect, test } from '@playwright/test'
import { holdMidway, LAYER, openPlay } from './fxPlay'

const SIZES = [
  { width: 1440, height: 900 },
  { width: 390, height: 844 },
]

const FLIER = '[data-fx-flier="cast"]'
const PASS = '[data-region="controls"] [data-slot="primary-action"]'

for (const size of SIZES) {
  test.describe(`the flier of a cast at ${size.width}x${size.height}`, () => {
    test.beforeEach(async ({ page }) => {
      await openPlay(page, size, 'seats=4&turn=you&prompt=priority&fx=cast')
      await holdMidway(page, FLIER)
    })

    test('is drawn over the table', async ({ page }) => {
      const seen = await page.evaluate(
        ({ layer, flier }) => {
          const root = document.querySelector<HTMLElement>(layer)
          const piece = document.querySelector<HTMLElement>(flier)
          if (root === null || piece === null) throw new Error('no flier')
          const box = piece.getBoundingClientRect()
          const x = box.left + box.width / 2
          const y = box.top + box.height / 2
          const style = getComputedStyle(root)
          // The layer takes no pointer, so what lies on top at a point is found by letting it take one for a moment.
          root.style.pointerEvents = 'auto'
          const top = document.elementFromPoint(x, y)
          root.style.pointerEvents = ''
          return {
            box: { left: box.left, top: box.top, right: box.right, bottom: box.bottom },
            width: box.width,
            height: box.height,
            opacity: Number(getComputedStyle(piece).opacity),
            position: style.position,
            top: top !== null && piece.contains(top),
            window: { width: window.innerWidth, height: window.innerHeight },
          }
        },
        { layer: LAYER, flier: FLIER },
      )
      expect(seen.position).toBe('fixed')
      expect(seen.opacity).toBe(1)
      // A card, not a speck: wider than the 40 px of a pod's chip and as high as it is wide at least.
      expect(seen.width).toBeGreaterThan(40)
      expect(seen.height).toBeGreaterThan(seen.width)
      expect(seen.box.left).toBeGreaterThanOrEqual(0)
      expect(seen.box.top).toBeGreaterThanOrEqual(0)
      expect(seen.box.right).toBeLessThanOrEqual(seen.window.width)
      expect(seen.box.bottom).toBeLessThanOrEqual(seen.window.height)
      expect(seen.top, 'the flier is the topmost thing at its middle').toBe(true)
    })

    test('takes no click: the pass control under it answers', async ({ page }) => {
      const pass = page.locator(PASS)
      await expect(pass).toBeVisible()
      const under = await page.evaluate(
        ({ flier, control }) => {
          const piece = document.querySelector<HTMLElement>(flier)
          const button = document.querySelector<HTMLElement>(control)
          if (piece === null || button === null) throw new Error('no flier or no pass control')
          // Lays the flier over the middle of the control, where its hold left it elsewhere.
          for (const animation of piece.getAnimations()) animation.cancel()
          const at = button.getBoundingClientRect()
          const size = piece.getBoundingClientRect()
          const x = at.left + at.width / 2
          const y = at.top + at.height / 2
          piece.style.transform = `translate(${x - size.width / 2}px, ${y - size.height / 2}px)`
          const laid = piece.getBoundingClientRect()
          const hit = document.elementFromPoint(x, y)
          return {
            lies: laid.left <= x && x <= laid.right && laid.top <= y && y <= laid.bottom,
            reaches: hit !== null && button.contains(hit),
          }
        },
        { flier: FLIER, control: PASS },
      )
      expect(under.lies, 'the flier lies over the middle of the pass control').toBe(true)
      expect(under.reaches, 'a pointer at that point reaches the control').toBe(true)
      // Playwright refuses a click that another element would take, so this also holds that nothing intercepts it.
      await pass.click({ timeout: 5_000 })
      await expect(page.locator('[data-decision]')).toHaveCount(0)
    })

    test('moves no layout', async ({ page }) => {
      const measured = await page.evaluate(
        ({ layer, flier }) => {
          const root = document.querySelector<HTMLElement>(layer)
          const piece = document.querySelector<HTMLElement>(flier)
          if (root === null || piece === null) throw new Error('no flier')
          const boxes = (): string =>
            [...document.body.querySelectorAll('*')]
              .filter((element) => !root.contains(element))
              .map((element) => {
                const box = element.getBoundingClientRect()
                return `${box.x},${box.y},${box.width},${box.height}`
              })
              .join('|')
          const pageSize = (): number[] => [
            document.documentElement.scrollWidth,
            document.documentElement.scrollHeight,
          ]
          const own = getComputedStyle(root)
          const layerBox = root.getBoundingClientRect()
          const shown = boxes()
          const shownSize = pageSize()
          root.style.display = 'none'
          const hidden = boxes()
          const hiddenSize = pageSize()
          root.style.display = ''
          // A flier beyond the window's corner must not make the page scrollable.
          for (const animation of piece.getAnimations()) animation.cancel()
          piece.style.transform = `translate(${window.innerWidth + 200}px, ${window.innerHeight + 200}px)`
          const far = pageSize()
          return {
            fixed:
              own.position === 'fixed' &&
              layerBox.width === window.innerWidth &&
              layerBox.height === window.innerHeight,
            same: shown === hidden,
            count: shown.split('|').length,
            shownSize,
            hiddenSize,
            far,
            window: [window.innerWidth, window.innerHeight],
          }
        },
        { layer: LAYER, flier: FLIER },
      )
      expect(measured.fixed, 'the layer is fixed to the window, outside the flow').toBe(true)
      expect(measured.count).toBeGreaterThan(100)
      expect(measured.same, 'every other box is the same with the layer hidden').toBe(true)
      expect(measured.shownSize).toEqual(measured.hiddenSize)
      expect(measured.far).toEqual(measured.hiddenSize)
      expect(measured.shownSize[0]).toBeLessThanOrEqual(measured.window[0] ?? 0)
      expect(measured.shownSize[1]).toBeLessThanOrEqual(measured.window[1] ?? 0)
    })
  })
}
