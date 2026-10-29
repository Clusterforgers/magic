// Goal: a Commander deck takes its commanders through the deck check, which refuses a card that cannot lead or pair with the reason, and the slot, the search filter, the result rows and the deck lines follow the commanders' colour identity.
import { screen, waitFor, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { lineOf } from '@/features/decks/fakeDecks'
import {
  BEAR,
  COMMANDER_CHECK,
  COMMANDER_POOL,
  FIREBALL,
  GIANTS,
  KRENKO,
  PIKEMAN,
  THRASIOS,
  TYMNA,
  WILSON,
} from './commanderFixture'
import { asked, deckPane, renderBuilder, searchBox } from './renderBuilder'

const commanderDeck = (commanders: readonly (typeof KRENKO)[] = []) =>
  renderBuilder({
    view: {
      format: 'commander',
      cards: [
        ...commanders.map((card) => lineOf(card.name, 1, { zone: 'commander', card })),
        lineOf('Pikeman', 3, { card: PIKEMAN }),
      ],
    },
    cards: { cards: COMMANDER_POOL },
    decks: { pool: COMMANDER_POOL },
    check: COMMANDER_CHECK,
  })

type Rendered = Awaited<ReturnType<typeof commanderDeck>>

// Searches for `name` and adds the first hit to the command zone through the row's menu.
async function addCommander(app: Rendered, name: string, search: string): Promise<void> {
  await app.user.clear(searchBox())
  await app.user.type(searchBox(), search)
  await app.user.click(await screen.findByRole('button', { name: `Add ${name} to another part of the deck` }))
  await app.user.click(await screen.findByRole('menuitem', { name: 'Commander' }))
}

const slot = () => within(deckPane()).getByRole('region', { name: /^Commander/ })
const commanderNames = () =>
  [...slot().querySelectorAll('[data-commander] [data-slot="card-name"]')].map((el) => el.textContent)

describe('choosing a commander', () => {
  it('shows an empty slot that says what may go there', async () => {
    await commanderDeck()
    expect(slot()).toHaveTextContent('No commander yet')
    expect(screen.getByRole('button', { name: 'Find a commander' })).toBeInTheDocument()
  })

  it('takes a legendary creature and offers a partner or a Background beside it', async () => {
    const app = await commanderDeck()
    await addCommander(app, KRENKO.name, 'krenko')
    await waitFor(() => expect(commanderNames()).toEqual([KRENKO.name]))
    expect(slot()).toHaveTextContent('Red identity')
    expect(screen.getByRole('button', { name: 'Find a partner' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Find a Background' })).toBeInTheDocument()
  })

  it('refuses a card that cannot lead the deck and says why, leaving the slot as it was', async () => {
    const app = await commanderDeck()
    await addCommander(app, BEAR.name, 'grizzly')
    const refusal = await screen.findByRole('alert')
    expect(refusal).toHaveTextContent('Grizzly Bears cannot be added as a commander')
    expect(refusal).toHaveTextContent('it is no legendary creature and does not say it can')
    expect(slot()).toHaveTextContent('No commander yet')
    await app.user.click(within(refusal).getByRole('button', { name: 'Dismiss' }))
    expect(screen.queryByRole('alert')).toBeNull()
  })

  it.each([
    ['a Partner pair', TYMNA, THRASIOS, 'thrasios'],
    ['a creature and its Background', WILSON, GIANTS, 'raised'],
  ] as const)('takes %s', async (_name, first, second, secondText) => {
    const app = await commanderDeck([first])
    await addCommander(app, second.name, secondText)
    await waitFor(() => expect(commanderNames()).toEqual([first.name, second.name].sort()))
    expect(screen.queryByRole('alert')).toBeNull()
  })

  it.each([
    ['two commanders without Partner', KRENKO, TYMNA, 'tymna', 'cannot be partnered'],
    [
      'a Background beside a creature that does not choose one',
      KRENKO,
      GIANTS,
      'raised',
      'cannot be partnered',
    ],
    ['a card that is no commander beside one', TYMNA, BEAR, 'grizzly', 'no legendary creature'],
  ] as const)('refuses %s', async (_name, first, second, secondText, reason) => {
    const app = await commanderDeck([first])
    await addCommander(app, second.name, secondText)
    expect(await screen.findByRole('alert')).toHaveTextContent(reason)
    expect(commanderNames()).toEqual([first.name])
  })

  it('refuses a third commander and a commander it already has', async () => {
    const app = await commanderDeck([TYMNA, THRASIOS])
    await addCommander(app, KRENKO.name, 'krenko')
    expect(await screen.findByRole('alert')).toHaveTextContent('has 3')
    await app.user.click(screen.getByRole('button', { name: 'Dismiss' }))
    await addCommander(app, TYMNA.name, 'tymna')
    expect(await screen.findByRole('alert')).toHaveTextContent('already a commander')
    expect(commanderNames()).toEqual([THRASIOS.name, TYMNA.name].sort())
  })

  it('removes a commander from the slot', async () => {
    const app = await commanderDeck([KRENKO])
    await app.user.click(
      within(slot()).getByRole('button', { name: `Remove ${KRENKO.name} from the command zone` }),
    )
    expect(slot()).toHaveTextContent('No commander yet')
  })

  it('moves a card from the deck into the command zone through the same check', async () => {
    const app = await commanderDeck()
    await app.user.click(
      await screen.findByRole('button', { name: 'Move Pikeman to another part of the deck' }),
    )
    await app.user.click(await screen.findByRole('menuitem', { name: 'One to commander' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('no legendary creature')
  })
})

describe('the colour identity', () => {
  it('limits the search to the commanders colours with a switch, and takes the limit off again', async () => {
    const app = await commanderDeck([TYMNA, THRASIOS])
    await app.user.type(searchBox(), 'a')
    const on = await screen.findByRole('button', { name: /^Within the commander’s colours/ })
    expect(on).toHaveAccessibleName('Within the commander’s colours: White, Blue, Black and Green')
    await app.user.click(on)
    await waitFor(() => expect(asked(app.cards.searches).at(-1)).toMatchObject({ identity: 'WUBG' }))
    expect(on).toHaveAttribute('aria-pressed', 'true')
    await app.user.click(on)
    await waitFor(() => expect(asked(app.cards.searches).at(-1)?.identity).toBeUndefined())
  })

  it('has no switch while the deck has no commander', async () => {
    await commanderDeck()
    expect(screen.queryByRole('button', { name: /^Within the commander’s colours/ })).toBeNull()
  })

  it('says under a result that lies outside the identity which colours are missing', async () => {
    const app = await commanderDeck([TYMNA])
    await app.user.type(searchBox(), 'fire')
    const row = (await screen.findByText(FIREBALL.name)).closest('li')
    expect(row).toHaveTextContent('Outside identity: Red')
  })

  it('looks for a Background when the slot asks, with the type filter set', async () => {
    const app = await commanderDeck([WILSON])
    await app.user.click(screen.getByRole('button', { name: 'Find a Background' }))
    expect(screen.getByLabelText('Type')).toHaveValue('background')
    await waitFor(() => expect(asked(app.cards.searches).at(-1)).toMatchObject({ type: 'background' }))
  })

  it('marks each card outside the identity in the deck and counts them in the slot', async () => {
    const app = await commanderDeck([KRENKO])
    await app.user.type(searchBox(), 'pike')
    await app.user.click(await screen.findByRole('button', { name: `Add ${PIKEMAN.name} to main deck` }))
    await waitFor(() => expect(slot()).toHaveTextContent('1 card lies outside it.'), { timeout: 4000 })
    const line = deckPane().querySelector('li[data-line="main:oracle-pikeman"]')
    expect(line).toHaveAttribute('data-outside', 'true')
    expect(line).toHaveTextContent('Pikeman has W in its color identity')
  })
})
