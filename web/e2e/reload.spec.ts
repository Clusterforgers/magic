// Goal: a seat that reloads its page mid-game gets the game log back, as the seat that stayed saw it, and the table at rest (nothing animates from a snapshot: no effect, no cross-fade, no picture fading in), at tables of two and four seats.
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { playToTheEnd, Run, turnOf } from './driver'
import type { Hooks } from './driver'
import { createAndJoin, everyoneReady } from './lobby'
import { SHOTS_DIR } from './table'

/** The turn at which every seat stops playing and the first one reloads. */
const RELOAD_AT_TURN = 4
const MAX_DECISIONS = 900
const STALL_MS = 30_000
const NAMES = ['Ada', 'Bo', 'Cy', 'Di']
const DECKS = ['mono-green', 'mono-red']
/** How long after the table is back the page is watched for animations. */
const REST_MS = 1_500

/** What the page keeps from the first moment of its document: every animation that ran, by kind, name and target. */
function recordAnimations(): void {
  const started: string[] = []
  Object.assign(window, { __started: started })
  const seen = new Set<string>()
  setInterval(() => {
    for (const animation of document.getAnimations()) {
      const target = (animation.effect as KeyframeEffect | null)?.target as HTMLElement | null
      const name =
        (animation as CSSAnimation).animationName ?? (animation as CSSTransition).transitionProperty
      const key = `${animation.constructor.name} ${name} ${target?.dataset?.['objectId'] ?? target?.tagName ?? ''}`
      if (seen.has(key)) continue
      seen.add(key)
      started.push(key)
    }
  }, 20)
}

/**
 * What a table plays for something that happened or that loaded: the effects of the fx layer, the cross-fade of the
 * focus board and anything on a card or its picture. The spinner of a pending button spins whenever it shows.
 */
const NOT_AT_REST = /^CSS(Animation (fx-|focus-board)|Transition .* (BUTTON|IMG|SPAN|LI|DIV)$)/

const started = (page: Page): Promise<string[]> =>
  page.evaluate(() => (window as unknown as { __started: string[] }).__started)

/** The lines of the game log, in order, each with its kind (`turn`, `step` or `play`). */
const logLines = (page: Page): Promise<string[]> =>
  page
    .getByRole('log', { name: 'Game log' })
    .locator('li')
    .evaluateAll((items) =>
      items.map((item) => `${item.getAttribute('data-kind')}: ${(item.textContent ?? '').trim()}`),
    )

for (const seats of [2, 4]) {
  test(`a seat at a table of ${seats} that reloads mid-game gets its game log back and sees the table at rest`, async ({
    browser,
  }) => {
    test.setTimeout(5 * 60_000)
    const players = await createAndJoin(
      browser,
      NAMES.slice(0, seats).map((name, index) => ({
        name,
        deck: DECKS[index % DECKS.length] ?? 'mono-green',
      })),
    )
    const ada = players[0]
    if (ada === undefined) throw new Error('a seat expected')
    try {
      await everyoneReady(players)
      const hooks: Hooks = {
        async before(_player, seen, run) {
          if (turnOf(seen.banner) >= RELOAD_AT_TURN) run.done = true
        },
      }
      await playToTheEnd(
        players.map((player) => ({ player, style: { blocks: true, autoPay: true, yields: true } })),
        new Run(MAX_DECISIONS, STALL_MS),
        hooks,
      )
      await ada.page.getByRole('tab', { name: /^Log/ }).click()
      const before = await logLines(ada.page)
      expect(before.length, 'Ada saw the game before the reload').toBeGreaterThan(20)
      expect(before.filter((line) => line.startsWith('turn:')).length).toBeGreaterThanOrEqual(
        RELOAD_AT_TURN - 1,
      )

      // Everything from here on is recorded from the first moment of the new document.
      await ada.page.addInitScript(recordAnimations)
      await ada.page.reload()
      await expect(ada.page.locator('[data-slot="banner"]')).toBeVisible()
      // A snapshot is a state, not an event: every permanent is simply there, nothing enters, fades or overlaps.
      await ada.page.waitForTimeout(REST_MS)
      const played = (await started(ada.page)).filter((key) => NOT_AT_REST.test(key))
      expect(played, 'animations after a reload').toEqual([])

      await ada.page.getByRole('tab', { name: /^Log/ }).click()
      await expect(ada.page.getByText('Nothing has happened yet.')).toHaveCount(0)
      const after = await logLines(ada.page)
      if (SHOTS_DIR !== undefined)
        await ada.page.screenshot({ path: `${SHOTS_DIR}/reload-${seats}-after.png` })
      // The same turns, in the same heads; a card that left the view since reads "a card" in the rebuilt lines, so the
      // rest is compared as a share.
      const turns = (lines: string[]) => lines.filter((line) => line.startsWith('turn:'))
      expect(turns(after)).toEqual(turns(before))
      const missing = before.filter((line) => !after.includes(line))
      expect(missing.length, `lines the reload lost:\n${missing.join('\n')}`).toBeLessThanOrEqual(
        before.length * 0.1,
      )
      expect(
        after.some((line) => /^play: Ada (plays|casts) /.test(line)),
        "Ada's own plays",
      ).toBe(true)
      for (const player of players) expect(player.errors, `${player.name}'s page errors`).toEqual([])
    } finally {
      await Promise.all(players.map(({ context }) => context.close()))
    }
  })
}
