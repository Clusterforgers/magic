// Goal: a search row keeps its add menu button in view at any name length: the add button has one column that may shrink below its longest line (jsdom has no layout, so the classes that do it are held), the name is cut with an ellipsis and keeps its full text as its title and the button's name, and the menu button is a sibling that never shrinks.
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { cardHit, deckView } from '@/features/decks/fakeDecks'
import { draftOf } from './deckEdit'
import { ResultRow } from './ResultRow'

const LONGEST =
  'Our Market Research Shows That Players Like Really Long Card Names So We Made this Card to Have the Absolute Longest Card Name Ever Elemental'

function renderRow(name: string) {
  render(
    <ul>
      <ResultRow
        card={cardHit(name, { mana_cost: '{2}{W}{W} // {3}{R}' })}
        draft={draftOf(deckView())}
        onAdd={() => undefined}
      />
    </ul>,
  )
  return {
    add: screen.getByRole('button', { name: `Add ${name} to main deck` }),
    menu: screen.getByRole('button', { name: `Add ${name} to another part of the deck` }),
  }
}

describe.each([
  ['a short name', 'Duress'],
  ['the longest name there is', LONGEST],
])('the row of %s', (_what, name) => {
  it('lets the add button shrink to the row, so the name is cut and the menu button stays in view', () => {
    const { add, menu } = renderRow(name)
    expect(add).toHaveClass('min-w-0', 'flex-1', 'grid-cols-[minmax(0,1fr)]')
    expect(menu).toHaveClass('shrink-0')
    expect(menu.parentElement).toBe(add.parentElement)
    expect(add.contains(menu)).toBe(false)
  })

  it('cuts the name with an ellipsis and keeps the whole of it as the title', () => {
    const { add } = renderRow(name)
    const shown = add.querySelector<HTMLElement>('.truncate')
    expect(shown).toHaveTextContent(name)
    expect(shown).toHaveAttribute('title', name)
  })

  it('keeps the mana cost whole while the name gives way', () => {
    const { add } = renderRow(name)
    expect(add.querySelector('[data-slot="mana-cost"]')).toHaveClass('shrink-0')
  })
})
