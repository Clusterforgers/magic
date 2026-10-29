// Goal: the driver's own helpers read a prompt as the pages draw it: the turn of a banner, the cards a Priority window marks legal in the hand, the command zone and the battlefield, which windows count as idle, and which defender an attack aims at.
import { expect, test } from '@playwright/test'
import { aimAt, isIdle, look, turnOf } from './driver'
import type { Card, Seen } from './driver'

const card = (id: string): Card => ({ id, label: `Card ${id}`, land: false })

const priority = (legal: Partial<Pick<Seen, 'hand' | 'command' | 'board'>>): Seen => ({
  over: false,
  kind: 'Priority',
  id: '1',
  banner: "Ada's turn · 3 · End step",
  text: 'You have priority',
  buttons: [],
  hand: [],
  command: [],
  board: [],
  ...legal,
})

test.describe('driver helpers', () => {
  test('the turn is the number between the dots of the banner', () => {
    expect(turnOf("Ada's turn · 5 · End step")).toBe(5)
    expect(turnOf('Your turn · 12 · Precombat main')).toBe(12)
    expect(turnOf('Opening hands')).toBe(0)
  })

  const windows: { name: string; seen: Seen; idle: boolean }[] = [
    { name: 'nothing is legal', seen: priority({}), idle: true },
    { name: 'a hand card is legal', seen: priority({ hand: [card('1')] }), idle: false },
    { name: 'a commander can be cast', seen: priority({ command: [card('2')] }), idle: false },
    {
      name: 'only an ability on the battlefield is legal',
      seen: priority({ board: [card('3')] }),
      idle: false,
    },
    {
      name: 'the decision is not a priority window',
      seen: { ...priority({}), kind: 'DeclareBlockers' },
      idle: false,
    },
  ]
  for (const { name, seen, idle } of windows) {
    test(`a window is ${idle ? '' : 'not '}idle when ${name}`, () => {
      expect(isIdle(seen)).toBe(idle)
    })
  }

  test('a prompt is read with its legal cards in the hand, the command zone and the battlefield', async ({
    page,
  }) => {
    await page.setContent(`
      <h2 data-slot="banner">Your turn · 5 · Precombat main</h2>
      <div data-decision="Priority" data-decision-id="7">
        <p data-slot="prompt-text">You have priority</p><button>Next: Combat</button><button aria-label="Attack with all" disabled><span>All</span></button>
      </div>
      <ul aria-label="Ada's hand">
        <li><button data-object-id="1" data-highlight="legal" aria-label="Forest, Basic Land">1</button></li>
        <li><button data-object-id="5" aria-label="Plains, Basic Land">5</button></li>
      </ul>
      <ul aria-label="Ada's commanders in the command zone">
        <li><button data-object-id="2" data-highlight="legal" aria-label="Emmara, Soul of the Accord">2</button></li>
      </ul>
      <section aria-label="Ada's battlefield">
        <button data-object-id="3" data-highlight="legal" aria-label="Sol Ring">3</button>
        <button data-object-id="4" aria-label="Swamp, Basic Land">4</button>
      </section>`)
    const seen = (await page.evaluate(look, { me: 'Ada', answered: null })) as Seen
    expect(seen.kind).toBe('Priority')
    // A button is named as the page names it: the combat bar's "All" is "Attack with all".
    expect(seen.buttons).toEqual([
      { label: 'Next: Combat', disabled: false },
      { label: 'Attack with all', disabled: true },
    ])
    expect(seen.hand).toEqual([{ id: '1', label: 'Forest, Basic Land', land: true }])
    expect(seen.command.map((c) => c.id)).toEqual(['2'])
    expect(seen.board.map((c) => c.id)).toEqual(['3'])
    expect(isIdle(seen)).toBe(false)
    // The prompt that was answered is not read again.
    expect(await page.evaluate(look, { me: 'Ada', answered: '7' })).toBeNull()
  })

  test('an attack aims at the first wanted player the list offers', async ({ page }) => {
    await page.setContent(`
      <div data-decision="DeclareAttackers" data-decision-id="3">
        <select aria-label="Attack at"><option value="p:0">Ada</option><option value="p:2">Cy</option><option value="p:3">Di</option></select>
      </div>`)
    const list = page.getByLabel('Attack at')
    expect(await aimAt(page, ['Bo', 'Cy', 'Ada'])).toBe('Cy')
    await expect(list).toHaveValue('p:2')
    // Nobody wanted is offered: the table's pick stays.
    expect(await aimAt(page, ['Bo'])).toBeNull()
    await expect(list).toHaveValue('p:2')
    expect(await aimAt(page, [])).toBeNull()
  })

  test('an attack with one defender has no list to aim with', async ({ page }) => {
    await page.setContent(
      '<div data-decision="DeclareAttackers" data-decision-id="3"><button>Attack with all</button></div>',
    )
    expect(await aimAt(page, ['Ada'])).toBeNull()
  })
})
