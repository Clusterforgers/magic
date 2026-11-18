// Goal: the prompt band of a card prompt on the development table (a grid of six, a duel, two teams, Two-Headed Giant and the boss) keeps its answers on one row, shows a stack of three chips whole beside the cards, draws every card whole with none scrolled sideways (twelve stay on the row or wrap), shows every player offered as a target, and never scrolls, at 1400x900 with the side panel open and folded and at 1920x1080; on a phone the cards stand in the choice overlay.
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { DEV_WEB_URL } from './ports'

/** What the band shows, read in one step. */
interface Band {
  /** The tops of the answers' buttons. */
  answerTops: number[]
  /** The stack chips in view, each with its name whole or not, and whether "+N" stands beside them. */
  chips: { whole: boolean }[]
  more: boolean
  /** The prompt's cards: how many, how many lie outside the sheet, the narrowest width. */
  cards: number
  cardsOut: number
  cardWidth: number
  /** The row of cards scrolls sideways; the players' column or the sheet scrolls down. */
  rowScrolls: boolean
  playersScroll: boolean
  playersOut: number
  sheetScrolls: boolean
}

function readBand(): Band {
  const sheet = document.querySelector('[data-slot="shared-sheet"]')
  if (sheet === null) throw new Error('no sheet')
  const edge = sheet.getBoundingClientRect()
  const out = (element: Element): boolean => {
    const box = element.getBoundingClientRect()
    return (
      box.left < edge.left - 0.5 ||
      box.right > edge.right + 0.5 ||
      box.top < edge.top - 0.5 ||
      box.bottom > edge.bottom + 0.5
    )
  }
  const cards = [...sheet.querySelectorAll('[data-candidate]')]
  const row = sheet.querySelector('[data-slot="card-row"]')
  const players = sheet.querySelector('[data-slot="player-targets"]')
  return {
    answerTops: [...sheet.querySelectorAll('[data-slot="prompt-actions"] button')].map((button) =>
      Math.round(button.getBoundingClientRect().top),
    ),
    chips: [...sheet.querySelectorAll('[data-slot="stack-chips"] li:not([aria-hidden]) button')].map(
      (chip) => {
        // The name, not the picture's own cut caption.
        const name = chip.querySelector(':scope > .truncate')
        return { whole: name !== null && name.scrollWidth <= name.clientWidth + 0.5 && !out(chip) }
      },
    ),
    more: sheet.querySelector('[data-slot="stack-more"]') !== null,
    cards: cards.length,
    cardsOut: cards.filter(out).length,
    cardWidth: Math.min(...cards.map((card) => card.getBoundingClientRect().width)),
    rowScrolls: row !== null && row.scrollWidth > row.clientWidth + 1,
    playersScroll: players !== null && players.scrollHeight > players.clientHeight + 1,
    playersOut: players === null ? 0 : [...players.querySelectorAll('button')].filter(out).length,
    sheetScrolls: sheet.scrollHeight > sheet.clientHeight + 1,
  }
}

/** Opens the dev table's `prompt` on `table` in a window of `width` x `height`, the side panel folded where asked. */
async function open(
  page: Page,
  table: string,
  prompt: string,
  width: number,
  height: number,
  folded = false,
) {
  await page.setViewportSize({ width, height })
  await page.goto(`${DEV_WEB_URL}/dev/table?${table}&prompt=${prompt}`)
  await page.locator('[data-slot="shared-sheet"] [data-candidate]').first().waitFor()
  // The fold is remembered, so a second page of the same test opens folded already.
  const fold = page.getByRole('button', { name: 'Fold the side panel' })
  if (folded && (await fold.isVisible())) await fold.click()
  if (folded) await expect(page.locator('aside[data-side="rail"]')).toBeAttached()
}

/** The band holds its rules: answers on one row, whole cards in the sheet, nothing scrolled. */
async function expectBand(page: Page, label: string, cards?: number): Promise<Band> {
  // The stack's column measures its chips after a layout; the band settles within a few frames.
  await expect
    .poll(() => page.evaluate(readBand), { message: label })
    .toMatchObject({
      cards: cards ?? expect.any(Number),
      cardsOut: 0,
      rowScrolls: false,
      sheetScrolls: false,
    })
  const band = await page.evaluate(readBand)
  expect(new Set(band.answerTops).size, `${label}: the answers keep one row`).toBe(1)
  expect(band.cardWidth, `${label}: cards at 3 rem or more`).toBeGreaterThanOrEqual(47.5)
  expect(band.playersScroll, `${label}: the players column scrolls`).toBe(false)
  expect(band.playersOut, `${label}: players outside the sheet`).toBe(0)
  return band
}

const TABLES = ['seats=6&mode=grid', 'seats=2', 'teams=2v2', 'teams=2hg', 'boss=1'] as const

test.describe('the prompt band of a card prompt', () => {
  // Each test opens the table up to six times.
  test.describe.configure({ timeout: 180_000 })

  for (const folded of [false, true]) {
    test(`keeps the answers on one row, three stack chips whole and every card whole at 1400x900 on a grid of six${folded ? ', the side panel folded' : ''}`, async ({
      page,
    }) => {
      for (const [prompt, cards] of [
        ['cards-battlefield', 4],
        ['targets-zones', 6],
        ['targets-two', 12],
      ] as const) {
        await open(page, 'seats=6&mode=grid', prompt, 1400, 900, folded)
        const band = await expectBand(page, prompt, cards)
        expect(band.chips, `${prompt}: three chips in view`).toHaveLength(3)
        expect(
          band.chips.every((chip) => chip.whole),
          `${prompt}: every chip whole`,
        ).toBe(true)
        expect(band.more, `${prompt}: "+N" beside three items`).toBe(false)
        // Six target cards beside the players were 48 px wide on one cut row; now they stand larger and whole.
        if (prompt === 'targets-zones') expect(band.cardWidth).toBeGreaterThan(70)
        if (prompt === 'cards-battlefield') expect(band.cardWidth).toBeGreaterThan(90)
      }
    })
  }

  for (const table of TABLES) {
    test(`holds on ${table} at 1400x900 and 1920x1080`, async ({ page }) => {
      for (const [width, height] of [
        [1400, 900],
        [1920, 1080],
      ] as const) {
        for (const prompt of ['targets-zones', 'targets-two', 'cards-battlefield']) {
          await open(page, table, prompt, width, height)
          const band = await expectBand(page, `${table} ${prompt} ${width}x${height}`)
          expect(band.cards, `${table} ${prompt} ${width}x${height}: cards in the band`).toBeGreaterThan(0)
        }
      }
    })
  }

  test('draws the cards of a phone in the choice overlay, the band keeping its answers', async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 })
    await page.goto(`${DEV_WEB_URL}/dev/table?seats=6&mode=grid&prompt=targets-zones`)
    const overlay = page.locator('[data-slot="choice-overlay"]')
    await expect(overlay.locator('[data-candidate]')).toHaveCount(6, { timeout: 30_000 })
    await expect(page.locator('[data-slot="shared-sheet"] [data-slot="card-row"]')).toHaveCount(0)
    await expect(page.locator('[data-slot="prompt-actions"] button').first()).toBeInViewport()
  })
})
