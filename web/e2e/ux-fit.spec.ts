// Goal: five fit defects of the layout stay fixed at the sizes of the design (1440x900, 1366x768, 1280x720, 1024x768, 390x844): the host sees the invite link and code of the lobby without scrolling; a hand of ten or more cards ends inside its dock, never under the controls, with its tags whole or an icon; the turn banner lies over neither the step ladder nor the turn badge while it shows; the opening hand is drawn large while the Mulligan prompt waits, with Keep and Mulligan in reach; and the table has no text under 12 px, the letters of a mana symbol included.
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { DEV_WEB_URL } from './ports'

interface Size {
  width: number
  height: number
}

const SIZES: readonly Size[] = [
  { width: 1440, height: 900 },
  { width: 1366, height: 768 },
  { width: 1280, height: 720 },
  { width: 1024, height: 768 },
  { width: 390, height: 844 },
]

const label = (size: Size): string => `${size.width}x${size.height}`

test.describe('the invite of the host', () => {
  test('the link, the code and their Copy buttons are in view without scrolling', async ({ page }) => {
    await page.goto(`${DEV_WEB_URL}/dev/lobby?seats=4&format=commander&invites=1&account=1`)
    await page.waitForSelector('[data-slot="join-invite"]')
    for (const size of SIZES) {
      await page.setViewportSize(size)
      await page.waitForTimeout(200)
      const seen = await page.evaluate(() => {
        const read = (selector: string) => {
          const element = document.querySelector(selector)
          if (element === null) return null
          const { left, top, right, bottom } = element.getBoundingClientRect()
          const middle = document.elementFromPoint((left + right) / 2, (top + bottom) / 2)
          return { left, top, right, bottom, free: middle !== null && element.contains(middle) }
        }
        return {
          inner: { width: window.innerWidth, height: window.innerHeight },
          scrolled: window.scrollY,
          things: {
            link: read('[data-slot="invite-link"]'),
            code: read('[data-slot="invite-code"]'),
            'copy link': read('button[aria-label="Copy invite link"]'),
            'copy code': read('button[aria-label="Copy code"]'),
          },
        }
      })
      expect(seen.scrolled, `${label(size)}: the page has not scrolled`).toBe(0)
      for (const [name, box] of Object.entries(seen.things)) {
        expect(box, `${label(size)}: ${name} is on the page`).not.toBeNull()
        if (box === null) continue
        expect(box.top, `${label(size)}: ${name} starts in the window`).toBeGreaterThanOrEqual(0)
        expect(box.bottom, `${label(size)}: ${name} ends in the window`).toBeLessThanOrEqual(
          seen.inner.height,
        )
        expect(box.right, `${label(size)}: ${name} is not cut sideways`).toBeLessThanOrEqual(seen.inner.width)
        expect(box.free, `${label(size)}: nothing lies over ${name}`).toBe(true)
      }
    }
  })
})

/** The cards of the hand dock and the room they stand in, read in one step. */
async function readHand(page: Page) {
  return page.evaluate(() => {
    const box = (element: Element) => {
      const { left, top, right, bottom } = element.getBoundingClientRect()
      return { left, top, right, bottom }
    }
    const dock = document.querySelector('[data-region="hand"]')
    const controls = document.querySelector('[data-region="controls"]')
    const fans = [...document.querySelectorAll<HTMLElement>('[data-region="hand"] [data-slot="hand-fan"]')]
    const cards = [...document.querySelectorAll('[data-region="hand"] button[data-object-id]')].map(box)
    return {
      dock: dock === null ? null : box(dock),
      controls: controls === null ? null : box(controls),
      cards,
      scrolls: fans.some((fan) => fan.dataset.scrolls !== undefined),
      cut: fans.filter((fan) => fan.scrollWidth > fan.clientWidth).length,
      rows: fans.map((fan) => fan.dataset.rows),
    }
  })
}

test.describe('the hand dock', () => {
  const COUNTS = [7, 9, 10, 11, 12, 14]
  const MARGIN_PX = 8

  test('holds ten or more cards inside its own dock, clear of the controls', async ({ page }) => {
    for (const size of SIZES.filter((s) => s.width >= 1024)) {
      await page.setViewportSize(size)
      for (const count of COUNTS) {
        await page.goto(`${DEV_WEB_URL}/dev/table?board=duel&turn=you&prompt=priority-main&hand=${count}`)
        await page.waitForSelector('[data-region="hand"] [data-slot="hand-slot"]')
        await page.waitForTimeout(250)
        const seen = await readHand(page)
        const name = `${label(size)}, ${count} cards`
        expect(seen.dock, `${name}: a hand dock`).not.toBeNull()
        expect(seen.controls, `${name}: a controls dock`).not.toBeNull()
        if (seen.dock === null || seen.controls === null) continue
        // The hand dock ends where the controls begin: a card past that is under the panel.
        expect(seen.dock.right, `${name}: the docks touch`).toBeLessThanOrEqual(seen.controls.left + 1)
        if (seen.scrolls) continue
        expect(seen.cut, `${name}: a row runs past its line`).toBe(0)
        for (const card of seen.cards) {
          expect(card.right, `${name}: a card ends clear of the controls`).toBeLessThanOrEqual(
            seen.controls.left - MARGIN_PX,
          )
          expect(card.left, `${name}: a card starts inside the dock`).toBeGreaterThanOrEqual(seen.dock.left)
        }
      }
    }
  })

  test('writes a drawn card as the word when it fits and as its icon when it does not, never cut', async ({
    page,
  }) => {
    for (const size of SIZES.filter((s) => s.width >= 1024)) {
      await page.setViewportSize(size)
      await page.goto(`${DEV_WEB_URL}/dev/table?board=duel&prompt=priority-main&hand=9&fx=draw&slow=20`)
      const mark = page.locator('[data-region="hand"] [data-fx-mark="drawn"]')
      await mark.waitFor({ state: 'attached' })
      const seen = await mark.evaluate((element) => {
        const word = element.querySelector<HTMLElement>('.truncate')
        return {
          compact: element.hasAttribute('data-compact'),
          word: word?.textContent ?? null,
          cut: word !== null && word.scrollWidth > word.clientWidth,
        }
      })
      expect(seen.cut, `${label(size)}: the word is cut`).toBe(false)
      expect(seen.word === null, `${label(size)}: the icon alone has no word`).toBe(seen.compact)
    }
  })

  test('shows the hint of a card that cannot be played whole', async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 })
    await page.goto(`${DEV_WEB_URL}/dev/table?board=duel&turn=you&prompt=priority-main&hand=9`)
    await page.waitForSelector('[data-region="hand"] [data-slot="hand-slot"]')
    const hinted = page.locator('[data-region="hand"] button[data-object-id]:has([data-slot="card-hint"])')
    await hinted.first().hover()
    const seen = await hinted
      .first()
      .locator('[data-slot="card-hint"]')
      .evaluate((hint) => ({
        across: hint.scrollWidth > hint.clientWidth,
        down: hint.scrollHeight > hint.clientHeight,
        shown: getComputedStyle(hint).display !== 'none',
      }))
    expect(seen.shown).toBe(true)
    expect(seen.across, 'the hint is cut sideways').toBe(false)
    expect(seen.down, 'the hint is cut downward').toBe(false)
  })
})

test.describe('the turn banner', () => {
  const SEES = [
    ['an opponent starts a turn', 'turn=you'],
    ['your turn starts', ''],
  ] as const

  for (const [name, query] of SEES) {
    test(`lies over neither the step ladder nor the turn badge when ${name}`, async ({ page }) => {
      for (const size of SIZES) {
        await page.setViewportSize(size)
        await page.goto(`${DEV_WEB_URL}/dev/table?seats=4&${query}&newturn=1&slow=20&prompt=priority-main`)
        await page.waitForSelector('[data-slot="turn-banner"][data-phase="hold"]')
        await page.waitForTimeout(300)
        const seen = await page.evaluate(() => {
          const box = (selector: string) => {
            const element = document.querySelector(selector)
            if (element === null) return null
            const { left, top, right, bottom, width, height } = element.getBoundingClientRect()
            return width === 0 || height === 0 ? null : { left, top, right, bottom }
          }
          const banner = document.querySelector('[data-slot="turn-banner"]')
          return {
            place: banner?.getAttribute('data-place') ?? null,
            opacity: banner === null ? 0 : Number(getComputedStyle(banner).opacity),
            banner: box('[data-slot="turn-banner"]'),
            over: { ladder: box('[data-slot="step-ladder"]'), badge: box('[data-slot="turn-badge"]') },
            inner: { width: window.innerWidth, height: window.innerHeight },
          }
        })
        const where = `${label(size)}, ${name}`
        expect(seen.opacity, `${where}: the banner shows`).toBeGreaterThan(0.9)
        expect(seen.banner, `${where}: a banner`).not.toBeNull()
        if (seen.banner === null) continue
        expect(seen.banner.left, `${where}: inside the window`).toBeGreaterThanOrEqual(0)
        expect(seen.banner.right, `${where}: inside the window`).toBeLessThanOrEqual(seen.inner.width)
        // Wide windows keep it in the bar; a phone has no room there and hangs it under the bar.
        expect(seen.place, `${where}: its place`).toBe(size.width >= 768 ? 'bar' : 'below')
        for (const [thing, other] of Object.entries(seen.over)) {
          if (other === null) continue
          const across = Math.min(seen.banner.right, other.right) - Math.max(seen.banner.left, other.left)
          const down = Math.min(seen.banner.bottom, other.bottom) - Math.max(seen.banner.top, other.top)
          expect(across > 1 && down > 1, `${where}: the banner lies over the ${thing}`).toBe(false)
        }
      }
    })
  }
})

test.describe('the opening hand', () => {
  // The fixture hand: 7 cards and two partner commanders share one line, so the width limits the cards (a table with one
  // commander gives 122 px at 1440x900); a hand dock that does not grow draws them 68 to 78 px wide.
  const LEAST: Record<string, number> = { '1440x900': 100, '1366x768': 96, '1280x720': 92, '1024x768': 88 }

  for (const size of SIZES.filter((one) => one.width >= 1024)) {
    test(`is drawn large while the Mulligan prompt waits at ${label(size)}, Keep and Mulligan in reach`, async ({
      page,
    }) => {
      await page.setViewportSize(size)
      await page.goto(`${DEV_WEB_URL}/dev/table?seats=2&prompt=mulligan`)
      await page.waitForSelector('[data-region="hand"] button[data-object-id]')
      await page.waitForTimeout(400)
      const seen = await page.evaluate(() => {
        const box = (element: Element | null | undefined) => {
          const { left, top, right, bottom } = element?.getBoundingClientRect() ?? new DOMRect()
          return { left, top, right, bottom }
        }
        const button = (name: string) =>
          [...document.querySelectorAll('button')].find((each) => each.textContent?.trim() === name)
        const widths = [...document.querySelectorAll('[data-region="hand"] button[data-object-id]')].map(
          (card) => card.getBoundingClientRect().width,
        )
        const keep = box(button('Keep'))
        const hit = document.elementFromPoint((keep.left + keep.right) / 2, (keep.top + keep.bottom) / 2)
        return {
          widths,
          dock: box(document.querySelector('[data-region="hand"]')),
          keep,
          keepFree: hit !== null && button('Keep')?.contains(hit) === true,
          mulligan: box(button('Mulligan')),
          scrolls: document.documentElement.scrollHeight > window.innerHeight,
        }
      })
      expect(Math.min(...seen.widths), `${label(size)}: card widths`).toBeGreaterThanOrEqual(
        LEAST[label(size)] ?? 88,
      )
      // The row holds every card side by side, none above another.
      expect(new Set(seen.widths.map(Math.round)).size).toBe(1)
      expect(seen.dock.bottom).toBeLessThanOrEqual(size.height + 1)
      expect(seen.keepFree, 'Keep is not covered').toBe(true)
      for (const each of [seen.keep, seen.mulligan]) {
        expect(each.top).toBeGreaterThanOrEqual(0)
        expect(each.bottom).toBeLessThan(seen.dock.top)
      }
      expect(seen.scrolls, 'the page scrolls').toBe(false)
    })
  }
})

test.describe('the text floor of the table', () => {
  test('seat zone labels and counter chips read at 12 px or more', async ({ page }) => {
    for (const size of SIZES) {
      await page.setViewportSize(size)
      await page.goto(`${DEV_WEB_URL}/dev/table?states=1&seats=4&prompt=priority-main`)
      await page.waitForSelector('[data-region="hand"] [data-slot="hand-slot"]')
      await page.waitForTimeout(300)
      const small = await page.evaluate(() => {
        const found: string[] = []
        const words = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT)
        for (let node = words.nextNode(); node !== null; node = words.nextNode()) {
          const text = (node.nodeValue ?? '').replace(/\s+/g, ' ').trim()
          const parent = node.parentElement
          // The letters of a mana symbol count too, though the symbol is a picture with its own spoken name (`role="img"`).
          if (text === '' || parent === null || parent.closest('svg') !== null) continue
          const px = parseFloat(getComputedStyle(parent).fontSize)
          if (px < 12) found.push(`${Math.round(px * 10) / 10}px ${text.slice(0, 20)}`)
        }
        return found
      })
      expect(small, `${label(size)}: text under 12 px`).toEqual([])
    }
  })
})
