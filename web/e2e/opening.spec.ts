import { expect, test } from '@playwright/test'
import { createAndJoin, everyoneReady } from './lobby'

test('two seats create, join by link and are dealt opening hands', async ({ browser }) => {
  const players = await createAndJoin(browser, [
    { name: 'Ada', deck: 'mono-green' },
    { name: 'Bo', deck: 'mono-red' },
  ])
  try {
    await everyoneReady(players)
    for (const { name, page } of players) {
      await expect(page.getByRole('heading', { name: 'Opening hands' })).toBeVisible()
      await expect(page.getByRole('list', { name: `${name}'s hand` }).getByRole('listitem')).toHaveCount(7)
    }
  } finally {
    await Promise.all(players.map(({ context }) => context.close()))
  }
})
