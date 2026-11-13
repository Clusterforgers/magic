// Goal: the Delves rules in short say how Spirits work and how a fallen hero comes back, folded until opened, and the run page's revival note lists every way back.
import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { DelvesRules, REVIVAL_WAYS, RevivalNote, SPIRIT_RULES } from './DelvesRules'

describe('the rules in short', () => {
  it.each([
    ['How Spirits work', SPIRIT_RULES],
    ['Bringing a hero back', REVIVAL_WAYS],
  ])('folds %s and opens it on a click', (title, lines) => {
    render(<DelvesRules />)
    const summary = screen.getByText(title)
    const entry = summary.closest('details')
    expect(entry).not.toHaveAttribute('open')
    fireEvent.click(summary)
    expect(entry).toHaveAttribute('open')
    for (const text of lines) expect(entry).toHaveTextContent(text)
  })

  it('says Spirits can be neither attacked nor targeted and how they rise', () => {
    expect(SPIRIT_RULES.join(' ')).toMatch(/can't attack a Spirit or target it/)
    expect(SPIRIT_RULES.join(' ')).toMatch(/Rise Again/)
  })
})

describe('the revival note', () => {
  it('lists every way back', () => {
    render(<RevivalNote />)
    const note = screen.getByRole('complementary', { name: 'Fallen heroes' })
    expect(note.querySelectorAll('li')).toHaveLength(REVIVAL_WAYS.length)
    expect(note).toHaveTextContent('Phoenix Feather')
  })
})
