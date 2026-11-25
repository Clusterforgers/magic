// Goal: a payment on a crowded board (60 and 120 more mana sources of every kind: basics, duals, rocks, Signets, Treasures, filter lands, Cabal Coffers, creature-only mana) opens within a small multiple of the plain pay step's main-thread time (and under a ceiling) and stays quick: a source pressed on the board reads pressed, "Pay for me" lays out the game's payment and Confirm pays it; in a duel and at four seats, at 1400x900 and on a phone (390x844).
import { expect, test } from './test'
import { mainThreadMs } from './mainThread'
import { DEV_WEB_URL } from './ports'

const meter = '[data-slot="pay-meter"]'

/**
 * What opening each crowded payment may cost, in main-thread CPU ms (`mainThreadMs`): a multiple of the plain pay step
 * on the same table and a ceiling. A busy machine stretches neither: other processes do not count, and a core shared
 * with a sibling thread slows the plain step as much as the crowded one.
 */
const BUDGET = {
  'pay-crowd': { ratio: 4, ceiling: 2_000 },
  'pay-crowd-120': { ratio: 5, ceiling: 3_000 },
}

for (const [width, height] of [
  [1400, 900],
  [390, 844],
] as const) {
  for (const seats of [2, 4]) {
    for (const prompt of ['pay-crowd', 'pay-crowd-120'] as const) {
      test(`${prompt} at ${seats} seats, ${width}x${height} stays quick`, async ({ page }) => {
        test.setTimeout(60_000)
        await page.setViewportSize({ width, height })
        // The table first, then the payment: the time to the pay step is the prompt's own work.
        await page.goto(`${DEV_WEB_URL}/dev/table?seats=${seats}`)
        await page.locator('section[aria-label$="\'s battlefield"]').first().waitFor()
        const go = (query: string, open: boolean) =>
          mainThreadMs(page, async () => {
            const found = await page.evaluate(
              async ([url, slot, want]) => {
                const start = performance.now()
                window.history.pushState({}, '', url)
                window.dispatchEvent(new PopStateEvent('popstate'))
                while ((document.querySelector(slot) !== null) !== want) {
                  await new Promise((done) => requestAnimationFrame(done))
                  if (performance.now() - start > 20_000) return false
                }
                return true
              },
              [`/dev/table?seats=${seats}${query}`, meter, open] as const,
            )
            expect(found, `the pay step ${open ? 'opens' : 'closes'}`).toBe(true)
          })
        // The first opening also loads and compiles the code of the payment; the best of three after it is the
        // prompt's own work, without a stray collection or a sibling thread's turn on the core.
        const best = async (query: string) => {
          const tries: number[] = []
          for (let n = 0; n < 3; n += 1) {
            await go('', false)
            tries.push(await go(query, true))
          }
          return Math.min(...tries)
        }
        await go(`&prompt=${prompt}`, true)
        await go('&prompt=pay', true)
        const plain = await best('&prompt=pay')
        const opened = await best(`&prompt=${prompt}`)
        // Against the plain pay step of the same table, measured just before: what the crowd adds stays a small
        // multiple of it (about 2 to 3 here), and a ceiling holds a slowdown that both share.
        const { ratio, ceiling } = BUDGET[prompt]
        expect(opened, `opening ${prompt} against the plain pay step's ${Math.round(plain)} ms`).toBeLessThan(
          plain * ratio,
        )
        expect(opened).toBeLessThan(ceiling)

        const legal = page
          .locator(
            `section[aria-label$="'s battlefield"] button[data-highlight="legal"][aria-label*="Basic Land"]`,
          )
          .first()
        const id = await legal.getAttribute('data-object-id')
        const card = page.locator(`button[data-object-id="${id}"]`)
        const pressed = await mainThreadMs(page, async () => {
          await card.click({ position: { x: 10, y: 6 } })
          await expect(card).toHaveAttribute('aria-pressed', 'true')
        })
        expect(pressed).toBeLessThan(1_000)
        await card.click({ position: { x: 10, y: 6 } })
        await expect(card).toHaveAttribute('aria-pressed', 'false')

        const confirm = page.getByRole('button', { name: 'Confirm payment' })
        const laidOut = await mainThreadMs(page, async () => {
          await page.getByRole('button', { name: 'Pay for me', exact: true }).click()
          await expect(confirm).toBeEnabled()
        })
        expect(laidOut).toBeLessThan(1_000)
        await confirm.click()
        await expect(page.locator('[data-decision]')).toHaveCount(0)
      })
    }
  }
}
