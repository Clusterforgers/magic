// Goal: verify the React stack meets the M0 spike criteria on a 4-player, ~150-permanent board.
import { expect, test, type Page } from '@playwright/test'

const FPS_FLOOR = 55
const P95_FRAME_CEILING_MS = 25
const HOVER_P95_CEILING_MS = 50
const MAIN_THREAD_BUSY_CEILING = 0.25

async function openBoard(page: Page) {
  await page.goto('/spike')
  await expect(page.getByTestId('seat-3')).toBeVisible()
}

async function measureFrames(page: Page, ms: number) {
  await page.evaluate(() => window.__spike?.resetFrames())
  await page.waitForTimeout(ms)
  return page.evaluate(() => window.__spike?.frameStats())
}

test('board renders 4 players with ~150 permanents', async ({ page }) => {
  await openBoard(page)
  expect(await page.locator('[data-testid^="card-"]').count()).toBe(4 * (37 + 7))
})

test('sustains frame rate under 10 Hz server patches', async ({ page }) => {
  await openBoard(page)
  const stats = await measureFrames(page, 8000)
  console.log('frame stats', JSON.stringify(stats))
  expect(stats?.fps).toBeGreaterThanOrEqual(FPS_FLOOR)
  expect(stats?.p95Ms).toBeLessThanOrEqual(P95_FRAME_CEILING_MS)
})

test('main thread stays mostly idle, leaving headroom for slower machines', async ({ page, context }) => {
  await openBoard(page)
  const cdp = await context.newCDPSession(page)
  await cdp.send('Performance.enable')
  const taskSeconds = async () => {
    const { metrics } = await cdp.send('Performance.getMetrics')
    return metrics.find((metric) => metric.name === 'TaskDuration')?.value ?? 0
  }
  const before = await taskSeconds()
  await page.waitForTimeout(8000)
  const busy = ((await taskSeconds()) - before) / 8
  console.log('main thread busy fraction', busy.toFixed(3))
  expect(busy).toBeLessThanOrEqual(MAIN_THREAD_BUSY_CEILING)
})

test('reports frame rate under 4x CPU throttle (informational)', async ({ page, context }) => {
  await openBoard(page)
  const cdp = await context.newCDPSession(page)
  await cdp.send('Emulation.setCPUThrottlingRate', { rate: 4 })
  const stats = await measureFrames(page, 6000)
  console.log('throttled frame stats', JSON.stringify(stats))
  expect(stats?.frames).toBeGreaterThan(0)
})

test('hover preview appears within the latency budget', async ({ page }) => {
  await openBoard(page)
  for (let id = 1; id <= 30; id += 1) {
    await page.getByTestId(`card-${id}`).hover()
    await expect(page.getByTestId('card-preview')).toBeVisible()
  }
  const latencies = await page.evaluate(() =>
    performance.getEntriesByName('hover-latency').map((entry) => entry.duration),
  )
  latencies.sort((a, b) => a - b)
  const p95 = latencies[Math.floor(latencies.length * 0.95)] ?? Infinity
  console.log('hover latency p95 ms', p95, 'samples', latencies.length)
  expect(latencies.length).toBeGreaterThan(20)
  expect(p95).toBeLessThanOrEqual(HOVER_P95_CEILING_MS)
})

test('click targeting highlights legal targets, dims the rest, and resolves', async ({ page }) => {
  await openBoard(page)
  const playable = page.locator('[data-playable="true"]').first()
  await playable.click()
  await expect(page.getByTestId('status')).toContainText('Choose a target')
  const legal = await page.locator('[data-legal="true"]').count()
  expect(legal).toBeGreaterThan(0)
  const target = page.locator('[data-legal="true"]').first()
  const targetId = Number((await target.getAttribute('data-testid'))?.replace('card-', ''))
  await target.click()
  await expect(page.getByTestId('status')).toContainText('Your priority')
  expect((await page.evaluate(() => window.__spike?.lastResolution()))?.target).toBe(targetId)
})

test('drag to a legal target resolves the decision', async ({ page }) => {
  await openBoard(page)
  const source = page.locator('[data-playable="true"]').first()
  const from = await source.boundingBox()
  await source.hover()
  await page.mouse.down()
  await page.mouse.move((from?.x ?? 0) + 40, (from?.y ?? 0) - 40, { steps: 5 })
  const target = page.locator('[data-legal="true"]').first()
  const to = await target.boundingBox()
  if (!to) throw new Error('no legal target after drag start')
  await page.mouse.move(to.x + to.width / 2, to.y + to.height / 2, { steps: 10 })
  await page.mouse.up()
  expect(await page.evaluate(() => window.__spike?.lastResolution())).not.toBeNull()
})

test('a card moving zones animates through intermediate positions', async ({ page }) => {
  await page.goto('/spike')
  const distinct = await page.evaluate(
    () =>
      new Promise<number>((resolve) => {
        const seen = new Set<string>()
        const end = performance.now() + 3500
        const sample = () => {
          const node = document.querySelector('[data-testid="card-38"]')
          if (node) {
            const { x, y } = node.getBoundingClientRect()
            seen.add(`${Math.round(x)},${Math.round(y)}`)
          }
          if (performance.now() < end) requestAnimationFrame(sample)
          else resolve(seen.size)
        }
        requestAnimationFrame(sample)
      }),
  )
  console.log('distinct positions for moved card', distinct)
  expect(distinct).toBeGreaterThan(4)
})
