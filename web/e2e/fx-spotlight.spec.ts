// Goal: at the six sizes of docs/ui-design.md the spotlight of `?fx=rival` (another seat's spell standing large over the shared strip) is drawn whole inside the window, lies over no button of the prompt in front of the viewer (the pass control, or the buttons of a declaring-blockers sheet), no control of the controls dock and no card of the hand, and takes no click: nothing in it or its layer can be the target of a pointer; the spotlight of the viewer's own cast (`?fx=cast`) keeps the same clear of all three.
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { holdMidway, LAYER, openPlay } from './fxPlay'
import { SIZES } from './layoutAudit'

const SPOTLIGHT = '[data-slot="fx-spotlight"]'

/** What the viewer faces while the spell stands, and the buttons of it that the spotlight must stay clear of. */
const PROMPTS = [
  { name: 'priority', query: 'seats=4&turn=you&prompt=priority', buttons: '[data-slot="primary-action"]' },
  {
    name: 'declaring blockers',
    query: 'seats=4&prompt=block',
    buttons: '[data-slot="prompt-actions"] button',
  },
] as const

/** The things a pointer of the viewer must reach whatever stands over the strip, by what they are called in a message. */
const CLEAR_OF = {
  'the controls dock': '[data-region="controls"] button',
  'the hand': '[data-region="hand"] button',
} as const

interface Seen {
  box: { left: number; top: number; right: number; bottom: number }
  opacity: number
  window: { width: number; height: number }
  /** For each selector, how many elements it found and the ones the spotlight's box lies over (by a short name). */
  over: Record<string, { found: number; covered: string[] }>
}

/** The box of the spotlight and, for each group of controls, how many there are and which the box lies over. */
async function look(page: Page, groups: Record<string, string>): Promise<Seen> {
  return page.evaluate(
    ({ spotlight, selectors }) => {
      const shown = document.querySelector<HTMLElement>(spotlight)
      if (shown === null) throw new Error('no spotlight')
      const box = shown.getBoundingClientRect()
      const over: Record<string, { found: number; covered: string[] }> = {}
      for (const [name, selector] of Object.entries(selectors)) {
        const found = [...document.querySelectorAll<HTMLElement>(selector)]
        const covered = found
          .filter((control) => {
            const at = control.getBoundingClientRect()
            return (
              at.width > 0 &&
              at.height > 0 &&
              at.left < box.right &&
              at.right > box.left &&
              at.top < box.bottom &&
              at.bottom > box.top
            )
          })
          .map((control) =>
            (control.textContent ?? control.getAttribute('aria-label') ?? '').trim().slice(0, 30),
          )
        over[name] = { found: found.length, covered }
      }
      return {
        box: { left: box.left, top: box.top, right: box.right, bottom: box.bottom },
        opacity: Number(getComputedStyle(shown).opacity),
        window: { width: window.innerWidth, height: window.innerHeight },
        over,
      }
    },
    { spotlight: SPOTLIGHT, selectors: groups },
  )
}

for (const prompt of PROMPTS) {
  test.describe(`the spotlight of a rival's cast while the viewer faces ${prompt.name}`, () => {
    for (const size of SIZES) {
      test(`lies over no button of the prompt and no control of the dock at ${size.width}x${size.height}`, async ({
        page,
      }) => {
        await openPlay(page, size, `${prompt.query}&fx=rival`)
        await holdMidway(page, SPOTLIGHT)
        const seen = await look(page, { 'the prompt': prompt.buttons, ...CLEAR_OF })
        // Whole and drawn: inside the window, and not faded out at the moment it is measured.
        expect(seen.opacity).toBe(1)
        expect(seen.box.left).toBeGreaterThanOrEqual(0)
        expect(seen.box.top).toBeGreaterThanOrEqual(0)
        expect(seen.box.right).toBeLessThanOrEqual(seen.window.width)
        expect(seen.box.bottom).toBeLessThanOrEqual(seen.window.height)
        // The groups are not empty, so that nothing is held by a selector that finds nothing.
        for (const [name, group] of Object.entries(seen.over)) {
          expect(group.found, `${name} has controls`).toBeGreaterThan(0)
          expect(group.covered, `the spotlight lies over ${name}`).toEqual([])
        }
      })
    }
  })
}

test.describe('the spotlight of the viewer’s own cast (`?fx=cast`)', () => {
  for (const size of [SIZES[1], SIZES[3], SIZES[5]]) {
    test(`is marked as the viewer's own and lies over no prompt button, no control of the dock and no card of the hand at ${size?.width}x${size?.height}`, async ({
      page,
    }) => {
      if (size === undefined) throw new Error('no size')
      await openPlay(page, size, `${PROMPTS[0].query}&fx=cast`)
      await holdMidway(page, SPOTLIGHT)
      await expect(page.locator(SPOTLIGHT)).toHaveAttribute('data-own', 'true')
      await expect(page.locator(`${SPOTLIGHT} [data-slot="fx-spotlight-caption"]`)).toContainText('You cast')
      const seen = await look(page, { 'the prompt': PROMPTS[0].buttons, ...CLEAR_OF })
      expect(seen.opacity).toBe(1)
      expect(seen.box.left).toBeGreaterThanOrEqual(0)
      expect(seen.box.top).toBeGreaterThanOrEqual(0)
      expect(seen.box.right).toBeLessThanOrEqual(seen.window.width)
      expect(seen.box.bottom).toBeLessThanOrEqual(seen.window.height)
      for (const [name, group] of Object.entries(seen.over)) {
        expect(group.found, `${name} has controls`).toBeGreaterThan(0)
        expect(group.covered, `the spotlight lies over ${name}`).toEqual([])
      }
      // It never holds back the viewer's next action: while it stands, the pass control answers a click.
      await page.locator(PROMPTS[0].buttons).click({ timeout: 5_000 })
      await expect(page.locator('[data-decision]')).toHaveCount(0)
    })
  }
})

test.describe('the spotlight of a rival’s cast takes no click', () => {
  for (const size of [SIZES[1], SIZES[3], SIZES[5]]) {
    test(`at ${size?.width}x${size?.height}`, async ({ page }) => {
      if (size === undefined) throw new Error('no size')
      await openPlay(page, size, 'seats=4&turn=you&prompt=priority&fx=rival')
      await holdMidway(page, SPOTLIGHT)
      const where = await page.evaluate(
        ({ spotlight, layer }) => {
          const shown = document.querySelector<HTMLElement>(spotlight)
          const root = document.querySelector<HTMLElement>(layer)
          if (shown === null || root === null) throw new Error('no spotlight')
          const box = shown.getBoundingClientRect()
          // The card's upper middle and the caption's foot: both lie in the spotlight.
          const points = [
            { x: box.left + box.width / 2, y: box.top + box.height / 4 },
            { x: box.left + box.width / 2, y: box.bottom - 16 },
          ]
          // Every click of the page is noted, to see where the pointer went.
          const reached: Element[] = []
          document.addEventListener(
            'click',
            (event) => event.target instanceof Element && reached.push(event.target),
            {
              capture: true,
            },
          )
          Object.assign(window, { clicked: reached })
          return {
            points,
            pointers: [root, shown, ...shown.querySelectorAll('*')].map(
              (element) => getComputedStyle(element).pointerEvents,
            ),
            under: points.map(({ x, y }) => {
              const hit = document.elementFromPoint(x, y)
              return hit !== null && !root.contains(hit)
            }),
          }
        },
        { spotlight: SPOTLIGHT, layer: LAYER },
      )
      expect(new Set(where.pointers), 'the layer and everything in the spotlight takes no pointer').toEqual(
        new Set(['none']),
      )
      expect(where.under, 'a pointer at the card and at the caption finds the table under them').toEqual([
        true,
        true,
      ])
      for (const { x, y } of where.points) await page.mouse.click(x, y)
      const clicked = await page.evaluate((layer) => {
        const root = document.querySelector(layer)
        const targets = (window as unknown as { clicked: Element[] }).clicked
        return { count: targets.length, inLayer: targets.filter((target) => root?.contains(target)).length }
      }, LAYER)
      // Both clicks arrived somewhere, and none of them in the layer.
      expect(clicked.count).toBe(2)
      expect(clicked.inLayer).toBe(0)
    })
  }
})
