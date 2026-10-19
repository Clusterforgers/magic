// Goal: a search row keeps its menu button in view at any name length: the row body has one column that may shrink below its longest line (jsdom has no layout, so the classes that do it are held), the name is cut with an ellipsis and keeps its full text as its title and as a button that opens the card's preview, the add button covers the body without holding the name (a button cannot sit inside a button) and adds from anywhere else on the row, and the menu button is a sibling that never shrinks.
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { cardHit, deckView } from '@/features/decks/fakeDecks'
import { draftOf } from './deckEdit'
import { ResultRow } from './ResultRow'

const LONGEST =
  'Our Market Research Shows That Players Like Really Long Card Names So We Made this Card to Have the Absolute Longest Card Name Ever Elemental'

function renderRow(name: string) {
  const card = cardHit(name, { mana_cost: '{2}{W}{W} // {3}{R}' })
  const onAdd = vi.fn()
  const onPreview = vi.fn()
  render(
    <ul>
      <ResultRow card={card} draft={draftOf(deckView())} onAdd={onAdd} onPreview={onPreview} />
    </ul>,
  )
  return {
    card,
    onAdd,
    onPreview,
    add: screen.getByRole('button', { name: `Add ${name} to main deck` }),
    title: screen.getByRole('button', { name }),
    menu: screen.getByRole('button', { name: `Add ${name} to another part of the deck` }),
  }
}

describe.each([
  ['a short name', 'Duress'],
  ['the longest name there is', LONGEST],
])('the row of %s', (_what, name) => {
  it('lets the body shrink to the row, so the name is cut and the menu button stays in view', () => {
    const { add, menu } = renderRow(name)
    const body = add.parentElement
    expect(body).toHaveClass('min-w-0', 'flex-1', 'grid-cols-[minmax(0,1fr)]')
    expect(menu).toHaveClass('shrink-0')
    expect(menu.parentElement).toBe(body?.parentElement)
    expect(body?.contains(menu)).toBe(false)
  })

  it('cuts the name with an ellipsis and keeps the whole of it as the title', () => {
    const { title } = renderRow(name)
    expect(title).toHaveClass('truncate', 'min-w-0')
    expect(title).toHaveTextContent(name)
    expect(title).toHaveAttribute('title', name)
  })

  it('keeps the mana cost whole while the name gives way', () => {
    const { add } = renderRow(name)
    expect(add.parentElement?.querySelector('[data-slot="mana-cost"]')).toHaveClass('shrink-0')
  })

  it('covers the body with the add button, which does not hold the name, and lifts the name above it', () => {
    const { add, title } = renderRow(name)
    expect(add).toHaveClass('absolute', 'inset-0')
    expect(add.contains(title)).toBe(false)
    expect(title).toHaveClass('relative', 'z-10')
    expect(title).toHaveAttribute('aria-haspopup', 'dialog')
  })
})

describe('the two actions of a row', () => {
  it('opens the preview from the name, with the name as the opener, and adds nothing', async () => {
    const { card, title, onAdd, onPreview } = renderRow('Duress')
    await userEvent.setup().click(title)
    expect(onPreview).toHaveBeenCalledExactlyOnceWith(card, title)
    expect(onAdd).not.toHaveBeenCalled()
  })

  it('adds a copy to the main deck from the add button and opens no preview', async () => {
    const { card, add, onAdd, onPreview } = renderRow('Duress')
    await userEvent.setup().click(add)
    expect(onAdd).toHaveBeenCalledExactlyOnceWith(card, 'main')
    expect(onPreview).not.toHaveBeenCalled()
  })

  it('is reached by the name first, then the add button, then the menu', async () => {
    const { title, add, menu } = renderRow('Duress')
    const user = userEvent.setup()
    await user.tab()
    expect(title).toHaveFocus()
    await user.tab()
    expect(add).toHaveFocus()
    await user.tab()
    expect(menu).toHaveFocus()
  })
})
