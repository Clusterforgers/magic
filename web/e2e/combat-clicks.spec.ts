// Goal: in a real game against a real server, at two seats and at four, a seat declares an attack creature by creature with clicks (a creature, then the player it attacks) and the defender blocks with clicks (a creature, then the attacker), each prompt marking the cards and pods it says it marks, every seat's page reading the block, and no page error.
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { attackersOffered, playToTheEnd, Run } from './driver'
import type { Hooks, Style } from './driver'
import { createAndJoin, everyoneReady } from './lobby'
import type { Player } from './lobby'

/** Decisions the seats may answer before the run is called stuck. */
const MAX_DECISIONS = 2_500
/** Time with no decision answered by any seat before the run is called stuck. */
const STALL_MS = 30_000

const DECKS = ['mono-green', 'mono-red', 'mono-green', 'mono-red']
const NAMES = ['Ada', 'Bo', 'Cy', 'Di']

const card = (page: Page, id: string) => page.locator(`button[data-object-id="${id}"]`)
const prompt = (page: Page) => page.locator('[data-decision]')
const button = (page: Page, name: string) => prompt(page).getByRole('button', { name, exact: true })
/** The pod (or the tile of a crowded table) of a seat. */
const pod = (page: Page, seat: number) =>
  page.locator(`section[data-seat="${seat}"], button[data-tile-seat="${seat}"]`)

/** What the clicks did: the last attack and the block that answered it. */
interface Found {
  attack?: { by: number; at: number; creature: string }
  block?: { by: number; blocker: string; attacker: string }
}

/** The hooks that declare the attack and the block by clicks until the first block is declared. */
function clicking(players: readonly Player[], found: Found): Hooks {
  const seats = players.length
  return {
    async answer(player, seen, run) {
      const { page } = player
      const index = players.indexOf(player)
      if (seen.kind === 'DeclareAttackers') {
        const [first] = seen.board
        if (first === undefined) return undefined
        // The seat before this one in turn order: at two seats the only opponent, at four a seat that is not next.
        const at = (index + seats - 1) % seats
        const several = seats > 2
        await card(page, first.id).click()
        await expect(card(page, first.id)).toHaveAttribute('data-highlight', 'selected')
        if (several) {
          // The creature waits for a click on the player it attacks; the click sends it there.
          await expect(prompt(page)).toContainText('Click a player')
          await pod(page, at).click()
          await expect(pod(page, at)).toHaveAttribute('data-defender', 'chosen')
        }
        // A second click takes the creature out again: nothing is declared and Confirm waits.
        await card(page, first.id).click()
        await expect(card(page, first.id)).toHaveAttribute('data-highlight', 'legal')
        await expect(button(page, 'Confirm attack')).toBeDisabled()
        await card(page, first.id).click()
        await expect(card(page, first.id)).toHaveAttribute('data-highlight', 'selected')
        if (several) {
          await pod(page, at).click()
          await expect(pod(page, at)).toHaveAttribute('data-defender', 'chosen')
        }
        await button(page, 'Confirm attack').click()
        found.attack = { by: index, at: several ? at : (index + 1) % seats, creature: first.id }
        return `attacks seat ${found.attack.at} with ${first.label.split(',')[0]} by click`
      }
      if (seen.kind === 'DeclareBlockers' && found.attack !== undefined && seen.board.length > 0) {
        const own = seen.board.map((blocker) => blocker.id)
        const [blocker] = seen.board
        if (blocker === undefined) return undefined
        await card(page, blocker.id).click()
        await expect(card(page, blocker.id)).toHaveAttribute('data-highlight', 'selected')
        // A creature that may block several attackers waits for a click on the one it blocks.
        const asking = page.getByRole('combobox', { name: /^Attacker .* blocks$/ })
        let attacker = found.attack.creature
        if ((await asking.count()) > 0) {
          const offered = await page.evaluate(attackersOffered, own)
          const wanted = offered.includes(attacker) ? attacker : offered[0]
          if (wanted === undefined) throw new Error('the chosen blocker offers no attacker to block')
          attacker = wanted
          await card(page, attacker).click()
          await asking.waitFor({ state: 'detached' })
        }
        await button(page, 'Confirm blocks').click()
        found.block = { by: index, blocker: blocker.id, attacker }
        run.done = true
        return `blocks ${attacker} with ${blocker.label.split(',')[0]} by click`
      }
      return undefined
    },
  }
}

for (const seats of [2, 4]) {
  test(`${seats} seats: an attack and a block declared by clicks reach every seat`, async ({
    browser,
  }, testInfo) => {
    test.setTimeout(5 * 60_000)
    const players = await createAndJoin(
      browser,
      NAMES.slice(0, seats).map((name, index) => ({ name, deck: DECKS[index] ?? 'mono-red' })),
    )
    const run = new Run(MAX_DECISIONS, STALL_MS)
    const found: Found = {}
    // Nobody blocks except by the hook, and every seat pays automatically and passes quickly.
    const style: Style = { blocks: false, autoPay: true, yields: true }
    try {
      await everyoneReady(players)
      await playToTheEnd(
        players.map((player) => ({ player, style })),
        run,
        clicking(players, found),
      )
      const { attack, block } = found
      if (attack === undefined || block === undefined) throw new Error('no attack was blocked')
      // Only the seat that was attacked is asked to block.
      expect(block.by, 'the seat that blocked is the one that was attacked').toBe(attack.at)
      // The block reaches every page: the log of each seat names a block.
      for (const { page, name } of players) {
        await page.getByRole('tab', { name: /^Log\b/ }).click()
        await expect(page.locator('[role="log"]').first(), `${name}'s log shows the block`).toContainText(
          /block/i,
          { timeout: 20_000 },
        )
      }
      for (const player of players) expect(player.errors, `${player.name}'s page errors`).toEqual([])
    } catch (error) {
      await testInfo.attach('trail.txt', { body: run.trail.join('\n') || '(no decision was answered)' })
      for (const { name, page } of players) {
        await testInfo.attach(`${name}.png`, { body: await page.screenshot(), contentType: 'image/png' })
      }
      throw error
    } finally {
      console.log(`combat clicks (${seats} seats): ${run.summary()}`)
      await Promise.all(players.map(({ context }) => context.close()))
    }
  })
}
