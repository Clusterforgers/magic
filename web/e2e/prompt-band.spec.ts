// Goal: on the development table (a duel, a grid of six, two teams, Two-Headed Giant and the boss) the prompt band is one line of the same height and place for every decision kind and between decisions, its answers on one row and nothing scrolled, no board, card or column moves from one prompt to the next, and the decision's detail stands in the Stack column at 1400x900 and 1280x720 (the side panel open) and in the sheet over the hand with the panel folded and on a phone, where the band is two lines and a choice of cards stands in the overlay; there the opening hand keeps its answers on one row with the line of who is still deciding beside them and nothing under them, at 2 to 40 seats.
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { DEV_WEB_URL } from './ports'

type Box = [number, number, number, number]

/** What a prompt shows, read in one step. */
interface Shown {
  /** The band's box, and whether what it holds is taller than it. */
  band: Box
  bandScrolls: boolean
  /** The tops of the answers' buttons. */
  answerTops: number[]
  /** The answers outside the band's box. */
  answersOut: number
  /** The boxes that must not move: the board rows, the first card of each, the hand and the side column. */
  still: Record<string, Box | null>
  /** The centre overlay lays out a long choice of cards: the strip stands above the hand meanwhile. */
  overlay: boolean
  /** Where the decision's detail is drawn, and whether its choices are in it. */
  detail: 'column' | 'sheet' | null
  /** The decision sheet's box. */
  sheet: Box | null
  chip: boolean
}

function readShown(): Shown {
  const box = (element: Element | null): Box | null => {
    if (element === null) return null
    const { x, y, width, height } = element.getBoundingClientRect()
    return [Math.round(x), Math.round(y), Math.round(width), Math.round(height)]
  }
  const sheet = document.querySelector('[data-slot="shared-sheet"]')
  const band = box(sheet)
  if (sheet === null || band === null) throw new Error('no band')
  const edge = sheet.getBoundingClientRect()
  const answers = [...sheet.querySelectorAll('[data-slot="prompt-actions"] button')]
  const one = (selector: string): Box | null => box(document.querySelector(selector))
  const detail = document.querySelector('[data-slot="decision-detail"]')
  return {
    band,
    bandScrolls: sheet.scrollHeight > sheet.clientHeight + 1,
    answerTops: answers.map((button) => Math.round(button.getBoundingClientRect().top)),
    answersOut: answers.filter((button) => {
      const at = button.getBoundingClientRect()
      return at.top < edge.top - 0.5 || at.bottom > edge.bottom + 0.5
    }).length,
    still: {
      opponents: one('[data-area="opponents"]'),
      own: one('[data-area="own"]'),
      hand: one('[data-area="hand"]'),
      side: one('aside[data-side]'),
      'own card': one('[data-area="own"] button[data-object-id][data-size]'),
      'their card': one('[data-area="opponents"] button[data-object-id][data-size]'),
    },
    overlay: document.querySelector('[data-slot="choice-host"]:not([hidden])') !== null,
    detail: detail === null ? null : (detail.getAttribute('data-where') as 'column' | 'sheet'),
    sheet: one('[data-slot="decision-sheet"]'),
    chip: sheet.querySelector('[data-slot="prompt-asker"]') !== null,
  }
}

/** The decisions of one moment of the fixture game, a kind each; `null` is the table between decisions. */
const PROMPTS = [
  null,
  'yesno',
  'option',
  'mode-two',
  'targets',
  'cards-graveyard',
  'order',
  'pay',
  'x',
  'scry',
  'order-cards',
  'divide',
  'costs',
  'replacement',
] as const

/** Opens the dev table's `prompt` on `table`, the side panel folded where asked, and reads it once it has settled. */
async function show(page: Page, table: string, prompt: string | null, folded = false): Promise<Shown> {
  await page.goto(`${DEV_WEB_URL}/dev/table?${table}${prompt === null ? '' : `&prompt=${prompt}`}`)
  await page.locator('[data-slot="shared-sheet"]').waitFor()
  // A payment's sentence is read aloud and not drawn.
  if (prompt !== null) await page.locator('[data-slot="prompt-text"]').first().waitFor({ state: 'attached' })
  // The fold is remembered, so a second page of the same test opens folded already.
  const fold = page.getByRole('button', { name: 'Fold the side panel' })
  if (folded && (await fold.isVisible())) {
    await fold.click()
    // The table holds its rows through a fold; a fresh page lays them out for the folded panel.
    await page.reload()
    await page.locator('[data-slot="shared-sheet"]').waitFor()
  }
  if (folded) await expect(page.locator('aside[data-side="rail"]')).toBeAttached()
  // The boards fit themselves over a few frames: two equal reads in a row are the settled table.
  let last = ''
  await expect
    .poll(
      async () => {
        const now = JSON.stringify(await page.evaluate(readShown))
        const same = now === last
        last = now
        return same
      },
      { message: `${table} ${prompt ?? 'no prompt'} settles`, intervals: [150] },
    )
    .toBe(true)
  return page.evaluate(readShown)
}

/** Whether two boxes are the same to the pixel but one. */
const same = (a: Box | null, b: Box | null): boolean =>
  a === null || b === null ? a === b : a.every((value, at) => Math.abs(value - (b[at] ?? 0)) <= 1)

const TABLES = [
  'seats=2&board=duel',
  'seats=6&mode=grid',
  'seats=4&teams=2v2',
  'seats=4&teams=2hg',
  'seats=4&boss=1',
] as const

test.describe('the prompt band and the Stack column', () => {
  // Each test opens the table once for every decision kind.
  test.describe.configure({ timeout: 300_000 })

  for (const [width, height] of [
    [1400, 900],
    [1280, 720],
  ] as const) {
    for (const table of TABLES) {
      test(`nothing moves from one decision to the next on ${table} at ${width}x${height}`, async ({
        page,
      }) => {
        await page.setViewportSize({ width, height })
        const quiet = await show(page, table, null)
        // One line of controls over the ticker's line.
        expect(quiet.band[3], 'one line').toBeLessThanOrEqual(88)
        expect(quiet.band[3]).toBeGreaterThanOrEqual(72)
        expect(quiet.bandScrolls, 'the band between decisions scrolls').toBe(false)
        for (const prompt of PROMPTS.slice(1)) {
          const shown = await show(page, table, prompt)
          const label = `${table} ${prompt} ${width}x${height}`
          expect(shown.band[3], `${label}: the band keeps its height`).toBe(quiet.band[3])
          expect(shown.bandScrolls, `${label}: the band scrolls`).toBe(false)
          expect(new Set(shown.answerTops).size, `${label}: the answers keep one row`).toBeLessThanOrEqual(1)
          expect(shown.answersOut, `${label}: answers outside the band`).toBe(0)
          // A long choice of cards takes the centre overlay, which covers the boards and stands the strip above the hand.
          if (!shown.overlay) {
            expect(
              same(shown.band, quiet.band),
              `${label}: the band keeps its box, ${shown.band} against ${quiet.band}`,
            ).toBe(true)
            for (const [name, box] of Object.entries(shown.still)) {
              // A payment opens your piles of equal lands, so each can be tapped: the cards of a full board size anew.
              if (prompt === 'pay' && name === 'own card') continue
              expect(
                same(box, quiet.still[name] ?? null),
                `${label}: ${name} moved, ${box} against ${quiet.still[name]}`,
              ).toBe(true)
            }
          }
          expect(shown.detail, `${label}: the detail is in the column`).toBe('column')
          expect(shown.sheet).toBeNull()
          await expect(page.getByRole('tab', { name: /^Stack/ }), label).toHaveAttribute(
            'aria-selected',
            'true',
          )
        }
      })
    }
  }

  test('shows the asking card in the band and the column, and gives the tab back after the answer', async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1400, height: 900 })
    const shown = await show(page, 'seats=2&board=duel', 'ask-may')
    expect(shown.chip).toBe(true)
    await expect(page.locator('[data-slot="decision-detail"] [data-slot="asked-clause"]')).toHaveText(
      /you may pay .*If you do, draw a card\./,
    )
    // The chip lights the card on the board.
    const asker = await page.locator('[data-slot="prompt-asker"]').getAttribute('data-asker')
    const card = page.locator(`[data-area="own"] button[data-object-id="${asker}"]`)
    await expect(card).toHaveCSS('outline-style', 'solid')
    await page.keyboard.press('n')
    await expect(page.getByRole('tab', { name: /^Log/ })).toHaveAttribute('aria-selected', 'true')
  })

  test('holds the detail in a sheet over the hand with the side panel folded, the band and the boards as they were', async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1400, height: 900 })
    const quiet = await show(page, 'seats=2&board=duel', null, true)
    for (const prompt of ['yesno', 'order', 'pay', 'mode-two']) {
      const shown = await show(page, 'seats=2&board=duel', prompt, true)
      expect(
        same(shown.band, quiet.band),
        `${prompt}: the band keeps its box, ${shown.band} against ${quiet.band}`,
      ).toBe(true)
      expect(
        same(shown.still['own'] ?? null, quiet.still['own'] ?? null),
        `${prompt}: your board moved`,
      ).toBe(true)
      expect(shown.detail, `${prompt}: the detail is in the sheet`).toBe('sheet')
      expect(shown.sheet?.[3] ?? 0, `${prompt}: the sheet is at most 330 px`).toBeLessThanOrEqual(331)
    }
  })

  test('is two lines on a phone, the detail in the sheet and a choice of cards in the overlay', async ({
    page,
  }) => {
    await page.setViewportSize({ width: 390, height: 844 })
    const quiet = await show(page, 'seats=2&board=duel', null)
    expect(quiet.band[3]).toBe(112)
    for (const prompt of ['yesno', 'order', 'pay', 'x', 'scry']) {
      const shown = await show(page, 'seats=2&board=duel', prompt)
      expect(same(shown.band, quiet.band), `${prompt}: the band keeps its box, ${shown.band}`).toBe(true)
      expect(shown.bandScrolls, `${prompt}: the band scrolls`).toBe(false)
      expect(new Set(shown.answerTops).size, `${prompt}: the answers keep one row`).toBeLessThanOrEqual(1)
      expect(shown.detail, `${prompt}: the detail is in the sheet`).toBe('sheet')
      expect(shown.sheet?.[3] ?? 0).toBeLessThanOrEqual(331)
    }
    // The handle folds the sheet to one line.
    const handle = page.locator('[data-slot="sheet-grab"]')
    await handle.click()
    await expect(handle).toHaveAttribute('aria-expanded', 'false')
    expect(
      await page
        .locator('[data-slot="decision-sheet"]')
        .evaluate((sheet) => (sheet as HTMLElement).offsetHeight),
    ).toBeLessThanOrEqual(48)
    await page.goto(`${DEV_WEB_URL}/dev/table?seats=6&mode=grid&prompt=targets-zones`)
    const overlay = page.locator('[data-slot="choice-overlay"]')
    await expect(overlay.locator('[data-candidate]')).toHaveCount(6, { timeout: 30_000 })
    await expect(page.locator('[data-slot="shared-sheet"] [data-slot="card-row"]')).toHaveCount(0)
    await expect(page.locator('[data-slot="prompt-actions"] button').first()).toBeInViewport()
    await expect(page.locator('[data-slot="sheet-grab"]')).toHaveAttribute('aria-expanded', 'false')
  })

  test('keeps the opening hand on a phone to one row of answers, the line of who is still deciding clear of them', async ({
    page,
  }) => {
    for (const [width, height] of [
      [390, 844],
      [360, 740],
    ] as const) {
      await page.setViewportSize({ width, height })
      for (const seats of [2, 4, 12, 40]) {
        const shown = await show(page, `seats=${seats}`, 'mulligan')
        const label = `${seats} seats at ${width}x${height}`
        // The hand may scroll the sheet on a short phone; the answers stay sticky at its foot (`PromptLayout`).
        expect(new Set(shown.answerTops).size, `${label}: the answers keep one row`).toBe(1)
        expect(shown.answersOut, `${label}: answers outside the band`).toBe(0)
        const clash = await page.evaluate(() => {
          const sheet = document.querySelector('[data-slot="shared-sheet"]')
          if (sheet === null) return ['no band']
          const edge = sheet.getBoundingClientRect()
          const answers = [...sheet.querySelectorAll('[data-slot="prompt-actions"] button')]
          // Whatever the band draws as words besides the answers: the line of who is still deciding, the status.
          const words = [
            ...sheet.querySelectorAll(
              '[data-slot="mulligan-waiting"], [data-slot="strip-status"] p, [data-slot="strip-status"] li',
            ),
          ].filter((element) => element.getBoundingClientRect().width > 0)
          const meets = (a: DOMRect, b: DOMRect): boolean =>
            a.left < b.right - 0.5 &&
            b.left < a.right - 0.5 &&
            a.top < b.bottom - 0.5 &&
            b.top < a.bottom - 0.5
          const problems: string[] = []
          for (const word of words) {
            const box = word.getBoundingClientRect()
            const name = (word.textContent ?? '').slice(0, 30)
            if (box.left < edge.left - 0.5 || box.right > edge.right + 0.5)
              problems.push(`"${name}" is cut off`)
            for (const answer of answers)
              if (meets(box, answer.getBoundingClientRect()))
                problems.push(`"${name}" runs under ${answer.textContent}`)
          }
          for (const answer of answers) {
            const box = answer.getBoundingClientRect()
            const top = document.elementFromPoint(box.left + box.width / 2, box.top + box.height / 2)
            if (top === null || !answer.contains(top)) problems.push(`${answer.textContent} is covered`)
          }
          return problems
        })
        expect(clash, label).toEqual([])
        if (seats > 2)
          await expect(page.locator('[data-slot="mulligan-waiting"]'), label).toHaveText(/^Still deciding: /)
      }
    }
  })
})
