// Goal: a custom card shows its name, art placeholder, type line, rules text and role, and only a risky relic wears the Dangerous badge and says it helps the boss too.
import { render, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { CustomCardFrame } from './CustomCardFrame'
import { isRisky } from './model'
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
    const badge = within(figure).queryByText('Dangerous')
    if (isRisky(card)) {
      expect(badge).toBeInTheDocument()
      expect(figure).toHaveAttribute('data-risky', 'true')
      expect(within(figure).getByText(roleLabel(card.role))).toBeInTheDocument()
    } else {
      expect(badge).toBeNull()
      expect(figure).not.toHaveAttribute('data-risky')
    }
  })

  it('says a risky relic helps the boss too', () => {
    expect(roleLabel('Risky')).toMatch(/helps the boss/)
    expect(roleLabel('Healer')).toBe('Healer')
  })
})
