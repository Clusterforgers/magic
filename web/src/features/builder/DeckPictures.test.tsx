// Goal: the deck pane draws the deck as pictures in groups by type with the copies of each, a quantity badge and the support mark on each card, keeps every control of the list (copies, move, remove, preview), puts the commander first, filters to the cards with issues, and switches to the dense list and back, remembering the choice.
import { screen, within } from '@testing-library/react'
import { beforeEach, describe, expect, it } from 'vitest'
import { cardHit, lineOf } from '@/features/decks/fakeDecks'
import { checked } from './checkedDeck'
import { DECK_LOOK_KEY } from './deckLook'
import { absent, copies, deckLine, deckPane, renderBuilder } from './renderBuilder'

const pane = () => within(deckPane())
const group = (kind: string): HTMLElement => {
  const found = deckPane().querySelector<HTMLElement>(`[data-group="${kind}"]`)
  if (found === null) throw new Error(`no group ${kind}`)
  return found
}

beforeEach(() => window.localStorage.clear())

describe('the pictures', () => {
  it('are grouped by type with the copies of each group, in the order of the shared preview', async () => {
    await renderBuilder({ view: checked(), look: 'pictures' })
    const heads = [...deckPane().querySelectorAll('[data-group]')].map(
      (el) => el.querySelector('h4')?.textContent,
    )
    expect(heads).toEqual(['Creatures 1', 'Instants 6', 'Artifacts 1', 'Lands 20', 'Sorceries 2'])
    expect(pane().getByRole('heading', { name: 'Main deck 28' })).toBeInTheDocument()
  })

  it('keeps a tile in its column and what the pane holds inside the pane (a hidden line cannot stretch the page)', async () => {
    await renderBuilder({ view: checked(), look: 'pictures' })
    expect(deckLine('Lightning Bolt', 'main').className).toContain('grid-cols-[minmax(0,1fr)]')
    const scrolls = [...document.querySelectorAll<HTMLElement>('[class*="overflow-y-auto"]')]
    expect(scrolls.length).toBeGreaterThan(0)
    for (const region of scrolls) expect(region.className).toMatch(/(^| )relative( |$)/)
  })

  it('gives every card a tile with its copies as a badge and as the stepper, and the mark of a card that is not verified', async () => {
    await renderBuilder({ view: checked(), look: 'pictures' })
    const bolt = deckLine('Lightning Bolt', 'main')
    expect(bolt.querySelector('[data-slot="count-badge"]')).toHaveTextContent('×4')
    expect(copies('Lightning Bolt', 'main')).toBe(4)
    expect(bolt).toHaveAttribute('data-support', 'partial')
    expect(within(bolt).getByText('Partial')).toBeInTheDocument()
    expect(bolt.querySelector('[data-slot="card-name"]')?.className).toContain('block')
    const sol = deckLine('Sol Ring', 'main')
    expect(sol.querySelector('[data-slot="count-badge"]')).toBeNull()
    expect(within(sol).getByText('Stale')).toBeInTheDocument()
    expect(within(deckLine('Mountain', 'main')).queryByText('Verified')).toBeNull()
  })

  it('keep every control of a line: copies up and down, remove from the menu, move to the sideboard', async () => {
    const { user } = await renderBuilder({ view: checked(), look: 'pictures' })
    await user.click(screen.getByRole('button', { name: 'Add one Giant Growth' }))
    expect(copies('Giant Growth', 'main')).toBe(3)
    await user.click(screen.getByRole('button', { name: 'Remove one Giant Growth' }))
    expect(copies('Giant Growth', 'main')).toBe(2)
    await user.click(screen.getByRole('button', { name: 'Move Sol Ring to another part of the deck' }))
    await user.click(await screen.findByRole('menuitem', { name: 'To sideboard' }))
    expect(deckLine('Sol Ring', 'sideboard')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Move Giant Growth to another part of the deck' }))
    await user.click(await screen.findByRole('menuitem', { name: 'Remove from the main deck' }))
    expect(absent('Giant Growth')).toBe(true)
  })

  it('take the last copy out with the minus button, as the list does', async () => {
    const { user } = await renderBuilder({ view: checked(), look: 'pictures' })
    await user.click(screen.getByRole('button', { name: 'Remove one Llanowar Elves' }))
    expect(absent('Llanowar Elves')).toBe(true)
    expect(deckPane().querySelector('[data-group="creature"]')).toBeNull()
  })

  it('open the preview of a card from its name and return the focus to it', async () => {
    const { user } = await renderBuilder({ view: checked(), look: 'pictures' })
    const name = within(deckLine('Lightning Bolt', 'main')).getByRole('button', { name: 'Lightning Bolt' })
    await user.click(name)
    expect(await screen.findByRole('dialog')).toBeInTheDocument()
    await user.keyboard('{Escape}')
    await screen.findByRole('button', { name: 'Lightning Bolt', hidden: false })
    expect(name).toHaveFocus()
  })

  it('show only the cards with issues under Issues only, and a zone with none is left out', async () => {
    const { user } = await renderBuilder({ view: checked(), look: 'pictures' })
    await user.click(pane().getByRole('switch', { name: 'Issues only' }))
    expect(absent('Mountain')).toBe(true)
    expect(deckLine('Lightning Bolt')).toBeInTheDocument()
    expect(pane().queryByRole('heading', { name: /^Sideboard/ })).toBeNull()
  })

  it('put the commander before every other card of a Commander deck, drawn large', async () => {
    const krenko = cardHit('Krenko', { type_line: 'Legendary Creature — Goblin', oracle_id: 'oracle-krenko' })
    await renderBuilder({
      look: 'pictures',
      view: {
        format: 'commander',
        cards: [
          lineOf('Lightning Bolt', 1, { card: cardHit('Lightning Bolt', { oracle_id: 'oracle-bolt' }) }),
          lineOf('Krenko', 1, { zone: 'commander', card: krenko }),
        ],
      },
    })
    const slot = deckPane().querySelector('[data-slot="commander-slot"]')
    const zone = deckPane().querySelector('[data-slot="picture-zone"]')
    expect(slot?.querySelector('[data-commander="oracle-krenko"]')).not.toBeNull()
    expect(slot?.compareDocumentPosition(zone as Node)).toBe(Node.DOCUMENT_POSITION_FOLLOWING)
  })
})

describe('the look', () => {
  it('switches between pictures and the dense list, keeping the lines and the choice', async () => {
    const { user } = await renderBuilder({ view: checked(), look: 'pictures' })
    const toggle = pane().getByRole('group', { name: 'Show cards as' })
    expect(within(toggle).getByRole('button', { name: 'Pictures' })).toHaveAttribute('aria-pressed', 'true')
    expect(deckPane().querySelector('[data-slot="picture-zone"]')).not.toBeNull()
    await user.click(within(toggle).getByRole('button', { name: 'List' }))
    expect(deckPane().querySelector('[data-slot="picture-zone"]')).toBeNull()
    expect(pane().getByRole('list', { name: 'Main deck cards' })).toBeInTheDocument()
    expect(copies('Lightning Bolt', 'main')).toBe(4)
    expect(window.localStorage.getItem(DECK_LOOK_KEY)).toBe('list')
    await user.click(within(toggle).getByRole('button', { name: 'Pictures' }))
    expect(group('instant')).toBeInTheDocument()
  })
})
