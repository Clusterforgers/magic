// Goal: a keyword word in a text shows its reminder on press and leads into the lexicon, and text without keywords is left as it is.
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { KeywordText } from './Keyword'
import { lexiconStore } from './lexicon/lexiconStore'

describe('KeywordText', () => {
  it('keeps the words of a text whole and marks its keywords', () => {
    const { container } = render(<KeywordText text="Ninjutsu {1}{U}, then Discover 4." />)
    expect(container.textContent).toBe('Ninjutsu {1}{U}, then Discover 4.')
    expect([...container.querySelectorAll('[data-slot="keyword-word"]')].map((n) => n.textContent)).toEqual([
      'Ninjutsu',
      'Discover',
    ])
  })

  it('marks nothing in a text without keywords', () => {
    const { container } = render(<KeywordText text="Draw a card." />)
    expect(container.querySelector('[data-slot="keyword-word"]')).toBeNull()
  })

  it('shows the reminder of a keyword on press', async () => {
    const user = userEvent.setup()
    render(<KeywordText text="Discover 4." />)
    await user.click(screen.getByRole('button', { name: 'Discover' }))
    expect(
      await screen.findByText(/Exile cards from the top of your library until you exile a nonland card/),
    ).toBeInTheDocument()
  })

  it.each([
    ['Ninjutsu {1}{U}.', 'Ninjutsu', 'Ninjutsu'],
    ['At the beginning of your upkeep, scry 1.', 'upkeep', 'Upkeep'],
  ])('opens the lexicon at the term from "%s"', async (text, word, title) => {
    const user = userEvent.setup()
    render(<KeywordText text={text} />)
    await user.click(screen.getByRole('button', { name: word }))
    await user.click(await screen.findByRole('button', { name: 'More in the lexicon' }))
    expect(lexiconStore.getState()).toEqual({ open: true, query: title, kind: 'all', held: false })
  })
})
