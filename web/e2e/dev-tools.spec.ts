// Goal: the development table's Effects button stands in the top bar's flow at a phone's width and a desktop's, so it covers none of the bar's controls, and its moves open below the bar inside the window.
import { expect, test } from './test'
import { DEV_WEB_URL } from './ports'

interface Box {
  left: number
  top: number
  right: number
  bottom: number
}

const overlaps = (a: Box, b: Box): boolean =>
  a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom

for (const [width, height] of [
  [390, 844],
  [1400, 900],
] as const) {
  test(`the Effects button covers no control of the top bar at ${width}x${height}`, async ({ page }) => {
    await page.setViewportSize({ width, height })
    await page.goto(`${DEV_WEB_URL}/dev/table?seats=4`)
    const toggle = page.getByRole('button', { name: 'Effects' })
    await toggle.waitFor()
    const read = () =>
      page.evaluate(() => {
        const box = (element: Element): Box => {
          const { left, top, right, bottom } = element.getBoundingClientRect()
          return { left, top, right, bottom }
        }
        const bar = document.querySelector('[data-region="bar"]')
        const tool = document.querySelector('[data-dev-tool]')
        const others = [...(bar?.querySelectorAll('button, a') ?? [])]
          .filter((element) => tool === null || !tool.contains(element))
          .filter((element) => element.getBoundingClientRect().width > 0)
          .map(box)
        return { tool: tool === null ? null : box(tool), others, inBar: bar?.contains(tool) ?? false }
      })
    const { tool, others, inBar } = await read()
    expect(inBar).toBe(true)
    expect(tool).not.toBeNull()
    for (const other of others) expect(overlaps(tool as Box, other)).toBe(false)
    await toggle.click()
    const moves = await page.getByRole('button', { name: 'attack', exact: true }).boundingBox()
    expect(moves).not.toBeNull()
    expect(moves?.x ?? -1).toBeGreaterThanOrEqual(0)
    expect((moves?.x ?? 0) + (moves?.width ?? 0)).toBeLessThanOrEqual(width)
    expect(moves?.y ?? 0).toBeGreaterThanOrEqual((tool as Box).bottom)
  })
}
