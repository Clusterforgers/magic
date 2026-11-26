// Goal: the development routes of the deck list and its groups, the open games on Play and the replay draw their real pages without a server and keep the layout rules at 1400x900 and 390x844: no sideways scroll or cut-off box, no text under 12 px, the primary action in the window and uncovered, and a page meant to fit (Play, the replay, the decks from the sm width) never scrolls down; none throws.
import { expect, test } from './test'
import type { Page } from '@playwright/test'
import { measure, settle, TABLE_EXEMPT, TEXT_FLOOR } from './layoutAudit'
import type { Ask } from './layoutAudit'
import { DEV_WEB_URL } from './ports'

/** A development page, what it must keep and how to know it is drawn. */
interface Stop {
  name: string
  path: string
  /** From this width on the page fits the window and never scrolls down. */
  fitsFrom: number
  actions: Record<string, string>
  exempt?: readonly string[]
  ready(page: Page): Promise<void>
}

const STOPS: Stop[] = [
  {
    name: 'the decks in groups',
    path: '/dev/decks',
    fitsFrom: 640,
    actions: { 'Import deck': 'text:Import deck' },
    ready: (page) => expect(page.getByRole('list', { name: 'Decks in Fun' })).toBeVisible(),
  },
  {
    name: 'the decks without groups',
    path: '/dev/decks?view=list',
    fitsFrom: 640,
    actions: { 'Import deck': 'text:Import deck' },
    ready: (page) => expect(page.getByRole('list', { name: 'Your decks' })).toBeVisible(),
  },
  {
    name: 'the open games',
    path: '/dev/play',
    fitsFrom: 0,
    actions: { 'Create game': 'text:Create game' },
    ready: (page) => expect(page.getByRole('list', { name: 'Open games, newest first' })).toBeVisible(),
  },
  {
    name: 'many open games with games to continue',
    path: '/dev/play?view=many&account=1',
    fitsFrom: 0,
    actions: { 'Create game': 'text:Create game' },
    ready: (page) => expect(page.getByRole('switch', { name: 'Hide full games' })).toBeVisible(),
  },
  {
    name: 'the replay of a duel at its end',
    path: '/dev/replay?step=last',
    fitsFrom: 0,
    actions: { 'First step': '[data-slot="replay-controls"] [aria-label="First step"]' },
    exempt: TABLE_EXEMPT,
    ready: (page) => expect(page.getByRole('toolbar', { name: 'Replay' })).toBeVisible(),
  },
  {
    name: 'the replay of a four-seat Commander game',
    path: '/dev/replay?game=commander',
    fitsFrom: 0,
    actions: { 'Step forward': '[data-slot="replay-controls"] [aria-label="Step forward"]' },
    exempt: TABLE_EXEMPT,
    ready: (page) => expect(page.getByRole('toolbar', { name: 'Replay' })).toBeVisible(),
  },
]

const SIZES = [
  { width: 1400, height: 900 },
  { width: 390, height: 844 },
] as const

for (const stop of STOPS) {
  for (const size of SIZES) {
    test(`${stop.name} keeps the layout rules at ${size.width}x${size.height}`, async ({ browser }) => {
      // The pages other than the table read their width when they mount, so each size opens the page anew.
      const context = await browser.newContext({ viewport: size })
      const page = await context.newPage()
      const errors: string[] = []
      page.on('pageerror', (error) => errors.push(error.message))
      await page.goto(`${DEV_WEB_URL}${stop.path}`)
      await stop.ready(page)
      const fits = size.width >= stop.fitsFrom
      const ask: Ask = {
        actions: stop.actions,
        reveal: !fits,
        exempt: [...(stop.exempt ?? [])],
        floor: TEXT_FLOOR,
      }
      const problems = await settle(() => page.evaluate(measure, ask), { fits })
      expect(problems, `${stop.name} at ${size.width}x${size.height}`).toEqual([])
      expect(errors, 'uncaught exceptions').toEqual([])
    })
  }
}
