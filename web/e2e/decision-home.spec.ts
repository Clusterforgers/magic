// Goal: on the development table (a duel, a grid of six, two teams, Two-Headed Giant and the boss) every decision kind has one home, the strip between the boards stays one status line and no board, card, hand or column moves from one prompt to the next: a card's question stands under its card in the Stack panel with the column open, in the dock with it folded or on its Log tab, and in the bar on a phone, whose payment grows into a sheet of at most 330 px; the answers are in the window and uncovered (your cards size anew, in place, only where the dock grows over their corner), and the dock and the bar never scroll.
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { DEV_WEB_URL } from './ports'

type Box = [number, number, number, number]

/** What a prompt shows, read in one step. */
interface Shown {
  /** The strip's box. */
  strip: Box | null
  /** The homes drawn for the decision, by `data-home`. */
  homes: string[]
  /** The answers of the home outside the window or covered by something else. */
  hidden: string[]
  /** The dock or bar holds more than its box. */
  homeScrolls: boolean
  /** The boxes that must not move: the board rows, the first card of each, the hand and the side column. */
  still: Record<string, Box | null>
  /** The centre overlay or a window lays out a choice: the strip stands above the hand meanwhile. */
  overlay: boolean
  /** The dock's body grows above its cell, over the corner of your board. */
  dockOver: boolean
  /** A phone's payment sheet. */
  sheet: Box | null
}

function readShown(): Shown {
  const box = (element: Element | null): Box | null => {
    if (element === null) return null
    const { x, y, width, height } = element.getBoundingClientRect()
    return [Math.round(x), Math.round(y), Math.round(width), Math.round(height)]
  }
  const one = (selector: string): Box | null => box(document.querySelector(selector))
  const lines = [...document.querySelectorAll<HTMLElement>('[data-slot="decision-line"]')]
  const hidden: string[] = []
  for (const line of lines) {
    for (const answer of line.querySelectorAll<HTMLElement>('[data-slot="prompt-actions"] > button')) {
      const at = answer.getBoundingClientRect()
      const name = answer.getAttribute('aria-label') ?? answer.textContent ?? ''
      // A disabled button takes no pointer, so the point finds what is under it; it is in view all the same.
      if (at.width === 0 || answer.matches(':disabled')) continue
      if (
        at.top < 0 ||
        at.bottom > window.innerHeight + 0.5 ||
        at.left < 0 ||
        at.right > window.innerWidth + 0.5
      ) {
        hidden.push(`${name} is outside the window`)
        continue
      }
      const top = document.elementFromPoint(at.left + at.width / 2, at.top + at.height / 2)
      if (top === null || !answer.contains(top)) hidden.push(`${name} is covered`)
    }
  }
  const home = document.querySelector<HTMLElement>('[data-slot="decision-dock"], [data-slot="decision-bar"]')
  return {
    strip: one('[data-region="shared"]'),
    homes: lines.map((line) => line.dataset['home'] ?? 'none'),
    hidden,
    homeScrolls: home !== null && home.scrollHeight > home.clientHeight + 1,
    still: {
      opponents: one('[data-area="opponents"]'),
      own: one('[data-area="own"]'),
      hand: one('[data-area="hand"]'),
      controls: one('[data-area="controls"]'),
      side: one('aside[data-side]'),
      'own card': one('[data-area="own"] button[data-object-id][data-size]'),
      'their card': one('[data-area="opponents"] button[data-object-id][data-size]'),
    },
    overlay: document.querySelector('[data-slot="choice-host"]:not([hidden])') !== null,
    dockOver:
      (document.querySelector('[data-slot="dock-body"]')?.getBoundingClientRect().top ?? Infinity) <
      (document.querySelector('[data-region="controls"]')?.getBoundingClientRect().top ?? -Infinity) - 1,
    sheet: one('[data-slot="decision-sheet"]'),
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

/** The game's questions, answered in the dock whatever the column shows. */
const GAME_ASKS = new Set(['priority', 'attack', 'block', 'mulligan', 'discard', 'bottom'])

type Side = 'open' | 'folded' | 'log'

/** The home a decision of `prompt` has on a desktop with the side column `side`. */
const homeOf = (prompt: string, side: Side): string =>
  GAME_ASKS.has(prompt) || side !== 'open' ? 'dock' : 'panel'

/** Opens the dev table's `prompt` on `table` with the side column as asked, and reads it once it has settled. */
async function show(page: Page, table: string, prompt: string | null, side: Side = 'open'): Promise<Shown> {
  await page.goto(
    `${DEV_WEB_URL}/dev/table?${table}&side=${side}${prompt === null ? '' : `&prompt=${prompt}`}`,
  )
  await page.locator('[data-slot="shared-sheet"]').waitFor()
  if (prompt !== null)
    await page.locator('[data-slot="decision-line"]').first().waitFor({ state: 'attached' })
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

/** The column open on every table; folded and on its Log tab on the duel, which holds every home the others do. */
const RUNS: Array<[string, Side]> = [
  ...TABLES.map((table): [string, Side] => [table, 'open']),
  ['seats=2&board=duel', 'folded'],
  ['seats=2&board=duel', 'log'],
]

test.describe('the decision homes', () => {
  // Each test opens the table once for every decision kind.
  test.describe.configure({ timeout: 300_000 })

  for (const [width, height] of [
    [1400, 900],
    [1280, 720],
  ] as const) {
    for (const [table, side] of RUNS) {
      test(`one home and nothing moves on ${table}, column ${side}, at ${width}x${height}`, async ({
        page,
      }) => {
        await page.setViewportSize({ width, height })
        const quiet = await show(page, table, null, side)
        expect(quiet.strip?.[3], 'one line').toBe(36)
        for (const prompt of PROMPTS.slice(1) as string[]) {
          const shown = await show(page, table, prompt, side)
          const label = `${table} ${prompt} ${side} ${width}x${height}`
          expect(shown.homes, `${label}: one home`).toEqual([homeOf(prompt, side)])
          expect(shown.hidden, `${label}: answers in view`).toEqual([])
          expect(shown.homeScrolls, `${label}: the dock scrolls`).toBe(false)
          expect(shown.sheet, `${label}: no sheet off a phone`).toBeNull()
          if (shown.overlay) continue
          expect(same(shown.strip, quiet.strip), `${label}: the strip moved, ${shown.strip}`).toBe(true)
          for (const [name, box] of Object.entries(shown.still)) {
            // A payment opens your piles of equal lands, so each can be tapped: the cards of a full board size anew.
            if (prompt === 'pay' && name === 'own card') continue
            // A dock grown over the corner of your board sizes your cards anew to keep them clear of it; they stay in place.
            if (name === 'own card' && shown.dockOver && box !== null) {
              const was = quiet.still[name] ?? null
              const corner = (at: Box): Box => [at[0], at[1], 0, 0]
              expect(same(corner(box), was === null ? null : corner(was)), `${label}: own card moved`).toBe(
                true,
              )
              continue
            }
            expect(
              same(box, quiet.still[name] ?? null),
              `${label}: ${name} moved, ${box} against ${quiet.still[name]}`,
            ).toBe(true)
          }
        }
      })
    }
  }

  test('brings the Stack tab forward for a card`s question, answers under its card, and gives the tab back', async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1400, height: 900 })
    await show(page, 'seats=2&board=duel', 'ask-may')
    await expect(page.getByRole('tab', { name: /^Stack/ })).toHaveAttribute('aria-selected', 'true')
    const line = page.locator('[data-slot="decision-panel"] [data-slot="decision-line"]')
    await expect(line).toHaveAttribute('data-home', 'panel')
    await expect(page.locator('[data-slot="decision-detail"] [data-slot="asked-clause"]')).toHaveText(
      /you may pay .*If you do, draw a card\./,
    )
    // The dock says where to answer; A takes the focus there.
    await expect(page.locator('[data-slot="answer-jump"]')).toContainText('Answer under')
    await page.keyboard.press('n')
    await expect(page.getByRole('tab', { name: /^Log/ })).toHaveAttribute('aria-selected', 'true')
  })

  test('answers every kind in the bar on a phone, the payment in a sheet of at most 330 px', async ({
    page,
  }) => {
    await page.setViewportSize({ width: 390, height: 844 })
    const quiet = await show(page, 'seats=2&board=duel', null)
    expect(quiet.strip?.[3]).toBe(28)
    for (const prompt of [...(PROMPTS.slice(1) as string[]), 'priority', 'attack', 'block', 'mulligan']) {
      const shown = await show(page, 'seats=2&board=duel', prompt)
      const label = `${prompt} on a phone`
      expect(shown.homes, label).toEqual(['bar'])
      expect(shown.hidden, label).toEqual([])
      expect(
        same(shown.still['controls'] ?? null, quiet.still['controls'] ?? null),
        `${label}: the bar moved`,
      ).toBe(true)
      // A window over the boards trades the strip with your board; the game moments of combat and the opening hand are tables of their own.
      if (!shown.overlay && !GAME_ASKS.has(prompt))
        expect(same(shown.strip, quiet.strip), `${label}: the strip moved`).toBe(true)
      if (prompt === 'pay') {
        expect(shown.sheet, label).not.toBeNull()
        expect(shown.sheet?.[3] ?? 0, label).toBeLessThanOrEqual(331)
      } else expect(shown.sheet, label).toBeNull()
    }
    // The handle folds the payment's sheet.
    await show(page, 'seats=2&board=duel', 'pay')
    const handle = page.locator('[data-slot="sheet-grab"]')
    await handle.click()
    await expect(handle).toHaveAttribute('aria-expanded', 'false')
  })
})
