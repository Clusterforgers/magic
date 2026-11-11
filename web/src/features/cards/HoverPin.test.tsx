// Goal: outside the table every card picture shares one large preview that a mouse hover shows and a click pins; a pinned preview stays while other cards are hovered, until its Close button, Escape or a second click on the same card, and it works on the lobby's deck preview, the deck list and the deck editor.
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it } from 'vitest'
import { lineOf } from '@/features/decks/fakeDecks'
import { must } from '@/test-utils/batches'
import { DeckPreview } from '@/features/decks/preview/DeckPreview'
import type { DeckPreviewProps } from '@/features/decks/preview/DeckPreview'
import { KRENKO, COMMANDER_CHECK, COMMANDER_POOL, PIKEMAN } from '@/features/builder/commanderFixture'
import { deckPane, renderBuilder } from '@/features/builder/renderBuilder'
import { CardPicture, HoverPicture } from './CardPicture'
import { hoverFit } from './hoverFit'
import { createHoverPreviewStore, hoverPreview } from './hoverPreview'

afterEach(() => act(() => hoverPreview.dismiss()))

const at = hoverFit({ left: 10, right: 60, top: 10, bottom: 80 }, { width: 1400, height: 900 })
type Step = ['show' | 'hide' | 'pin', string] | ['dismiss']

describe('the shared preview store', () => {
  it.each<[string, Step[], { key: string; pinned: boolean } | null]>([
    ['hover shows a card', [['show', 'a']], { key: 'a', pinned: false }],
    [
      'leaving hides it',
      [
        ['show', 'a'],
        ['hide', 'a'],
      ],
      null,
    ],
    [
      'a hover replaces a hover',
      [
        ['show', 'a'],
        ['show', 'b'],
      ],
      { key: 'b', pinned: false },
    ],
    [
      'a click pins',
      [
        ['show', 'a'],
        ['pin', 'a'],
      ],
      { key: 'a', pinned: true },
    ],
    [
      'a pin stays when its card is left',
      [
        ['pin', 'a'],
        ['hide', 'a'],
      ],
      { key: 'a', pinned: true },
    ],
    [
      'hovering another card keeps the pin',
      [
        ['pin', 'a'],
        ['show', 'b'],
      ],
      { key: 'a', pinned: true },
    ],
    [
      'leaving another card keeps the pin',
      [
        ['pin', 'a'],
        ['show', 'b'],
        ['hide', 'b'],
      ],
      { key: 'a', pinned: true },
    ],
    [
      'a second click on the card unpins',
      [
        ['pin', 'a'],
        ['pin', 'a'],
      ],
      null,
    ],
    ['Close unpins', [['pin', 'a'], ['dismiss']], null],
    [
      'a click on another card pins that one',
      [
        ['pin', 'a'],
        ['pin', 'b'],
      ],
      { key: 'b', pinned: true },
    ],
  ])('%s', (_, steps, expected) => {
    const store = createHoverPreviewStore()
    for (const step of steps) {
      if (step[0] === 'dismiss') store.dismiss()
      else store[step[0]](step[1], at)
    }
    const target = store.get()
    expect(target === null ? null : { key: target.key, pinned: target.pinned }).toEqual(expected)
  })
})

function Two() {
  return (
    <>
      <HoverPicture pin oracleId="bolt" name="Lightning Bolt">
        <CardPicture oracleId="bolt" name="Lightning Bolt" alt="" />
      </HoverPicture>
      <HoverPicture pin oracleId="ring" name="Sol Ring">
        <CardPicture oracleId="ring" name="Sol Ring" alt="" />
      </HoverPicture>
    </>
  )
}

const panel = () => document.querySelector<HTMLElement>('[data-slot="hover-picture"]')
const bolt = () => screen.getByRole('button', { name: 'Show Lightning Bolt large' })
const ring = () => screen.getByRole('button', { name: 'Show Sol Ring large' })
const hover = (element: HTMLElement) => act(() => fireEvent.pointerEnter(element, { pointerType: 'mouse' }))
const leave = (element: HTMLElement) => act(() => fireEvent.pointerLeave(element, { pointerType: 'mouse' }))

describe('a pinnable picture', () => {
  it('shows the card large on hover, in the page body, and hides it on leaving', () => {
    render(<Two />)
    hover(bolt())
    expect(panel()).toHaveTextContent('Lightning Bolt')
    expect(panel()?.parentElement).toBe(document.body)
    expect(panel()).not.toHaveAttribute('data-pinned')
    leave(bolt())
    expect(panel()).toBeNull()
  })

  it('pins on a click and keeps the pin while another card is hovered and left', async () => {
    const user = userEvent.setup()
    render(<Two />)
    await user.click(bolt())
    expect(panel()).toHaveAttribute('data-pinned', 'true')
    expect(bolt()).toHaveAttribute('aria-pressed', 'true')
    leave(bolt())
    hover(ring())
    expect(document.querySelectorAll('[data-slot="hover-picture"]')).toHaveLength(1)
    expect(panel()).toHaveTextContent('Lightning Bolt')
    leave(ring())
    expect(panel()).toHaveTextContent('Lightning Bolt')
    expect(panel()).toHaveAttribute('data-pinned', 'true')
  })

  it.each<[string, (user: ReturnType<typeof userEvent.setup>) => Promise<void>]>([
    ['its Close button', (user) => user.click(screen.getByRole('button', { name: 'Close preview' }))],
    ['a second click on the same card', (user) => user.click(bolt())],
    ['Escape', (user) => user.keyboard('{Escape}')],
  ])('unpins with %s', async (_, close) => {
    const user = userEvent.setup()
    render(<Two />)
    await user.click(bolt())
    hover(ring())
    await close(user)
    expect(panel()).toBeNull()
    expect(bolt()).toHaveAttribute('aria-pressed', 'false')
  })

  it('pins from the keyboard too', async () => {
    const user = userEvent.setup()
    render(<Two />)
    act(() => bolt().focus())
    expect(panel()).toHaveTextContent('Lightning Bolt')
    await user.keyboard('{Enter}')
    expect(panel()).toHaveAttribute('data-pinned', 'true')
    await user.keyboard(' ')
    expect(panel()).toBeNull()
  })

  it('draws the panel inside a dialog around the picture, so a press on it stays in the dialog', async () => {
    const user = userEvent.setup()
    render(
      <div role="dialog" aria-label="Deck">
        <Two />
      </div>,
    )
    await user.click(bolt())
    expect(panel()?.parentElement).toBe(screen.getByRole('dialog', { name: 'Deck' }))
  })

  it('leaves with its card', async () => {
    const user = userEvent.setup()
    const { unmount } = render(<Two />)
    await user.click(bolt())
    unmount()
    expect(hoverPreview.get()).toBeNull()
  })
})

const LOBBY_DECK: DeckPreviewProps['deck'] = {
  name: 'Dragons',
  source: 'Precon C18, Commander Deck',
  format: 'commander',
  commanders: [
    {
      oracleId: 'o-ur-dragon',
      name: 'The Ur-Dragon',
      count: 1,
      typeLine: 'Legendary Creature — Dragon',
      manaValue: 9,
      manaCost: '{4}{W}{U}{B}{R}{G}',
      identity: ['W', 'U', 'B', 'R', 'G'],
      support: 'verified',
      reasons: [],
    },
  ],
  cards: [],
  sideboard: [],
}

describe('the pages outside the table', () => {
  it("shows and pins a commander of the lobby's deck preview", async () => {
    const user = userEvent.setup()
    render(<DeckPreview deck={LOBBY_DECK} />)
    const commander = within(screen.getByRole('list', { name: 'Commanders' })).getByRole('button', {
      name: 'Show The Ur-Dragon large',
    })
    hover(commander)
    expect(panel()).toHaveTextContent('Legendary Creature — Dragon')
    await user.click(commander)
    leave(commander)
    expect(panel()).toHaveAttribute('data-pinned', 'true')
    await user.click(commander)
    expect(panel()).toBeNull()
  })

  it("shows and pins the deck editor's commander, and keeps it pinned over a deck line", async () => {
    const app = await renderBuilder({
      view: {
        format: 'commander',
        cards: [
          lineOf(KRENKO.name, 1, { zone: 'commander', card: KRENKO }),
          lineOf('Pikeman', 3, { card: PIKEMAN }),
        ],
      },
      cards: { cards: COMMANDER_POOL },
      decks: { pool: COMMANDER_POOL },
      check: COMMANDER_CHECK,
    })
    const commander = await within(deckPane()).findByRole('button', { name: `Show ${KRENKO.name} large` })
    hover(commander)
    expect(panel()).toHaveTextContent(KRENKO.name)
    await app.user.click(commander)
    leave(commander)
    const line = must((await within(deckPane()).findAllByRole('button', { name: 'Pikeman' }))[0])
    hover(line)
    await waitFor(() => expect(panel()).toHaveTextContent(KRENKO.name))
    expect(panel()).toHaveAttribute('data-pinned', 'true')
    await app.user.click(screen.getByRole('button', { name: 'Close preview' }))
    expect(panel()).toBeNull()
  })
})
