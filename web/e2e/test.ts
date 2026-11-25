import { test as base } from '@playwright/test'

export { expect } from '@playwright/test'

/**
 * Playwright's `test` with one change: every browser context a test opens is closed when the test ends, passed or
 * failed. A context left open keeps its pages live (sockets, polls, animations) in the worker's one browser for every
 * later spec of the run, so a full run slows down where a spec alone does not. Every spec imports `test` from here.
 */
export const test = base.extend<{ closesContexts: undefined }>({
  closesContexts: [
    async ({ browser }, use) => {
      const before = new Set(browser.contexts())
      await use(undefined)
      // Torn down after Playwright's own `context`, so what is left is what the test opened and did not close.
      const left = browser.contexts().filter((context) => !before.has(context))
      await Promise.all(left.map((context) => context.close()))
    },
    { auto: true },
  ],
})
