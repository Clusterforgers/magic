// Goal: on a phone an attack arrow at a seat whose tile is scrolled out of the tile strip ends at the edge of the strip it went out by, never off the window or at the tile's hidden place, before and after the strip is scrolled, at a crowded table and in a team game.
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { DEV_WEB_URL } from './ports'

/** How far past the strip's edge an arrowhead may end, in px: the edge mark is 16 px and the head has its own length. */
const SLACK_PX = 24

interface Arrow {
  to: string
  end: { x: number; y: number }
  /** The tile's box and whether any of it shows inside its strip. */
  tile: { left: number; right: number; top: number; bottom: number; shown: boolean } | null
  strip: { left: number; right: number; top: number; bottom: number } | null
}

/** Every attack arrow at a seat with a tile: where it ends, its tile and the strip that clips it. */
function arrows(page: Page): Promise<Arrow[]> {
  return page.evaluate(() => {
    const found: Arrow[] = []
    for (const group of document.querySelectorAll<SVGGElement>('[data-layer="lines"] g[data-to^="seat:"]')) {
      const to = group.getAttribute('data-to') ?? ''
      const seat = to.slice('seat:'.length)
      const tile = document.querySelector(`[data-tile-seat="${seat}"]`)
      if (tile === null) continue
      const path = group.querySelector<SVGPathElement>('path.combat-line')
      const svg = group.ownerSVGElement
      if (path === null || svg === null) continue
      const point = path.getPointAtLength(path.getTotalLength())
      const base = svg.getBoundingClientRect()
      let strip: Element | null = tile.parentElement
      while (strip !== null && !/(auto|scroll|hidden)/.test(getComputedStyle(strip).overflowX)) {
        strip = strip.parentElement
      }
      const box = tile.getBoundingClientRect()
      const room = strip?.getBoundingClientRect()
      const shown = room === undefined || (box.right > room.left && box.left < room.right)
      found.push({
        to,
        end: { x: base.left + point.x, y: base.top + point.y },
        tile: { left: box.left, right: box.right, top: box.top, bottom: box.bottom, shown },
        strip:
          room === undefined
            ? null
            : { left: room.left, right: room.right, top: room.top, bottom: room.bottom },
      })
    }
    return found
  })
}

function expectAtEdge(arrow: Arrow, width: number): void {
  expect(arrow.end.x, `${arrow.to} ends inside the window`).toBeGreaterThanOrEqual(-SLACK_PX)
  expect(arrow.end.x, `${arrow.to} ends inside the window`).toBeLessThanOrEqual(width + SLACK_PX)
  if (arrow.tile === null || arrow.strip === null || arrow.tile.shown) return
  // Scrolled out: the arrow ends inside the strip, near the edge the tile went out by.
  expect(arrow.end.x, `${arrow.to} ends inside the strip`).toBeGreaterThanOrEqual(arrow.strip.left - SLACK_PX)
  expect(arrow.end.x, `${arrow.to} ends inside the strip`).toBeLessThanOrEqual(arrow.strip.right + SLACK_PX)
  expect(arrow.end.y, `${arrow.to} ends at the strip's height`).toBeGreaterThanOrEqual(
    arrow.strip.top - SLACK_PX,
  )
  expect(arrow.end.y, `${arrow.to} ends at the strip's height`).toBeLessThanOrEqual(
    arrow.strip.bottom + SLACK_PX,
  )
  const edge = arrow.tile.left >= arrow.strip.right ? arrow.strip.right : arrow.strip.left
  expect(Math.abs(arrow.end.x - edge), `${arrow.to} ends at the strip's edge`).toBeLessThanOrEqual(SLACK_PX)
}

for (const [name, query] of [
  ['a crowded five-seat table', 'seats=5&crowd=1'],
  ['six seats in two teams', 'seats=6&teams=2v2&prompt=attack-sides'],
] as const) {
  test(`phone arrows at scrolled-out tiles end at the strip's edge: ${name}`, async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 })
    await page.goto(`${DEV_WEB_URL}/dev/table?${query}`)
    await page.locator('[data-layer="lines"] g[data-to^="seat:"]').first().waitFor({ state: 'attached' })
    await page.waitForTimeout(600)
    const before = await arrows(page)
    expect(before.length).toBeGreaterThan(0)
    for (const arrow of before) expectAtEdge(arrow, 390)
    // Scroll every strip to its other end: the tiles that showed go out, the hidden ones come in.
    await page.evaluate(() => {
      for (const tile of document.querySelectorAll('[data-tile-seat]')) {
        let strip: Element | null = tile.parentElement
        while (strip !== null && !/(auto|scroll)/.test(getComputedStyle(strip).overflowX))
          strip = strip.parentElement
        if (strip !== null) strip.scrollLeft = strip.scrollWidth
      }
    })
    await page.waitForTimeout(400)
    const after = await arrows(page)
    for (const arrow of after) expectAtEdge(arrow, 390)
    const hidden = [...before, ...after].filter((arrow) => arrow.tile !== null && !arrow.tile.shown)
    expect(hidden.length, 'an arrow at a tile out of view was checked').toBeGreaterThan(0)
  })
}
