// Goal: a custom card shows its name, art placeholder, type line, rules text and role, and a risky relic says it helps the boss too.
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { CustomCardFrame } from './CustomCardFrame'
import { roleLabel } from './words'
import { CONTENT } from './fakeDelves'

describe('CustomCardFrame', () => {
  it.each(CONTENT.cards.map((card) => [card.name, card] as const))('draws %s', (_, card) => {
    render(<CustomCardFrame card={card} />)
    const figure = screen.getByRole('figure', { name: card.name })
    expect(figure).toHaveAttribute('data-kind', card.kind)
    expect(screen.getByText(card.type_line)).toBeInTheDocument()
    expect(screen.getByText(card.text)).toBeInTheDocument()
    expect(screen.getByRole('img', { name: `Art placeholder: ${card.art}` })).toBeInTheDocument()
  })

  it('says a risky relic helps the boss too', () => {
    expect(roleLabel('Risky')).toMatch(/helps the boss/)
    expect(roleLabel('Healer')).toBe('Healer')
  })
})
