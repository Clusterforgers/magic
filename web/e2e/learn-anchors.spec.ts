// Goal: at a desktop, a laptop and a phone with touch, at every step of every lesson each lit part is one element inside the window that is on top at its centre (and at its corners, for a control), the ring lies on it, and the coach card is inside the window, shows its whole text and covers no lit part, the prompt band, the primary button, an open decision sheet or the card preview.
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { LAYOUTS, LESSONS, coachState, settle, startLesson, walkLesson } from './learnWalker'
import type { CoachState } from './learnWalker'

/** What the page says about the coach and what it lights at one step. */
async function measure(page: Page) {
  return page.evaluate(() => {
    const box = (element: Element) => {
      const rect = element.getBoundingClientRect()
      return { x: rect.left, y: rect.top, width: rect.width, height: rect.height }
    }
    const root = document.querySelector<HTMLElement>('[data-slot="coach"]')
    const selectors = JSON.parse(root?.dataset['coachSelectors'] ?? '[]') as string[]
    const drawn = (element: HTMLElement): boolean => {
      const rect = element.getBoundingClientRect()
      const style = getComputedStyle(element)
      return rect.width > 0 && rect.height > 0 && style.visibility !== 'hidden' && style.display !== 'none'
    }
    // Each lit part as the coach names it: the drawn outermost elements its selector finds (a card's face carries its id too).
    const found = selectors.map((selector) => {
      const all = [...document.querySelectorAll<HTMLElement>(selector)].filter(drawn)
      return all.filter((element) => !all.some((other) => other !== element && other.contains(element)))
    })
    const lit = found.map((all) => all[0]).filter((element): element is HTMLElement => element !== undefined)
    const counts = found.map((all) => all.length)
    const onTop = (element: HTMLElement, x: number, y: number): boolean => {
      // A mark drawn for the eye only (pointer-events: none) is on top when nothing else is over it.
      const passive = getComputedStyle(element).pointerEvents === 'none'
      if (passive) element.style.pointerEvents = 'auto'
      try {
        return hitOn(element, x, y)
      } finally {
        if (passive) element.style.pointerEvents = ''
      }
    }
    const hitOn = (element: HTMLElement, x: number, y: number): boolean => {
      const hit = document.elementFromPoint(x, y)
      return hit !== null && (hit === element || element.contains(hit))
    }
    const card = document.querySelector<HTMLElement>('[data-slot="coach-card"]')
    const clear = [
      '[data-coach="band"]',
      '[data-coach="pass.primary"]',
      '[data-slot="decision-sheet"]',
      '[data-coach="preview"]',
    ].flatMap((selector) =>
      [...document.querySelectorAll(selector)].map((element) => ({ selector, box: box(element) })),
    )
    return {
      window: { width: window.innerWidth, height: window.innerHeight },
      // The ring's own place, not its box: it pulses (a scale) as it arrives.
      rings: [...document.querySelectorAll<HTMLElement>('[data-slot="coach-ring"]')].map((ring) => ({
        x: parseFloat(ring.style.left),
        y: parseFloat(ring.style.top),
        width: parseFloat(ring.style.width),
        height: parseFloat(ring.style.height),
      })),
      lit: lit.map((element) => {
        const b = box(element)
        const inset = 4
        // A control must be whole under the finger; a region (a board, a pod, the band) only at its centre, as a hand fan's
        // lifted cards may overlap a board's edge.
        const control =
          element.matches('button, [role="button"], [role="menuitem"], a, input') &&
          !element.hasAttribute('data-object-id')
        const points = [[b.x + b.width / 2, b.y + b.height / 2]]
        if (control)
          points.push(
            [b.x + inset, b.y + inset],
            [b.x + b.width - inset, b.y + inset],
            [b.x + inset, b.y + b.height - inset],
            [b.x + b.width - inset, b.y + b.height - inset],
          )
        return {
          name: element.dataset['coach'] ?? `card ${element.dataset['objectId']}`,
          box: b,
          covered: points.filter(([x, y]) => !onTop(element, x ?? 0, y ?? 0)).length,
          by: points
            .filter(([x, y]) => !onTop(element, x ?? 0, y ?? 0))
            .map(([x, y]) => {
              const hit = document.elementFromPoint(x ?? 0, y ?? 0)
              const slot = hit?.closest('[data-slot]')
              return `${hit?.tagName ?? 'none'} ${slot?.getAttribute('data-slot') ?? ''}`
            })
            .join(', '),
        }
      }),
      card: card === null ? null : box(card),
      cut:
        card === null
          ? false
          : card.scrollHeight > card.clientHeight + 1 || card.scrollWidth > card.clientWidth + 1,
      counts,
      clear,
    }
  })
}

type Box = { x: number; y: number; width: number; height: number }
const overlaps = (a: Box, b: Box): boolean =>
  a.x < b.x + b.width - 1 && b.x < a.x + a.width - 1 && a.y < b.y + b.height - 1 && b.y < a.y + a.height - 1

async function check(page: Page, shown: CoachState): Promise<void> {
  await settle(page)
  // What the coach shows once the page is still; the step may have started meanwhile (the game stopped moving).
  const state = (await coachState(page)) ?? shown
  // A part that animates (a life total that just changed) is measured once it rests under its ring: the snapshot is
  // taken when the rings lie on the parts, as the coach drew them.
  let seen = await measure(page)
  await expect
    .poll(async () => {
      seen = await measure(page)
      return seen.rings.every((ring, at) => {
        const b = seen.lit[at]?.box
        return b !== undefined && Math.abs(ring.x + 4 - b.x) < 2 && Math.abs(ring.width - 8 - b.width) < 2
      })
    })
    .toBe(true)
    .catch(() => undefined)
  const missing = await page.locator('[data-slot="coach"]').getAttribute('data-coach-missing')
  const placed = await page.locator('[data-slot="coach"]').getAttribute('data-coach-place')
  const where = `step ${state.step} (${state.lit}; missing ${missing ?? ''}; place ${placed ?? ''})`
  const card = seen.card
  expect(card, where).not.toBeNull()
  if (card === null) return
  expect(card.x, `${where}: the card is inside the window`).toBeGreaterThanOrEqual(0)
  expect(card.y, `${where}: the card is inside the window`).toBeGreaterThanOrEqual(0)
  expect(card.x + card.width, `${where}: the card is inside the window`).toBeLessThanOrEqual(
    seen.window.width + 0.5,
  )
  expect(card.y + card.height, `${where}: the card is inside the window`).toBeLessThanOrEqual(
    seen.window.height + 0.5,
  )
  expect(seen.cut, `${where}: the card's text is cut`).toBe(false)
  const lit = state.lit === '' ? [] : state.lit.split('|')
  expect(seen.lit.length, `${where}: one element per lit part`).toBe(lit.length)
  expect(seen.counts, `${where}: each lit part is one drawn element`).toEqual(lit.map(() => 1))
  expect(seen.rings.length, `${where}: one ring per lit part`).toBe(lit.length)
  for (const [at, each] of seen.lit.entries()) {
    const b = each.box
    expect(b.x + b.width, `${where}: ${each.name} is inside the window`).toBeGreaterThan(0)
    expect(b.y + b.height, `${where}: ${each.name} is inside the window`).toBeGreaterThan(0)
    expect(b.x, `${where}: ${each.name} is inside the window`).toBeLessThan(seen.window.width)
    expect(b.y, `${where}: ${each.name} is inside the window`).toBeLessThan(seen.window.height)
    expect(each.covered, `${where}: ${each.name} is covered by ${each.by}`).toBe(0)
    const ring = seen.rings[at]
    if (ring !== undefined) {
      expect(
        Math.abs(ring.x + 4 - b.x),
        `${where}: the ring ${JSON.stringify(ring)} lies on ${each.name} ${JSON.stringify(b)}`,
      ).toBeLessThan(2)
      expect(
        Math.abs(ring.width - 8 - b.width),
        `${where}: the ring ${JSON.stringify(ring)} lies on ${each.name} ${JSON.stringify(b)}`,
      ).toBeLessThan(2)
    }
    expect(overlaps(card, b), `${where}: the card covers ${each.name}`).toBe(false)
  }
  for (const each of seen.clear)
    expect(overlaps(card, each.box), `${where}: the card covers ${each.selector}`).toBe(false)
}

for (const layout of LAYOUTS) {
  test.describe(`the anchors at ${layout.name}`, () => {
    test.use({ hasTouch: layout.touch, isMobile: layout.touch })
    for (const lesson of LESSONS) {
      test(`${lesson}: every lit part is drawn, on top and clear of the card`, async ({ page }) => {
        test.setTimeout(240_000)
        await startLesson(page, layout, lesson)
        const shots = process.env['LEARN_SHOTS']
        const steps = await walkLesson(page, layout, {
          check,
          shots: shots === undefined ? undefined : `${shots}/${layout.name}-${lesson}`,
        })
        expect(steps.at(-1)).toBe('end')
      })
    }
  })
}
