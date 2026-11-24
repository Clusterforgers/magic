// Times work in a page by the CPU its main thread spends on tasks (Chromium's own task timing, in thread ticks), which other processes on a busy machine do not stretch the way they stretch the wall clock.
import type { Page } from '@playwright/test'

/** The main-thread CPU milliseconds the page's tasks (script, style, layout) take while `work` runs. */
export async function mainThreadMs(page: Page, work: () => Promise<void>): Promise<number> {
  const cdp = await page.context().newCDPSession(page)
  try {
    await cdp.send('Performance.enable', { timeDomain: 'threadTicks' })
    const taskSecs = async () =>
      (await cdp.send('Performance.getMetrics')).metrics.find((metric) => metric.name === 'TaskDuration')
        ?.value ?? 0
    const before = await taskSecs()
    await work()
    return ((await taskSecs()) - before) * 1000
  } finally {
    await cdp.detach()
  }
}
