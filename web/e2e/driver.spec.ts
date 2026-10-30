// Goal: the driver's own helpers read a prompt as the pages draw it: which cards of a hand are for casting and which for an ability alone, how a click on a card with an ability is answered in the chooser, the turn of a banner, the cards a Priority window marks legal in the hand, the command zone and the battlefield, which windows count as idle, which defender an attack aims at, and that a play which comes back to its window unanswered is tried twice there.
import { expect, test } from '@playwright/test'
import {
  aimAt,
  attackersOffered,
  hasAbility,
  isIdle,
  isPlayable,
  look,
  pickPlay,
  Run,
  turnOf,
} from './driver'
import type { Card, Seen } from './driver'

const card = (id: string): Card => ({ id, label: `Card ${id}`, land: false })
const land = (id: string): Card => ({ id, label: `Forest ${id}, Basic Land`, land: true })

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
      name: 'only untapped lands are legal, for their mana abilities',
      seen: priority({ board: [land('4'), land('5')] }),
      idle: true,
    },
    {
      name: 'an untapped land and an ability on the battlefield are legal',
      seen: priority({ board: [land('4'), card('3')] }),
      idle: false,
    },
    {
      name: 'a land is legal in the hand',
      seen: priority({ hand: [land('1')], board: [land('4')] }),
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

  const labelled = (label: string): Card => ({ id: '1', label, land: false })
  const named: { label: string; ability: boolean; playable: boolean }[] = [
    { label: 'Grizzly Bears, Creature, available', ability: false, playable: true },
    { label: 'Cycler, Creature, available, Activate one of 2 abilities', ability: true, playable: true },
    {
      label: 'Cycler, Creature, available, Activate Cycling 2, not castable: needs mana',
      ability: true,
      playable: false,
    },
    { label: 'Mind Stone, Artifact, not available, needs mana', ability: false, playable: true },
  ]
  for (const { label, ability, playable } of named) {
    test(`"${label}" ${ability ? 'has' : 'has no'} ability and is ${playable ? '' : 'not '}for casting`, () => {
      expect(hasAbility(labelled(label))).toBe(ability)
      expect(isPlayable(labelled(label))).toBe(playable)
    })
  }

  // The chooser the table opens beside a card with an ability, and the decision that follows a pick.
  const table = (chooser: boolean): string => `
    <div data-decision="Priority" data-decision-id="7"><p data-slot="prompt-text">You have priority</p></div>
    ${
      chooser
        ? `<div data-slot="ability-chooser" role="menu">
            <button role="menuitem" data-play="cast" onclick="document.querySelector('[data-decision]').dataset.decisionId = '8'; this.parentElement.remove()">Cast Cycler</button>
            <button role="menuitem" data-play="ability-0-null" onclick="document.body.dataset.wrong = 'yes'">Cycling {2}</button>
          </div>`
        : ''
    }`

  test('a chooser beside a cast is answered with the cast', async ({ page }) => {
    await page.setContent(table(true))
    expect(await pickPlay(page, '7', 'cast')).toBe(' from the chooser')
    expect(await page.locator('[data-decision]').getAttribute('data-decision-id')).toBe('8')
    expect(await page.locator('body').getAttribute('data-wrong')).toBeNull()
  })

  test('a click that answered at once opens no chooser, and nothing more is done', async ({ page }) => {
    await page.setContent(table(false))
    await page.evaluate(() => {
      document.querySelector<HTMLElement>('[data-decision]')?.setAttribute('data-decision-id', '8')
    })
    expect(await pickPlay(page, '7', 'cast')).toBe('')
  })

  test('a click that does neither is left to the run after the wait', async ({ page }) => {
    await page.setContent(table(false))
    expect(await pickPlay(page, '7', 'land')).toBe('')
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

  test('a chosen blocker is offered the attackers the table marks legal, not the blockers', async ({
    page,
  }) => {
    await page.setContent(`
      <section aria-label="Bo's battlefield">
        <button data-object-id="21" data-highlight="legal">Wolf</button>
        <button data-object-id="22" data-highlight="dimmed">Bear</button>
        <button data-object-id="23" data-highlight="legal">Elk</button>
      </section>
      <section aria-label="Ada's battlefield">
        <button data-object-id="11" data-highlight="selected">Chosen</button>
        <button data-object-id="12" data-highlight="legal">Other blocker</button>
        <button data-object-id="13" data-highlight="dimmed">Land</button>
      </section>`)
    expect(await page.evaluate(attackersOffered, ['11', '12'])).toEqual(['21', '23'])
    expect(await page.evaluate(attackersOffered, ['11', '12', '21', '23'])).toEqual([])
  })
})

test.describe('a play that comes back to its window unanswered', () => {
  const window = "Cy's turn · 19 · Precombat main"
  const spell: Card = { id: '7', label: 'Spectral Steel, Enchantment, Aura', land: false }

  test('is tried twice there, then left alone and reported once', () => {
    const run = new Run(10, 1_000)
    expect(run.fresh(spell, window)).toBe(true)
    run.tried('Cy', spell, window)
    expect(run.fresh(spell, window)).toBe(true)
    run.tried('Cy', spell, window)
    expect(run.fresh(spell, window)).toBe(false)
    expect(run.unanswered).toEqual([`Cy: Spectral Steel at ${window}`])
  })

  test('is tried again in the next window, and never after the run skipped it', () => {
    const run = new Run(10, 1_000)
    run.tried('Cy', spell, window)
    run.tried('Cy', spell, window)
    expect(run.fresh(spell, "Cy's turn · 19 · Postcombat main")).toBe(true)
    expect(run.fresh({ ...spell, id: '8' }, window)).toBe(true)
    run.skip.add('8')
    expect(run.fresh({ ...spell, id: '8' }, "Cy's turn · 21 · Precombat main")).toBe(false)
  })
})
