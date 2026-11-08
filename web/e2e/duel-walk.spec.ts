// Goal: in a real two-seat game of two white-blue decks the seats walk a counterspell, a combat trick and a sacrifice by hand, and each step's prompt, stack chips and log lines read right in both seats, the hidden card named only to its owner.
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { createAndJoin, everyoneReady } from './lobby'
import type { Player } from './lobby'
import { apply, logOf, openRow } from './manual'

const releaseFocus = (page: Page) =>
  page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur())

/** The sentence of the seat's prompt. */
const sentence = (player: Player) => player.page.locator('[data-decision] [data-slot="prompt-text"]')

/** The accessible names of the stack's chips, top first. */
const stack = (player: Player) =>
  player.page
    .locator('[data-stack-chip]')
    .evaluateAll((chips) => chips.map((chip) => chip.getAttribute('aria-label')))

/** The turn banner: whose turn, which turn and which step. */
const banner = (player: Player) => player.page.locator('[data-slot="banner"]')

/** Presses the primary button of the prompt: pass priority, or whatever the moment calls the pass. */
const pass = (player: Player) =>
  player.page.locator('[data-region="controls"] [data-slot="primary-action"]').click()

const handCard = (player: Player, name: string) =>
  player.page
    .locator(`ul[aria-label="${player.name}'s hand"] button[data-object-id][aria-label^="${name}"]`)
    .first()

const ownCard = (player: Player, name: string) =>
  player.page
    .locator(
      `section[aria-label="${player.name}'s battlefield"] button[data-object-id][aria-label^="${name}"]`,
    )
    .first()

/** Searches the library for the card by hand and takes it into the hand. */
async function tutor(player: Player, name: string): Promise<void> {
  const row = await openRow(player.page, 'Search for a card')
  await row.locator('[data-slot="command-apply"]').click()
  const grid = player.page.locator('[data-slot="tutor-grid"]')
  await grid
    .getByRole('button', { name: new RegExp(`^${name}`) })
    .first()
    .click()
  await player.page.locator('[data-slot="command-apply"]').click()
  await expect(grid).toHaveCount(0)
  await expect(handCard(player, name)).toBeVisible()
}

/** Adds mana of one kind to the pool by hand. */
async function addMana(player: Player, kind: string, count: number): Promise<void> {
  const row = await openRow(player.page, 'Mana')
  await row.getByRole('combobox', { name: 'Mana' }).selectOption(kind)
  await row.getByRole('button', { name: String(count), exact: true }).click()
  await apply(row, /^Add/)
}

/** Casts a hand card, choosing `target` in the prompt when it has one, and pays from the pool. */
async function cast(player: Player, name: string, target?: string): Promise<void> {
  await handCard(player, name).click()
  if (target !== undefined) {
    await player.page.locator('[data-decision] button', { hasText: target }).click()
    await player.page.getByRole('button', { name: 'Confirm targets' }).click()
  }
  await player.page.getByRole('button', { name: 'Pay automatically' }).click()
  await expect(sentence(player)).toContainText(`You are responding to your ${name}`)
}

test('a counterspell, a combat trick and a sacrifice by hand, both seats reading each step', async ({
  browser,
}) => {
  test.setTimeout(6 * 60_000)
  const players = await createAndJoin(browser, [
    { name: 'Ada', deck: 'white-blue' },
    { name: 'Bo', deck: 'white-blue' },
  ])
  try {
    const [ada, bo] = players as [Player, Player]
    await everyoneReady(players)
    await Promise.all(
      players.map(({ page }) => page.getByRole('button', { name: 'Keep', exact: true }).click()),
    )
    // Ada has the first turn of the pinned seed. Both seats hold Full Control, so each is asked at every window.
    await expect(sentence(ada)).toContainText('You have priority')
    await expect(banner(ada)).toContainText('Your turn · 1 · Precombat main')
    for (const { page } of players) await releaseFocus(page)
    for (const { page } of players) await page.keyboard.press('Control+Shift')
    await expect(bo.page.getByRole('button', { name: 'Full Control: held' })).toBeVisible()

    // 1. A counterspell. Ada casts a creature; Bo answers with Counterspell on top of it.
    await tutor(ada, 'Silvercoat Lion')
    await addMana(ada, 'White', 2)
    await cast(ada, 'Silvercoat Lion')
    const lion = 'Silvercoat Lion, cost 1 generic, white, Creature, power 2, toughness 2, cast by Ada'
    await expect.poll(() => stack(ada)).toEqual([`Top of the stack: ${lion}`])
    await pass(ada)
    await expect(sentence(bo)).toContainText("You are responding to Ada's Silvercoat Lion")
    await expect(banner(bo)).toContainText("Ada's turn · 1 · Precombat main")
    await tutor(bo, 'Counterspell')
    await addMana(bo, 'Blue', 2)
    await cast(bo, 'Counterspell', 'Silvercoat Lion')
    const counter =
      'Top of the stack: Counterspell, cost blue, blue, Instant, cast by Bo, targeting Silvercoat Lion'
    for (const player of players) await expect.poll(() => stack(player)).toEqual([counter, lion])
    await pass(bo)
    await expect(sentence(ada)).toContainText("You are responding to Bo's Counterspell")
    await pass(ada)
    await expect(sentence(ada)).toContainText('You have priority')
    for (const player of players) await expect.poll(() => stack(player)).toEqual([])
    const countered = [
      /Ada casts Silvercoat Lion/,
      /Bo casts Counterspell targeting Silvercoat Lion/,
      /Counterspell counters Silvercoat Lion/,
    ]
    const adaLog = await logOf(ada.page, [...countered, /Bo puts a card from their library into their hand/])
    const boLog = await logOf(bo.page, [
      ...countered,
      /Bo puts Counterspell from their library into their hand/,
    ])
    // A tutored card is named to its owner only.
    expect(adaLog).not.toMatch(/Bo puts Counterspell/)
    expect(boLog).not.toMatch(/Ada puts Silvercoat Lion/)

    // 2. A combat trick. Ada attacks with a creature Bo blocks, and removes the blocker at instant speed.
    await tutor(ada, 'Pearled Unicorn')
    await addMana(ada, 'White', 3)
    await cast(ada, 'Pearled Unicorn')
    await pass(ada)
    await expect(sentence(bo)).toContainText("You are responding to Ada's Pearled Unicorn")
    await pass(bo)
    await expect(sentence(ada)).toContainText('You have priority')
    await ownCard(ada, 'Pearled Unicorn').click({ button: 'right' })
    await ada.page.locator('[data-item="unsick"]').click()
    const tokens = await openRow(bo.page, 'Tokens')
    await tokens.getByRole('combobox', { name: 'Token' }).selectOption('Soldier')
    await tokens.getByRole('button', { name: '1', exact: true }).click()
    await apply(tokens, /^Create 1 Soldier/)
    await pass(ada)
    await expect(sentence(bo)).toContainText('You have priority')
    await pass(bo)
    await expect(banner(ada)).toContainText('Beginning of combat')
    await pass(ada)
    await expect(sentence(bo)).toContainText('You have priority')
    await pass(bo)
    await expect(sentence(ada)).toContainText('Choose the creatures that attack Bo')
    await ada.page.getByRole('button', { name: 'Attack with all', exact: true }).click()
    await ada.page.getByRole('button', { name: 'Confirm attack', exact: true }).click()
    await expect(banner(ada)).toContainText('Declare attackers')
    await pass(ada)
    await expect(sentence(bo)).toContainText('You have priority')
    await pass(bo)
    await expect(sentence(bo)).toContainText('Click a blocker, then the attacker it blocks')
    await ownCard(bo, 'Soldier').click()
    await bo.page.getByRole('button', { name: 'Confirm blocks' }).click()
    await expect(banner(ada)).toContainText('Declare blockers')
    await expect(sentence(ada)).toContainText('You have priority')
    await tutor(ada, 'Swords to Plowshares')
    await addMana(ada, 'White', 1)
    await cast(ada, 'Swords to Plowshares', 'Soldier')
    const swords =
      'Top of the stack: Swords to Plowshares, cost white, Instant, cast by Ada, targeting Soldier'
    for (const player of players) await expect.poll(() => stack(player)).toEqual([swords])
    await pass(ada)
    await expect(sentence(bo)).toContainText("You are responding to Ada's Swords to Plowshares")
    await pass(bo)
    await expect(sentence(ada)).toContainText('You have priority')
    for (const player of players) await expect.poll(() => stack(player)).toEqual([])
    await expect(ownCard(bo, 'Soldier')).toHaveCount(0)
    const trick = [
      /Ada attacks Bo with Pearled Unicorn/,
      /Bo declares blockers: Soldier blocks Pearled Unicorn/,
      /Ada casts Swords to Plowshares targeting Soldier/,
      /Soldier is exiled by Swords to Plowshares/,
    ]
    await logOf(ada.page, trick)
    await logOf(bo.page, trick)

    // 3. A sacrifice by hand, from the card's menu; both logs say it and the card leaves the battlefield.
    await ownCard(ada, 'Pearled Unicorn').click({ button: 'right' })
    await ada.page.locator('[data-item="sacrifice"]').click()
    await expect(ownCard(ada, 'Pearled Unicorn')).toHaveCount(0)
    await expect(
      bo.page.locator(
        `section[aria-label="Ada's battlefield"] button[data-object-id][aria-label^="Pearled Unicorn"]`,
      ),
    ).toHaveCount(0)
    await logOf(ada.page, [/Ada sacrifices Pearled Unicorn \(by hand\)/])
    await logOf(bo.page, [/Ada sacrifices Pearled Unicorn \(by hand\)/])

    for (const player of players) expect(player.errors, `${player.name}'s page errors`).toEqual([])
  } finally {
    await Promise.all(players.map(({ context }) => context.close()))
  }
})
